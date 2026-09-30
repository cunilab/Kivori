//! The device-side protocol dispatcher (contracts/protocol.md §4/§6; FR-002).
//!
//! Reads framed messages off a [`Transport`], applies the sequence policy, answers the handshake and
//! heartbeat, applies `SetState` to the [`DeviceState`], and emits `StateReport` on change. Malformed
//! frames are dropped without side effects and never panic (SC-008).

use crate::health::{build_pong, diagnostic_for_sequence, DeviceDiagnostic, RejectReason};
use crate::ports::Transport;
use crate::state::{DeviceEvent, DeviceState};
use heapless::Vec;
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, ControlId, DeviceId, FirmwareVersion, HelloAck, InputEvent,
    InputKind, MascotActionApplied, Message, Nonce, PlayMascotAction, Presentation, SeqClass,
    SequenceTracker, StateReport, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

/// Inbound accumulation capacity: room for a partial packet plus one full wire packet.
pub const RX_CAPACITY: usize = MAX_WIRE * 2;

/// The device's advertised identity, versions, and capabilities.
#[derive(Debug, Clone, Copy)]
pub struct DeviceIdentity {
    /// Opaque device identity (never logged raw).
    pub device_id: DeviceId,
    /// Firmware version.
    pub firmware_version: FirmwareVersion,
    /// Device capabilities.
    pub capabilities: Capabilities,
}

/// A dispatch failure. Protocol-level malformed input is handled internally (dropped), so only an
/// underlying transport error is surfaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError<E> {
    /// The transport read or write failed.
    Transport(E),
}

/// The device protocol dispatcher.
pub struct Dispatcher {
    rx: Vec<u8, RX_CAPACITY>,
    tracker: SequenceTracker,
    tx_seq: u16,
    identity: DeviceIdentity,
    version: ProtocolVersion,
    /// One pending safe diagnostic, so the production runtime can report *that* a frame was rejected or a
    /// gap was seen without the dispatcher itself owning a transmit policy. Deliberately a single slot:
    /// a burst of garbage produces one report, not a flood.
    pending: Option<DeviceDiagnostic>,
    /// Total frames the decoder rejected this session (observability for tests and the run loop).
    rejected: u32,
    /// Replies actually transmitted, so the run loop can report protocol progress without re-parsing its
    /// own output. Cumulative and monotonic.
    hello_acks: u32,
    pongs: u32,
    state_reports: u32,
    pending_action: Option<PlayMascotAction>,
    /// The capability set negotiated for the current session (set from `Ready`; the protocol
    /// crate itself is capability-blind, so this is the dispatcher's own gate).
    negotiated_caps: Capabilities,
    hello_caps: Option<Capabilities>,
    /// The session nonce accepted at the last `Hello`, or `None` before any handshake / after
    /// `Bye`/link loss. The only session an emitted `InputEvent` may claim.
    accepted_session: Option<Nonce>,
    /// Set when a session boundary just occurred — `Bye`, a transport failure ([`Self::link_lost`]),
    /// or a new `Hello` (a reconnect that never sent `Bye`) — so the caller can reset any input
    /// state (the decoder/gesture layers) that must not survive it. Single-slot, like `pending`:
    /// consumed once via [`Self::take_session_ended`].
    session_ended: bool,
    /// One pending accepted `Presentation`, until [`Self::take_presentation`] drains it (the
    /// runtime does so every tick, so nothing here is ever silently overwritten unseen). Only ever
    /// set when `PRESENTATION_V1` is negotiated — an unnegotiated capability leaves this field
    /// permanently empty, never merely unread.
    pending_presentation: Option<Presentation>,
}

impl Dispatcher {
    /// Creates a dispatcher advertising `identity` at the current protocol version.
    #[must_use]
    pub fn new(identity: DeviceIdentity) -> Self {
        Self {
            rx: Vec::new(),
            tracker: SequenceTracker::new(),
            tx_seq: 0,
            identity,
            version: ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            pending: None,
            rejected: 0,
            hello_acks: 0,
            pongs: 0,
            state_reports: 0,
            pending_action: None,
            negotiated_caps: Capabilities::NONE,
            hello_caps: None,
            accepted_session: None,
            session_ended: false,
            pending_presentation: None,
        }
    }

