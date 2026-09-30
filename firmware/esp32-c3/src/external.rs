//! External serial test mode (`wokwi-serial` feature).
//!
//! Unlike the internal self-test (which drives the core from inside the firmware), this mode runs the
//! **real receive/transmit loop**: bytes arrive from *outside* the device over USB Serial/JTAG — injected
//! by Wokwi `write-serial` automation steps — and are handed to the real COBS/header/CRC/postcard
//! decoder, the real sequence policy, and the real [`Dispatcher`], which writes real response frames
//! back out through the same peripheral.
//!
//! Alongside the binary frames the harness prints redacted `KIVORI-EXT …` markers so a scenario can
//! assert semantics without reading payload bytes. Markers carry message *kinds*, sequence numbers, and
//! state names only — never payload contents or raw identity (ADR-0005). Each marker begins on a fresh
//! line so it is never glued onto the binary bytes that share the stream.
//!
//! [`StateCycle`] tracks the state-cycle post-conditions from **observed** events and emits
//! `ALL PASS state-cycle` only once every one of them has genuinely happened — a scenario can therefore
//! never pass merely because the simulator exited or because a boot-time marker scrolled by.
//!
//! Scope: this proves the firmware's serial receive/transmit path works against a simulated USB
//! Serial/JTAG peripheral. It does NOT prove the device enumerates as an OS serial port, and the Tauri
//! desktop cannot open a Wokwi device — that remains a hardware task (T115/T117).

use crate::health::build_health;
use crate::ports::Transport;
use crate::proto::{DeviceIdentity, Dispatcher};
use crate::state::{DeviceEvent, DeviceState};
use crate::transport::UsbJtagTransport;
use core::fmt::Write as _;
use heapless::{String, Vec};
use kivori_model::{Capabilities, CompanionState, SendableState};
use kivori_protocol::{
    decode_message, FirmwareVersion, Message, SeqClass, SequenceTracker, MAX_FRAME, MAX_WIRE,
    PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

/// Marker prefix every line shares, so scenarios can filter the stream.
const TAG: &str = "KIVORI-EXT";
/// Inbound accumulation capacity (one full wire packet plus slack).
const RX_CAPACITY: usize = MAX_WIRE * 2;

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: [0x5A; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: Capabilities::MASCOT_INTERACTION
            .union(Capabilities::PHYSICAL_INPUT_V1)
            .union(Capabilities::PRESENTATION_V1),
    }
}

/// Prints one marker line over the same USB Serial/JTAG the protocol uses. The leading CRLF guarantees
/// the marker starts a line even when binary response bytes were just transmitted.
fn line(io: &mut UsbJtagTransport<'_>, text: &str) {
    let mut out: String<192> = String::new();
    let _ = write!(out, "\r\n{TAG} {text}\r\n");
    io.write_all(out.as_bytes());
}

/// Wire token for a companion state (marker text only).
fn state_name(state: CompanionState) -> &'static str {
    match state {
        CompanionState::Booting => "booting",
        CompanionState::Idle => "idle",
        CompanionState::Happy => "happy",
        CompanionState::Busy => "busy",
        CompanionState::Sleeping => "sleeping",
        CompanionState::Offline => "offline",
    }
}

/// Marker name for a message kind (never its contents).
fn kind_name(message: &Message) -> &'static str {
    match message {
        Message::Hello(_) => "Hello",
        Message::HelloAck(_) => "HelloAck",
        Message::Ready(_) => "Ready",
        Message::Bye(_) => "Bye",
        Message::SetState(_) => "SetState",
        Message::StateReport(_) => "StateReport",
        Message::Ping(_) => "Ping",
        Message::Pong(_) => "Pong",
        Message::Health(_) => "Health",
        Message::Diagnostic(_) => "Diagnostic",
        Message::Error(_) => "Error",
        Message::PlayMascotAction(_) => "PlayMascotAction",
        Message::MascotActionApplied(_) => "MascotActionApplied",
        Message::InputEvent(_) => "InputEvent",
        Message::Presentation(_) => "Presentation",
    }
}

fn seq_class_name(class: SeqClass) -> &'static str {
    match class {
        SeqClass::First => "first",
        SeqClass::Ok => "ok",
        SeqClass::Duplicate => "duplicate",
        SeqClass::Gap(_) => "gap",
    }
}

