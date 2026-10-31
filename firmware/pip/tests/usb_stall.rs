//! A host that stops reading the device's USB output for a while (a Mac under heavy load) while the
//! knob is turned fast: the production runtime over the production outbound queue ([`TxBuffered`])
//! over a pipe whose host side stalls like the real 64-byte USB FIFO ([`SimPipe::set_host_stalled`]).
//!
//! The session must survive: rotation that cannot be sent right away is kept as a bounded net count
//! and sent as space frees, button events keep their order, and heartbeat replies are never starved.
//! Host simulation only; a physical USB stall is a hardware row (docs/virtual-pip.md, Limits).
#![cfg(feature = "host-sim")]

use std::cell::RefCell;
use std::collections::VecDeque;

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::{Clock, InputSource, Transport};
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig, Tick};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_firmware::tx_buffer::{TxBuffered, FIFO_BYTES, TX_QUEUE_BYTES};
use kivori_model::input::{Direction, InputLevels};
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, ControlId, FirmwareVersion, Hello, InputEvent, InputKind,
    Message, Ping, Ready, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const CAPS: Capabilities = Capabilities::PHYSICAL_INPUT_V1
    .union(Capabilities::BUTTON_INPUT_V1)
    .union(Capabilities::CONTEXT_BUTTONS_V1);
const NONCE: u32 = 0x5EED_0007;
/// One detent every 10 ms: the fast human spin of the task (about 100 a second).
const DETENT_MS: u32 = 10;
/// A detent is four level snapshots, the last at this offset from its start: where the device
/// stamps the detent it completes.
const DETENT_DONE_MS: u32 = 3;

/// The captured level snapshots the test schedules, handed over once the runtime's clock reaches them.
#[derive(Default)]
struct Scheduled {
    edges: RefCell<VecDeque<(InputLevels, u32)>>,
    last: RefCell<InputLevels>,
}

impl InputSource for &Scheduled {
    fn sample(&mut self) -> InputLevels {
        *self.last.borrow()
    }

    fn drain(&mut self, now_ms: u32, f: &mut dyn FnMut(InputLevels, u32)) {
        let mut edges = self.edges.borrow_mut();
        while edges.front().is_some_and(|(_, at)| *at <= now_ms) {
            let (levels, at) = edges.pop_front().expect("front checked");
            *self.last.borrow_mut() = levels;
            f(levels, at);
        }
        f(*self.last.borrow(), now_ms);
    }
}

fn levels(mask: u8) -> InputLevels {
    InputLevels {
        a: mask & 1 != 0,
        b: mask & 2 != 0,
        sw: mask & 4 != 0,
        keys: [mask & 8 != 0, mask & 16 != 0, mask & 32 != 0],
    }
}

struct Rig {
    runtime: Runtime<'static>,
    clock: VirtualClock,
    tx: TxBuffered<SimPipe>,
    display: Box<CaptureDisplay>,
    blob: AssetBlob<'static>,
    input: Scheduled,
    seq: u16,
    /// Everything the host has read so far, decoded in arrival order.
    received: Vec<Message>,
    /// Whether any tick reported the session dropped.
    link_dropped: bool,
    /// Packets that did not decode (a frame cut short on the wire).
    garbled: u32,
}