    /// `HelloAck` replies transmitted this session.
    #[must_use]
    pub const fn hello_acks(&self) -> u32 {
        self.hello_acks
    }

    /// `Pong` replies transmitted this session.
    #[must_use]
    pub const fn pongs(&self) -> u32 {
        self.pongs
    }

    /// `StateReport` messages transmitted this session.
    #[must_use]
    pub const fn state_reports(&self) -> u32 {
        self.state_reports
    }

    /// Number of frames the decoder has rejected since this dispatcher was created.
    #[must_use]
    pub const fn rejected_frames(&self) -> u32 {
        self.rejected
    }

    /// The session nonce accepted at the last `Hello`, or `None` before any handshake, after
    /// `Bye`, or after a link loss.
    #[must_use]
    pub const fn accepted_session(&self) -> Option<Nonce> {
        self.accepted_session
    }

    /// Encode one input event. Returns `false` when the capability is not negotiated
    /// or no session is accepted — an unnegotiated capability MUST stay inert.
    pub fn send_input_event<T: Transport>(
        &mut self,
        transport: &mut T,
        gesture_id: u16,
        kind: InputKind,
        device_ms: u32,
    ) -> bool {
        if !self
            .negotiated_caps
            .contains(Capabilities::PHYSICAL_INPUT_V1)
        {
            return false;
        }
        let Some(session) = self.accepted_session else {
            return false;
        };
        let msg = Message::InputEvent(InputEvent {
            session,
            gesture_id,
            control: ControlId::Rotary,
            kind,
            device_ms,
        });
        self.send(transport, &msg).is_ok()
    }

    /// Takes the pending accepted `Presentation`, if any (see [`Self::pending_presentation`]).
    pub fn take_presentation(&mut self) -> Option<Presentation> {
        self.pending_presentation.take()
    }

    /// Takes the pending safe diagnostic, if any. The caller decides whether to transmit it.
    pub fn take_diagnostic(&mut self) -> Option<DeviceDiagnostic> {
        self.pending.take()
    }

    /// Takes the latest accepted social action for the renderer. Actions never queue.
    pub fn take_mascot_action(&mut self) -> Option<PlayMascotAction> {
        self.pending_action.take()
    }

    /// Records an allowlisted diagnostic for the caller to pick up (e.g. a display fault the run loop saw).
    pub fn note_diagnostic(&mut self, diagnostic: DeviceDiagnostic) {
        self.pending = Some(diagnostic);
    }

    /// Takes the "a session boundary just occurred" flag, if set (`Bye`, link loss, or a new
    /// `Hello`), so the caller can reset session-scoped input state exactly once per boundary.
    pub fn take_session_ended(&mut self) -> bool {
        core::mem::take(&mut self.session_ended)
    }

    /// Clears session state after a transport failure, exactly like `Bye` minus the diagnostic
    /// (the caller already reports its own `LinkLost`/failure diagnostic for a transport error).
    ///
    /// Without this, a link drop that is never followed by an explicit `Bye` (e.g. a desktop
    /// crash or a yanked cable) would leave `accepted_session`/`negotiated_caps` — and therefore
    /// the caller's decoder/gesture state — exactly as they were, letting a gesture opened before
    /// the drop keep emitting under the stale session once the link recovers.
    pub fn link_lost(&mut self) {
        self.tracker = SequenceTracker::new();
        self.accepted_session = None;
        self.negotiated_caps = Capabilities::NONE;
        self.hello_caps = None;
        self.pending_action = None;
        self.session_ended = true;
    }

    /// Encodes and writes a device-originated message (`Health`, `Diagnostic`) on the session's sequence.
    ///
    /// # Errors
    /// [`DispatchError::Transport`] if the transport write fails.
    pub fn emit<T: Transport>(
        &mut self,
        transport: &mut T,
        message: &Message,
    ) -> Result<(), DispatchError<T::Error>> {
        self.send(transport, message)
    }