/// Observed post-conditions for the state-cycle scenario.
///
/// Every flag is set from a real event (a decoded inbound frame, a decoded outbound frame, or the
/// device's own state) — never from "the step ran". `ALL PASS state-cycle` is emitted exactly once, and
/// only when all of them hold.
#[derive(Default)]
struct StateCycle {
    /// The structural sendable guarantee held at boot (FR-014/FR-015).
    sendable_guard: bool,
    /// A `HelloAck` was actually transmitted.
    hello_ack_tx: bool,
    /// `StateReport(idle)` was transmitted while the device really was idle.
    report_idle_tx: bool,
    /// `StateReport(happy)` was transmitted while the device really was happy.
    report_happy_tx: bool,
    /// The real decoder rejected at least one frame.
    drop_seen: bool,
    /// The device's state was unchanged across that rejected frame.
    state_intact_after_drop: bool,
    /// A valid frame was served (a response transmitted) after the rejected one.
    recovery_after_drop: bool,
    /// The completion marker has been emitted (emit once).
    announced: bool,
}

impl StateCycle {
    /// Records an outbound message and the device state at that moment.
    fn on_tx(&mut self, message: &Message, current: CompanionState) {
        match message {
            Message::HelloAck(_) => self.hello_ack_tx = true,
            Message::StateReport(report) => match report.reported {
                CompanionState::Idle if current == CompanionState::Idle => {
                    self.report_idle_tx = true;
                }
                CompanionState::Happy if current == CompanionState::Happy => {
                    self.report_happy_tx = true;
                }
                _ => {}
            },
            _ => {}
        }
        if self.drop_seen {
            self.recovery_after_drop = true;
        }
    }

    /// Records that the real decoder rejected a frame, and whether the state survived it.
    fn on_drop(&mut self, before: CompanionState, after: CompanionState) {
        self.drop_seen = true;
        self.state_intact_after_drop = before == after;
    }

    fn complete(&self) -> bool {
        self.sendable_guard
            && self.hello_ack_tx
            && self.report_idle_tx
            && self.report_happy_tx
            && self.drop_seen
            && self.state_intact_after_drop
            && self.recovery_after_drop
    }

    /// Emits the completion marker once, after every post-condition genuinely holds.
    fn announce_if_complete(&mut self, io: &mut UsbJtagTransport<'_>) {
        if !self.announced && self.complete() {
            self.announced = true;
            line(io, "ALL PASS state-cycle");
        }
    }
}