impl Rig {
    fn new() -> Self {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            device_id: [0xAB; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            capabilities: CAPS,
        };
        let mut rig = Self {
            runtime: Runtime::new(identity, RuntimeConfig::default()),
            clock: VirtualClock::new(),
            tx: TxBuffered::new(SimPipe::new()),
            display: Box::new(CaptureDisplay::new()),
            blob: AssetBlob::parse(bytes).expect("valid blob"),
            input: Scheduled::default(),
            seq: 0,
            received: Vec::new(),
            link_dropped: false,
            garbled: 0,
        };
        rig.step();
        rig.send(&Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: CAPS,
            nonce: NONCE,
        }));
        rig.step();
        rig.send(&Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: CAPS,
        }));
        rig.step();
        rig.received.clear();
        rig
    }

    fn send(&mut self, message: &Message) {
        let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
        encode_message(
            message,
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            self.seq,
            &mut wire,
        )
        .expect("encode");
        self.seq += 1;
        self.tx.inner_mut().host_send(&wire).expect("pipe capacity");
    }

    fn step(&mut self) -> Tick {
        let mut input = &self.input;
        let tick = self.runtime.step(
            &self.clock,
            &mut self.tx,
            &mut input,
            self.display.as_mut(),
            &self.blob,
        );
        self.link_dropped |= tick.link_dropped;
        tick
    }

    /// Runs 2 ms ticks until `until_ms`, the host reading (or not) according to `stalled`.
    fn run_until(&mut self, until_ms: u32) {
        while self.clock.now_ms() < until_ms {
            self.clock.advance(2);
            self.step();
            self.host_read();
        }
    }

    fn host_read(&mut self) {
        let bytes = self.tx.inner_mut().host_recv();
        let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
        for packet in bytes.split(|&b| b == 0).filter(|p| !p.is_empty()) {
            match decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
                Ok((_, message)) => self.received.push(message),
                Err(_) => self.garbled += 1,
            }
        }
    }

    fn stall(&mut self, stalled: bool) {
        self.tx.inner_mut().set_host_stalled(stalled);
    }

    /// Schedules `count` detents in `direction`, one every [`DETENT_MS`], the first at `start_ms`.
    fn spin(&mut self, direction: Direction, count: u32, start_ms: u32) {
        self.spin_every(direction, count, start_ms, DETENT_MS);
    }

    /// Like [`Self::spin`] with a detent every `period_ms` (at least 4: a detent is four snapshots).
    fn spin_every(&mut self, direction: Direction, count: u32, start_ms: u32, period_ms: u32) {
        let phases: [u8; 4] = match direction {
            Direction::Cw => [2, 3, 1, 0],
            Direction::Ccw => [1, 3, 2, 0],
        };
        let mut edges = self.input.edges.borrow_mut();
        for detent in 0..count {
            for (i, phase) in phases.into_iter().enumerate() {
                edges.push_back((levels(phase), start_ms + detent * period_ms + i as u32));
            }
        }
    }

    /// Schedules a push-switch press and release (a short press) at `down_ms`.
    fn press(&mut self, down_ms: u32) {
        let mut edges = self.input.edges.borrow_mut();
        edges.push_back((levels(4), down_ms));
        edges.push_back((levels(0), down_ms + 80));
    }

    /// Schedules one contextual key press and release at `down_ms`.
    fn key(&mut self, index: u8, down_ms: u32) {
        let bit = 8u8 << index;
        let mut edges = self.input.edges.borrow_mut();
        edges.push_back((levels(bit), down_ms));
        edges.push_back((levels(0), down_ms + 80));
    }

    fn events(&self) -> Vec<InputEvent> {
        self.received
            .iter()
            .filter_map(|message| match message {
                Message::InputEvent(event) => Some(*event),
                _ => None,
            })
            .collect()
    }

    /// Net rotation the host sees: clockwise detents minus counter-clockwise ones.
    fn net_rotation(&self) -> i32 {
        self.events()
            .iter()
            .map(|event| match event.kind {
                InputKind::Detent(Direction::Cw) => 1,
                InputKind::Detent(Direction::Ccw) => -1,
                _ => 0,
            })
            .sum()
    }

    /// The input events that are not rotary, in arrival order.
    fn switch_events(&self) -> Vec<(ControlId, InputKind)> {
        self.events()
            .iter()
            .filter(|event| event.control != ControlId::Rotary)
            .map(|event| (event.control, event.kind))
            .collect()
    }

    fn ping(&mut self, t_ms: u32) {
        self.send(&Message::Ping(Ping { t_ms }));
    }

    fn pongs(&self) -> Vec<u32> {
        self.received
            .iter()
            .filter_map(|message| match message {
                Message::Pong(pong) => Some(pong.t_ms_echo),
                _ => None,
            })
            .collect()
    }

    /// The stream the host decoded obeys the gesture rules the desktop enforces: a detent belongs to
    /// an open gesture, and a gesture ends only once it was started.
    fn assert_gestures_well_formed(&self) {
        let mut open = std::collections::BTreeSet::new();
        for event in self
            .events()
            .iter()
            .filter(|e| e.control == ControlId::Rotary)
        {
            match event.kind {
                InputKind::GestureStarted => {
                    open.insert(event.gesture_id);
                }
                InputKind::Detent(_) => {
                    assert!(open.contains(&event.gesture_id), "detent outside a gesture")
                }
                InputKind::GestureEnded => {
                    assert!(open.remove(&event.gesture_id), "gesture ended unstarted")
                }
                other => panic!("rotary control carried {other:?}"),
            }
        }
    }
}