    /// Drains all currently-available inbound bytes, handling each complete frame and writing any
    /// responses. `now_ms` is the device uptime (used for `Pong` / `StateReport`).
    ///
    /// # Errors
    /// [`DispatchError::Transport`] if the transport read/write fails.
    pub fn poll<T: Transport>(
        &mut self,
        transport: &mut T,
        device: &mut DeviceState,
        now_ms: u32,
    ) -> Result<(), DispatchError<T::Error>> {
        self.fill_rx(transport)?;
        while let Some(pos) = self.rx.iter().position(|&b| b == 0) {
            if pos > 0 && pos <= MAX_WIRE {
                // Copy the packet out (without the delimiter) so the borrow of `self.rx` ends before
                // we drop the consumed bytes and handle the message.
                let mut packet: Vec<u8, MAX_WIRE> = Vec::new();
                let _ = packet.extend_from_slice(&self.rx[..pos]);
                self.consume_rx(pos + 1);
                self.handle_packet(&packet, transport, device, now_ms)?;
            } else {
                // Empty (consecutive delimiter) or oversized packet: skip it and resync.
                self.consume_rx(pos + 1);
            }
        }
        Ok(())
    }

    /// Reads all available bytes into the accumulation buffer. On overflow the buffer is reset (a
    /// frame that large is unrecoverable garbage; we resync on the next delimiter).
    fn fill_rx<T: Transport>(&mut self, transport: &mut T) -> Result<(), DispatchError<T::Error>> {
        let mut chunk = [0u8; 128];
        loop {
            let n = transport
                .read(&mut chunk)
                .map_err(DispatchError::Transport)?;
            if n == 0 {
                return Ok(());
            }
            for &b in &chunk[..n] {
                if self.rx.push(b).is_err() {
                    self.rx.clear();
                    let _ = self.rx.push(b);
                }
            }
        }
    }

    /// Removes the first `count` bytes from the accumulation buffer.
    fn consume_rx(&mut self, count: usize) {
        let count = count.min(self.rx.len());
        let mut tail: Vec<u8, RX_CAPACITY> = Vec::new();
        let _ = tail.extend_from_slice(&self.rx[count..]);
        self.rx = tail;
    }