/// Runs the external serial loop forever. `now_ms` is supplied by the caller's real clock.
pub fn run<F: FnMut() -> u32>(io: &mut UsbJtagTransport<'_>, mut now_ms: F) -> ! {
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());
    let mut tracker = SequenceTracker::new();
    let mut progress = StateCycle::default();
    let mut rx: Vec<u8, RX_CAPACITY> = Vec::new();
    let mut chunk = [0u8; 128];

    // The device owns booting → offline before any host drives it (FR-014).
    let _ = device.apply(DeviceEvent::BootComplete);

    let mut ready: String<128> = String::new();
    let _ = write!(
        ready,
        "READY iface=usb-serial-jtag proto={PROTOCOL_MAJOR}.{PROTOCOL_MINOR} state={}",
        state_name(device.current())
    );
    line(io, &ready);

    // The structural sendable guarantee (FR-014/FR-015): `SendableState` has exactly four variants and
    // none of them is device-originated, so no encodable `SetState` payload can carry
    // `booting`/`offline`. Asserted at boot and folded into the completion condition.
    progress.sendable_guard = SendableState::ALL.len() == 4
        && SendableState::ALL
            .iter()
            .all(|s| !s.to_companion().is_device_originated());
    line(
        io,
        if progress.sendable_guard {
            "SENDABLE-GUARD ok"
        } else {
            "FAIL sendable-guard"
        },
    );

    let health = build_health(0);
    let mut health_line: String<64> = String::new();
    let _ = write!(health_line, "HEALTH free_bytes={}", health.free_bytes);
    line(io, &health_line);

    loop {
        // 1. Real receive path: pull whatever the (simulated) host has sent.
        if let Ok(n) = io.read(&mut chunk) {
            for &byte in &chunk[..n] {
                if rx.push(byte).is_err() {
                    rx.clear();
                    let _ = rx.push(byte);
                }
            }
        }

        // 2. Frame-by-frame: report what the real decoder made of it, then let the real dispatcher act.
        while let Some(pos) = rx.iter().position(|&b| b == 0) {
            let mut packet: Vec<u8, MAX_WIRE> = Vec::new();
            let _ = packet.extend_from_slice(&rx[..pos.min(MAX_WIRE)]);
            // Drop the consumed bytes (including the delimiter).
            let mut tail: Vec<u8, RX_CAPACITY> = Vec::new();
            let _ = tail.extend_from_slice(&rx[(pos + 1).min(rx.len())..]);
            rx = tail;
            if packet.is_empty() {
                continue;
            }

            let state_before = device.current();
            let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
            let decoded = decode_message(&packet, &mut scratch, &[PROTOCOL_MAJOR]);
            let rejected = decoded.is_err();
            match decoded {
                Ok((header, message)) => {
                    let class = tracker.classify(header.seq);
                    let mut rx_line: String<128> = String::new();
                    let _ = write!(
                        rx_line,
                        "RX kind={} seq={} len={}",
                        kind_name(&message),
                        header.seq,
                        header.payload_len
                    );
                    line(io, &rx_line);
                    let mut seq_line: String<64> = String::new();
                    let _ = write!(
                        seq_line,
                        "SEQ seq={} class={}",
                        header.seq,
                        seq_class_name(class)
                    );
                    line(io, &seq_line);
                }
                Err(_) => {
                    // Malformed framing, bad CRC, or an unsupported major: dropped with no side effect
                    // and no response frame (SC-008, FR-003). The reason stays a safe category — never
                    // the offending bytes. The frame is still handed to the dispatcher below, which must
                    // survive it and keep serving the next valid frame.
                    line(io, "DROP reason=decode");
                }
            }

            // 3. Real dispatch: the Dispatcher consumes this frame through a buffered view of the link
            //    and writes real response frames, which are captured and then transmitted verbatim.
            let mut replay = ReplayLink {
                inbound: packet.clone(),
                cursor: 0,
                out: Vec::new(),
            };
            let _ = replay.inbound.push(0);
            let _ = dispatcher.poll(&mut replay, &mut device, now_ms());

            if rejected {
                // The state must be untouched by a frame the decoder refused.
                progress.on_drop(state_before, device.current());
            }

            // Forward the device's real response bytes to the host, then report their semantics.
            if !replay.out.is_empty() {
                io.write_all(&replay.out);
                let mut reply_scratch: Vec<u8, MAX_FRAME> = Vec::new();
                for reply in replay.out.split(|&b| b == 0) {
                    if reply.is_empty() {
                        continue;
                    }
                    if let Ok((_, msg)) =
                        decode_message(reply, &mut reply_scratch, &[PROTOCOL_MAJOR])
                    {
                        report_tx(io, &msg);
                        progress.on_tx(&msg, device.current());
                    }
                }
            }

            let mut state_line: String<64> = String::new();
            let _ = write!(state_line, "STATE current={}", state_name(device.current()));
            line(io, &state_line);

            // 4. Announce completion only once every observed post-condition holds.
            progress.announce_if_complete(io);
        }
    }
}

/// Reports one outbound message as markers (kind + the one safe field a scenario asserts).
fn report_tx(io: &mut UsbJtagTransport<'_>, message: &Message) {
    let mut out: String<160> = String::new();
    match message {
        Message::HelloAck(ack) => {
            let _ = write!(
                out,
                "TX kind=HelloAck nonce={}",
                if ack.nonce_echo == 0 { "zero" } else { "ok" }
            );
            line(io, &out);
            let mut caps: String<64> = String::new();
            let _ = write!(caps, "CAPS advertised={:#010x}", ack.device_caps.bits());
            line(io, &caps);
        }
        Message::Pong(pong) => {
            let _ = write!(out, "TX kind=Pong echo={}", pong.t_ms_echo);
            line(io, &out);
        }
        Message::StateReport(report) => {
            let _ = write!(
                out,
                "TX kind=StateReport reported={}",
                state_name(report.reported)
            );
            line(io, &out);
        }
        other => {
            let _ = write!(out, "TX kind={}", kind_name(other));
            line(io, &out);
        }
    }
}

/// A buffered view of one already-received frame, so the real [`Dispatcher`] can consume it while its
/// responses are captured for reporting (and then forwarded to the host verbatim).
struct ReplayLink {
    inbound: Vec<u8, MAX_WIRE>,
    cursor: usize,
    out: Vec<u8, MAX_WIRE>,
}

impl Transport for ReplayLink {
    type Error = ();

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let remaining = self.inbound.len().saturating_sub(self.cursor);
        let n = buf.len().min(remaining);
        buf[..n].copy_from_slice(&self.inbound[self.cursor..self.cursor + n]);
        self.cursor += n;
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let _ = self.out.extend_from_slice(buf);
        Ok(buf.len())
    }
}