#[test]
fn a_host_stall_during_a_fast_spin_keeps_the_session() {
    let mut rig = Rig::new();
    // 120 detents at 100 a second; the host reads nothing from 100 ms to 800 ms, longer than the
    // outbound queue holds at this pace (about 40 events, 400 ms).
    rig.spin(Direction::Cw, 120, 20);
    rig.run_until(100);
    rig.stall(true);
    rig.run_until(800);
    rig.stall(false);
    rig.run_until(1_700);
    assert!(
        !rig.link_dropped,
        "a stall the host recovers from must not end the session"
    );
    assert_eq!(rig.garbled, 0, "no frame may be cut short on the wire");
    assert_eq!(
        rig.net_rotation(),
        120,
        "every detent turned reaches the host as net rotation"
    );
    rig.assert_gestures_well_formed();
}

#[test]
fn a_short_stall_of_a_very_fast_spin_keeps_the_session() {
    let mut rig = Rig::new();
    // 300 detents at 250 a second (the endurance flood's pace): the queue is full 150 ms in.
    rig.spin_every(Direction::Ccw, 300, 20, 4);
    rig.run_until(100);
    rig.stall(true);
    rig.run_until(500);
    rig.stall(false);
    rig.run_until(1_700);
    assert!(!rig.link_dropped);
    assert_eq!(rig.net_rotation(), -300);
    rig.assert_gestures_well_formed();
}

#[test]
fn net_rotation_after_a_stall_equals_the_detents_turned_in_both_directions() {
    let mut rig = Rig::new();
    // Clockwise 70, a pause long enough to end the gesture, counter-clockwise 25, clockwise 6;
    // the host reads nothing across all of it. Net: 70 - 25 + 6 = 51.
    rig.spin(Direction::Cw, 70, 20);
    rig.spin(Direction::Ccw, 25, 1_100);
    rig.spin(Direction::Cw, 6, 1_450);
    rig.run_until(60);
    rig.stall(true);
    rig.run_until(2_000);
    rig.stall(false);
    rig.run_until(3_000);
    assert!(!rig.link_dropped);
    assert_eq!(rig.net_rotation(), 51, "no lost or duplicated step");
    rig.assert_gestures_well_formed();
    // Every gesture the host saw was closed.
    let events = rig.events();
    let started = events
        .iter()
        .filter(|e| e.kind == InputKind::GestureStarted)
        .count();
    let ended = events
        .iter()
        .filter(|e| e.kind == InputKind::GestureEnded)
        .count();
    assert_eq!(started, ended);
}