    fn handle_packet<T: Transport>(
        &mut self,
        packet: &[u8],
        transport: &mut T,
        device: &mut DeviceState,
        now_ms: u32,
    ) -> Result<(), DispatchError<T::Error>> {
        let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
        let (header, message) = match decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
            Ok(decoded) => decoded,
            Err(error) => {
                // Malformed frame — drop it (SC-008). No side effects, no panic. The *class* of failure is
                // recorded as a safe diagnostic; the offending bytes are not retained anywhere.
                self.rejected = self.rejected.saturating_add(1);
                self.pending = Some(DeviceDiagnostic::FrameRejected(RejectReason::of(&error)));
                return Ok(());
            }
        };
        // Sequence policy (§6): a duplicate must not re-apply side effects.
        let class = self.tracker.classify(header.seq);
        if let Some(diagnostic) = diagnostic_for_sequence(class) {
            self.pending = Some(diagnostic);
        }
        if matches!(class, SeqClass::Duplicate) {
            return Ok(());
        }
        match message {
            Message::Hello(hello) => {
                self.negotiated_caps = Capabilities::NONE;
                self.hello_caps = Some(hello.desktop_caps);
                self.pending_action = None;
                let ack = HelloAck {
                    device_caps: self.identity.capabilities,
                    device_id: self.identity.device_id,
                    firmware_version: self.identity.firmware_version,
                    nonce_echo: hello.nonce,
                };
                self.send(transport, &Message::HelloAck(ack))?;
                self.hello_acks = self.hello_acks.saturating_add(1);
                // A new `Hello` always starts a fresh session, whether or not the previous one
                // ended with a `Bye` (a reconnect after a desktop crash never sends one). Flag the
                // boundary BEFORE recording the new nonce, so the caller resets input state (a
                // gesture from the old session must never complete in the new one) even when no
                // `Bye` was ever seen. The previous session's capabilities were already cleared
                // above; the `Ready` that follows re-establishes them.
                self.session_ended = true;
                self.accepted_session = Some(hello.nonce);
            }
            Message::Ready(ready) => {
                // Intersect with what the device itself advertised and what this session's `Hello`
                // offered: the desktop's `Ready` is untrusted input, and an unnegotiated (or
                // never-advertised) capability MUST NOT activate behavior even if a buggy or
                // hostile peer claims otherwise.
                if let Some(hello_caps) = self.hello_caps {
                    self.negotiated_caps = ready
                        .negotiated_caps
                        .intersection(self.identity.capabilities)
                        .intersection(hello_caps);
                }
            }
            Message::SetState(set) => {
                if let Some(now) = device.apply(DeviceEvent::SetState(set.desired)) {
                    let report = StateReport {
                        reported: now,
                        elapsed_ms: now_ms,
                    };
                    self.send(transport, &Message::StateReport(report))?;
                    self.state_reports = self.state_reports.saturating_add(1);
                }
            }
            Message::Ping(ping) => {
                self.send(transport, &Message::Pong(build_pong(ping.t_ms, now_ms)))?;
                self.pongs = self.pongs.saturating_add(1);
            }
            // Acknowledge only a reaction that will actually play (PRD §9.5): over Busy, Booting or
            // Offline the request is dropped unacknowledged, so the desktop never believes it ran.
            Message::PlayMascotAction(action)
                if self
                    .negotiated_caps
                    .contains(Capabilities::MASCOT_INTERACTION)
                    && device.current().allows_reaction() =>
            {
                self.pending_action = Some(action);
                self.send(
                    transport,
                    &Message::MascotActionApplied(MascotActionApplied {
                        action: action.action,
                        personality: action.personality,
                        seed: action.seed,
                        applied_at_ms: now_ms,
                    }),
                )?;
            }
            Message::Presentation(presentation) => {
                // An unnegotiated capability MUST stay completely inert: drop it exactly like any
                // message kind this device does not act on (the `_` arm below) — no diagnostic,
                // no render.
                if self.negotiated_caps.contains(Capabilities::PRESENTATION_V1) {
                    self.pending_presentation = Some(presentation);
                }
            }
            Message::Bye(_) => {
                // Session closed: drop the link and reset sequence tracking for the next session.
                let _ = device.apply(DeviceEvent::LinkDown);
                self.tracker = SequenceTracker::new();
                self.pending = Some(DeviceDiagnostic::LinkLost);
                self.pending_action = None;
                // A gesture (or partial motion) from the old session must never complete in a new
                // one, so the caller resets its input state too (invariant: no stale replay).
                self.accepted_session = None;
                self.negotiated_caps = Capabilities::NONE;
                self.hello_caps = None;
                self.session_ended = true;
            }
            // The device→desktop message kinds are not acted on by the device.
            _ => {}
        }
        Ok(())
    }

    /// Encodes `message` and writes the whole wire packet to the transport.
    fn send<T: Transport>(
        &mut self,
        transport: &mut T,
        message: &Message,
    ) -> Result<(), DispatchError<T::Error>> {
        let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
        // Encoding only fails on an over-capacity payload, which our fixed messages never hit.
        if encode_message(message, self.version, self.tx_seq, &mut wire).is_ok() {
            self.tx_seq = self.tx_seq.wrapping_add(1);
            let mut sent = 0;
            while sent < wire.len() {
                let n = transport
                    .write(&wire[sent..])
                    .map_err(DispatchError::Transport)?;
                if n == 0 {
                    break; // transport full; best-effort (the real adapter buffers; sim never fills)
                }
                sent += n;
            }
        }
        Ok(())
    }
}