#[test]
fn presses_during_a_stall_arrive_intact_and_in_order_after_the_rotation() {
    let mut rig = Rig::new();
    // 50 detents fill the queue; the gesture closes 250 ms after the last (at about 773 ms), then
    // the switch and two keys are pressed, all while the host still reads nothing.
    rig.spin(Direction::Cw, 50, 20);
    rig.press(900);
    rig.key(1, 1_100);
    rig.key(0, 1_300);
    rig.run_until(60);
    rig.stall(true);
    rig.run_until(1_600);
    rig.stall(false);
    rig.run_until(2_500);
    assert!(!rig.link_dropped);
    assert_eq!(rig.garbled, 0);
    assert_eq!(
        rig.switch_events(),
        [
            (ControlId::Button, InputKind::Press),
            (ControlId::ContextButton(1), InputKind::Press),
            (ControlId::ContextButton(0), InputKind::Press),
        ],
        "each press arrives once, in the order it happened"
    );
    assert_eq!(rig.net_rotation(), 50);
    // The rotation, closed by its own `GestureEnded`, precedes the presses that came after it.
    let events = rig.events();
    let last_rotary = events
        .iter()
        .rposition(|e| e.control == ControlId::Rotary)
        .unwrap();
    let first_switch = events
        .iter()
        .position(|e| e.control != ControlId::Rotary)
        .unwrap();
    assert!(
        last_rotary < first_switch,
        "a press never overtakes earlier rotation"
    );
    assert_eq!(events[last_rotary].kind, InputKind::GestureEnded);
    rig.assert_gestures_well_formed();
}

#[test]
fn a_heartbeat_reply_is_answered_after_a_stall_even_with_the_queue_full_of_input() {
    let mut rig = Rig::new();
    rig.spin(Direction::Cw, 120, 20);
    rig.run_until(60);
    rig.stall(true);
    // Mid-spin, with the queue at its input limit, the host pings; after the stall it must get the Pong.
    rig.run_until(700);
    rig.ping(4_242);
    rig.run_until(900);
    rig.stall(false);
    rig.run_until(1_700);
    assert!(!rig.link_dropped);
    assert_eq!(rig.pongs(), [4_242], "the Pong is not starved by input");
    assert_eq!(rig.net_rotation(), 120);
}

#[test]
fn normal_load_is_unchanged_every_detent_goes_out_at_once_with_its_own_time() {
    let mut rig = Rig::new();
    rig.spin(Direction::Cw, 60, 20);
    rig.run_until(1_500);
    assert!(!rig.link_dropped);
    let events = rig.events();
    let detents: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, InputKind::Detent(_)))
        .collect();
    assert_eq!(detents.len(), 60);
    for (k, event) in detents.iter().enumerate() {
        assert_eq!(
            event.device_ms,
            20 + k as u32 * DETENT_MS + DETENT_DONE_MS,
            "detent {k} keeps the time of the edge that completed it"
        );
    }
    assert_eq!(
        events.first().map(|e| e.kind),
        Some(InputKind::GestureStarted)
    );
    assert_eq!(events.last().map(|e| e.kind), Some(InputKind::GestureEnded));
    rig.assert_gestures_well_formed();
}

#[test]
fn input_queued_in_a_stall_never_reaches_a_later_session() {
    let mut rig = Rig::new();
    rig.spin(Direction::Cw, 100, 20);
    rig.run_until(60);
    rig.stall(true);
    rig.run_until(700);
    // The desktop reconnects (new nonce) while the old session's input still waits.
    rig.send(&Message::Hello(Hello {
        desktop_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        desktop_caps: CAPS,
        nonce: NONCE + 1,
    }));
    rig.run_until(750);
    rig.stall(false);
    rig.run_until(1_700);
    let acked = rig
        .received
        .iter()
        .position(|m| matches!(m, Message::HelloAck(ack) if ack.nonce_echo == NONCE + 1))
        .expect("the new session is acknowledged");
    assert!(
        rig.received[acked..]
            .iter()
            .all(|m| !matches!(m, Message::InputEvent(_))),
        "no input of the old session follows the new handshake"
    );
}

#[test]
fn input_that_outgrows_its_bound_ends_the_session_instead_of_growing() {
    let mut rig = Rig::new();
    // Fill the queue with rotation, then press more keys than the pending bound holds while the
    // host still reads nothing: memory stays bounded, so the session ends (the old behaviour).
    rig.spin(Direction::Cw, 60, 20);
    for k in 0..12 {
        rig.key(0, 1_000 + k * 200);
    }
    rig.run_until(60);
    rig.stall(true);
    rig.run_until(4_000);
    assert!(rig.link_dropped);
}

#[test]
fn control_frames_fit_in_the_room_input_leaves() {
    use kivori_model::internals::{ChipModel, DeviceInternals, ResetReason};
    use kivori_protocol::{BuildInfo, Health, HelloAck, Identity, InternalsReport, Pong};

    let full = DeviceInternals {
        app_image_bytes: Some(u32::MAX),
        app_partition_bytes: Some(u32::MAX),
        asset_blob_bytes: Some(u32::MAX),
        asset_blob_cap_bytes: Some(u32::MAX),
        flash_total_bytes: Some(u32::MAX),
        ram_total_bytes: Some(u32::MAX),
        ram_static_bytes: Some(u32::MAX),
        ram_free_bytes: Some(u32::MAX),
        stack_free_min_bytes: Some(u32::MAX),
        busy_percent: Some(u8::MAX),
        compose_ms: Some(u16::MAX),
        flush_ms: Some(u16::MAX),
        fps: Some(u8::MAX),
        chip_model: Some(ChipModel::Esp32C3),
        chip_revision: Some((u8::MAX, u8::MAX)),
        temperature_decicelsius: Some(i16::MIN),
        uptime_ms: Some(u32::MAX),
        reset_reason: Some(ResetReason::Brownout),
        edge_queue_high_water: Some(u16::MAX),
        dropped_edges: Some(u32::MAX),
        invalid_transitions: Some(u32::MAX),
        frames_in: Some(u32::MAX),
        frames_out: Some(u32::MAX),
        frames_rejected: Some(u32::MAX),
    };
    let wire_len = |message: &Message| {
        let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
        encode_message(
            message,
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            u16::MAX,
            &mut wire,
        )
        .expect("encode");
        wire.len()
    };
    let control = [
        Message::Internals(InternalsReport {
            session: u32::MAX,
            internals: full,
        }),
        Message::Pong(Pong {
            t_ms_echo: u32::MAX,
            uptime_ms: u32::MAX,
        }),
        Message::Health(Health {
            free_bytes: u32::MAX,
        }),
        Message::HelloAck(HelloAck {
            device_caps: Capabilities::from_bits(u32::MAX),
            device_id: [0xFF; 16],
            firmware_version: FirmwareVersion {
                major: u16::MAX,
                minor: u16::MAX,
                patch: u16::MAX,
            },
            nonce_echo: u32::MAX,
        }),
        Message::BuildInfo(BuildInfo {
            session: u32::MAX,
            source_id: Some([0xFF; 6]),
            dirty: true,
        }),
        Message::Identity(Identity {
            session: u32::MAX,
            product: u16::MAX,
            board: u16::MAX,
            hw_rev: Some(u8::MAX),
            source: kivori_protocol::IdentitySource::Chip,
        }),
    ];
    let largest = control.iter().map(wire_len).max().unwrap();
    // Room for the largest control frame with a heartbeat reply and a health report behind it.
    let behind = wire_len(&control[1]) + wire_len(&control[2]);
    assert!(
        largest + behind <= kivori_firmware::proto::CONTROL_RESERVE_BYTES,
        "largest control frame {largest} B, plus {behind} B"
    );
}

#[test]
fn the_queue_reports_the_room_left_after_moving_what_the_fifo_takes() {
    let mut tx = TxBuffered::new(SimPipe::new());
    assert_eq!(tx.tx_free(), TX_QUEUE_BYTES);
    // A stalled host leaves one FIFO's worth in the pipe and the rest queued.
    tx.inner_mut().set_host_stalled(true);
    assert_eq!(tx.write(&[1; 200]), Ok(200));
    assert_eq!(tx.tx_free(), TX_QUEUE_BYTES - (200 - FIFO_BYTES));
    // Once the host reads again the queue drains by itself.
    tx.inner_mut().set_host_stalled(false);
    assert_eq!(tx.tx_free(), TX_QUEUE_BYTES);
}
