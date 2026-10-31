//! The device-side protocol dispatcher (docs/architecture.md, wire protocol; FR-002).
//!
//! Reads framed messages off a [`Transport`], applies the sequence policy, answers the handshake and
//! heartbeat, applies `SetState` to the [`DeviceState`], and answers each with a `StateReport`. Malformed
//! frames are dropped without side effects and never panic (SC-008).

use crate::health::{build_pong, diagnostic_for_sequence, DeviceDiagnostic, RejectReason};
use crate::identity::{BuildId, ProductIdentity};
use crate::ports::Transport;
use crate::state::{DeviceEvent, DeviceState};
use heapless::{Deque, Vec};
use kivori_model::input::Direction;
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, BuddySettings, BuildInfo, ByeReason, ControlId,
    ControlLabelsUpdate, DeviceId, Feedback, FirmwareVersion, HelloAck, Identity, InputEvent,
    InputKind, MascotActionApplied, MediaInfoUpdate, Message, MicStateUpdate, Nonce,
    PlayMascotAction, Presentation, ProtoError, SeqClass, SequenceTracker, StateReport, Status,
    MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

/// Bounded storage for one inbound packet awaiting its delimiter.
pub const RX_CAPACITY: usize = MAX_WIRE;
/// Maximum inbound bytes processed per tick, leaving time for input, rendering and recovery.
pub const RX_BYTES_PER_POLL: usize = 512;

/// Outbound bytes kept free for everything that is not input (`Pong`, `Health`, `StateReport`,
/// `Internals`, a `HelloAck`), so a queue that input has filled up to its share still takes the next
/// heartbeat reply. The largest such frame is well under half of it (tested).
pub const CONTROL_RESERVE_BYTES: usize = 256;
/// Input that may wait for outbound space at once: rotation runs and button events together.
const PENDING_INPUT_ITEMS: usize = 8;

/// Input the outbound queue had no room for yet, kept in the order it happened.
#[derive(Debug, Clone, Copy)]
enum PendingInput {
    /// Rotation, coalesced: the detents not sent yet as a net count (the protocol has one step per
    /// `Detent`, so the net is sent as that many single steps once there is room).
    Rotation {
        /// The gesture the steps go out under.
        gesture_id: u16,
        /// `GestureStarted` has not been sent yet.
        announce: bool,
        /// Clockwise steps minus counter-clockwise steps not sent yet.
        net: i16,
        /// `GestureEnded` is due once the steps are out.
        ended: bool,
        /// Device time of the newest event folded in.
        device_ms: u32,
    },
    /// One press, hold or double press of a switch.
    Button {
        control: ControlId,
        gesture_id: u16,
        kind: InputKind,
        device_ms: u32,
    },
}

/// What a `Bye` told the device the host is doing (needs `HOST_TAKEOVERS_V1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostTakeover {
    /// `Bye(HostSleeping)`: the host sleeps or locks.
    Sleeping,
    /// `Bye(FirmwareUpdate)`: the host is about to flash this device.
    Updating,
}

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
    /// The outbound message could not be encoded.
    Encode(ProtoError),
    /// The transport did not accept the complete wire frame.
    IncompleteWrite {
        /// Bytes accepted before the transport stopped.
        written: usize,
        /// Bytes required for the complete frame.
        expected: usize,
    },
}

/// The device protocol dispatcher.
pub struct Dispatcher {
    rx: Vec<u8, RX_CAPACITY>,
    /// An oversized packet owns the stream until its delimiter; never decode its tail.
    discarding_rx: bool,
    tracker: SequenceTracker,
    tx_seq: u16,
    identity: DeviceIdentity,
    /// What the `Identity` message reports (`PRODUCT_IDENTITY_V1`): the build's until the board
    /// adapter says otherwise.
    product: ProductIdentity,
    version: ProtocolVersion,
    /// One pending safe diagnostic, so the production runtime can report *that* a frame was rejected or a
    /// gap was seen without the dispatcher itself owning a transmit policy. Deliberately a single slot:
    /// a burst of garbage produces one report, not a flood.
    pending: Option<DeviceDiagnostic>,
    /// Total frames the decoder rejected this session (observability for tests and the run loop).
    rejected: u32,
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
    /// The latest negotiated `Status`, awaiting [`Self::take_status`].
    pending_status: Option<Status>,
    /// The latest negotiated `Feedback`, awaiting [`Self::take_feedback`]. Newest wins.
    pending_feedback: Option<Feedback>,
    /// The latest negotiated `MediaInfo`, awaiting [`Self::take_media_info`].
    pending_media_info: Option<MediaInfoUpdate>,
    pending_controls: Option<ControlLabelsUpdate>,
    /// The latest accepted `BuddySettings`, awaiting [`Self::take_buddy_settings`].
    pending_buddy: Option<BuddySettings>,
    /// The latest accepted `MicState`, awaiting [`Self::take_mic_state`].
    pending_mic: Option<MicStateUpdate>,
    /// The latest takeover announced by a negotiated `Bye`, awaiting [`Self::take_host_takeover`].
    pending_takeover: Option<HostTakeover>,
    /// Frames decoded since boot (wrapping): the runtime watches it move to notice a silent host.
    inbound_frames: u32,
    /// Frames fully written to the transport since boot (wrapping).
    outbound_frames: u32,
    /// The session of a negotiated `InternalsRequest`, awaiting [`Self::take_internals_request`].
    pending_internals: Option<Nonce>,
    /// Input waiting for outbound space (the host stopped reading for a while). Always empty
    /// outside a stall, and emptied at every session boundary: it never outlives its session.
    pending_input: Deque<PendingInput, PENDING_INPUT_ITEMS>,
}

impl Dispatcher {
    /// Creates a dispatcher advertising `identity` at the current protocol version.
    #[must_use]
    pub fn new(identity: DeviceIdentity) -> Self {
        Self {
            rx: Vec::new(),
            discarding_rx: false,
            tracker: SequenceTracker::new(),
            tx_seq: 0,
            identity,
            product: ProductIdentity::BUILD,
            version: ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            pending: None,
            rejected: 0,
            pending_action: None,
            negotiated_caps: Capabilities::NONE,
            hello_caps: None,
            accepted_session: None,
            session_ended: false,
            pending_presentation: None,
            pending_status: None,
            pending_feedback: None,
            pending_media_info: None,
            pending_controls: None,
            pending_buddy: None,
            pending_mic: None,
            pending_takeover: None,
            inbound_frames: 0,
            outbound_frames: 0,
            pending_internals: None,
            pending_input: Deque::new(),
        }
    }

    /// Sets the identity the device reports right after a handshake that negotiated
    /// `PRODUCT_IDENTITY_V1` (the board adapter passes the chip's record result).
    pub fn set_product_identity(&mut self, product: ProductIdentity) {
        self.product = product;
    }

    /// Narrows the advertised capabilities to `keep`. A device that runs no product (wrong board)
    /// offers only what lets the app recognise and reflash it.
    pub fn restrict_capabilities(&mut self, keep: Capabilities) {
        self.identity.capabilities = self.identity.capabilities.intersection(keep);
    }

    /// Frames decoded since boot (wrapping). It moves whenever the host sends anything valid, so
    /// a run loop that sees it stand still knows the host has gone quiet.
    #[must_use]
    pub const fn inbound_frames(&self) -> u32 {
        self.inbound_frames
    }

    /// Frames written to the transport since boot (wrapping).
    #[must_use]
    pub const fn outbound_frames(&self) -> u32 {
        self.outbound_frames
    }

    /// Takes the session of a pending `InternalsRequest`, if one was accepted.
    pub fn take_internals_request(&mut self) -> Option<Nonce> {
        self.pending_internals.take()
    }

    /// Takes the pending host takeover announced by a `Bye`, if any.
    pub fn take_host_takeover(&mut self) -> Option<HostTakeover> {
        self.pending_takeover.take()
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

    /// Emits one rotary `InputEvent` (`GestureStarted`, `Detent` or `GestureEnded`). Returns `false`
    /// when the capability is not negotiated or no session is accepted — an unnegotiated capability
    /// MUST stay inert.
    ///
    /// When the outbound queue has no room (the host stopped reading), the event is kept instead of
    /// ending the session: detents wait as a bounded net count and go out, in order, as room frees
    /// (see [`Self::flush_input`]). The session ends only on a transport failure.
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
            || self.accepted_session.is_none()
        {
            return false;
        }
        debug_assert!(matches!(
            kind,
            InputKind::GestureStarted | InputKind::Detent(_) | InputKind::GestureEnded
        ));
        // Rotation folds into the run still waiting at the tail of the queue; otherwise it starts one.
        let step = match kind {
            InputKind::Detent(Direction::Cw) => 1,
            InputKind::Detent(Direction::Ccw) => -1,
            _ => 0,
        };
        let folded = if let Some(PendingInput::Rotation {
            net,
            ended,
            device_ms: at,
            ..
        }) = self.pending_input.back_mut()
        {
            *net = net.saturating_add(step);
            match kind {
                InputKind::GestureStarted => *ended = false,
                InputKind::GestureEnded => *ended = true,
                _ => {}
            }
            *at = device_ms;
            true
        } else {
            false
        };
        let queued = folded
            || self
                .pending_input
                .push_back(PendingInput::Rotation {
                    gesture_id,
                    announce: kind == InputKind::GestureStarted,
                    net: step,
                    ended: kind == InputKind::GestureEnded,
                    device_ms,
                })
                .is_ok();
        self.settle_input(transport, queued)
    }

    /// Emits one push-switch `InputEvent` (`Press` or `Hold`) for the accepted session.
    ///
    /// Inert (returns `false`, writes nothing) unless both `PHYSICAL_INPUT_V1` and
    /// `BUTTON_INPUT_V1` were negotiated and a session is accepted. A press with no session is
    /// simply not sent (no stale replay); one that finds the outbound queue short of room waits
    /// its turn behind earlier input, never lost or reordered, and ends the session only if that
    /// wait outgrows its small bound.
    pub fn send_button_event<T: Transport>(
        &mut self,
        transport: &mut T,
        control: ControlId,
        gesture_id: u16,
        kind: InputKind,
        device_ms: u32,
    ) -> bool {
        let control_cap = match control {
            ControlId::ContextButton(_) => Capabilities::CONTEXT_BUTTONS_V1,
            ControlId::Button | ControlId::Rotary => Capabilities::BUTTON_INPUT_V1,
        };
        let needed = Capabilities::PHYSICAL_INPUT_V1.union(control_cap);
        if !self.negotiated_caps.contains(needed) || self.accepted_session.is_none() {
            return false;
        }
        let queued = self
            .pending_input
            .push_back(PendingInput::Button {
                control,
                gesture_id,
                kind,
                device_ms,
            })
            .is_ok();
        self.settle_input(transport, queued)
    }

    /// Sends what the queue's room allows after an event was queued (`queued`), or ends the session
    /// when it could not even be queued.
    fn settle_input<T: Transport>(&mut self, transport: &mut T, queued: bool) -> bool {
        if queued {
            self.flush_input(transport);
        } else {
            self.link_lost();
        }
        queued && self.accepted_session.is_some()
    }

    /// Sends queued input, oldest first, as far as the outbound queue's room allows. Input never
    /// takes the last [`CONTROL_RESERVE_BYTES`], so heartbeat replies and other control frames always
    /// find room. The run loop calls it at every service point; without queued input it does nothing.
    /// A transport failure ends the session like any other write failure.
    pub fn flush_input<T: Transport>(&mut self, transport: &mut T) {
        let Some(session) = self.accepted_session else {
            self.pending_input.clear();
            return;
        };
        while let Some(item) = self.pending_input.front().copied() {
            match self.emit_input(transport, session, item) {
                Ok(None) => {
                    let _ = self.pending_input.pop_front();
                }
                Ok(Some(rest)) => {
                    if let Some(front) = self.pending_input.front_mut() {
                        *front = rest;
                    }
                    return;
                }
                Err(_) => {
                    self.link_lost();
                    return;
                }
            }
        }
    }

    /// Writes as much of `item` as there is room for; what is left over, if any.
    fn emit_input<T: Transport>(
        &mut self,
        transport: &mut T,
        session: Nonce,
        item: PendingInput,
    ) -> Result<Option<PendingInput>, DispatchError<T::Error>> {
        let mut send = |this: &mut Self, gesture_id, control, kind, device_ms| {
            let message = Message::InputEvent(InputEvent {
                session,
                gesture_id,
                control,
                kind,
                device_ms,
            });
            this.send_within(transport, &message, CONTROL_RESERVE_BYTES)
        };
        match item {
            PendingInput::Button {
                control,
                gesture_id,
                kind,
                device_ms,
            } => Ok(if send(self, gesture_id, control, kind, device_ms)? {
                None
            } else {
                Some(item)
            }),
            PendingInput::Rotation {
                gesture_id,
                mut announce,
                mut net,
                ended,
                device_ms,
            } => {
                // A gesture that opened and closed with no net motion and was never announced says nothing.
                if announce && ended && net == 0 {
                    return Ok(None);
                }
                let rotary = ControlId::Rotary;
                if announce {
                    if !send(
                        self,
                        gesture_id,
                        rotary,
                        InputKind::GestureStarted,
                        device_ms,
                    )? {
                        return Ok(Some(item));
                    }
                    announce = false;
                }
                while net != 0 {
                    let (direction, step) = if net > 0 {
                        (Direction::Cw, -1)
                    } else {
                        (Direction::Ccw, 1)
                    };
                    if !send(
                        self,
                        gesture_id,
                        rotary,
                        InputKind::Detent(direction),
                        device_ms,
                    )? {
                        return Ok(Some(PendingInput::Rotation {
                            gesture_id,
                            announce,
                            net,
                            ended,
                            device_ms,
                        }));
                    }
                    net += step;
                }
                if ended && !send(self, gesture_id, rotary, InputKind::GestureEnded, device_ms)? {
                    return Ok(Some(PendingInput::Rotation {
                        gesture_id,
                        announce,
                        net,
                        ended,
                        device_ms,
                    }));
                }
                Ok(None)
            }
        }
    }

    /// Takes the pending accepted `Status`, if any.
    pub fn take_status(&mut self) -> Option<Status> {
        self.pending_status.take()
    }

    /// Takes the pending accepted `MediaInfo`, if any.
    pub fn take_media_info(&mut self) -> Option<MediaInfoUpdate> {
        self.pending_media_info.take()
    }

    /// Takes the pending accepted `ControlLabels`, if any.
    pub fn take_control_labels(&mut self) -> Option<ControlLabelsUpdate> {
        self.pending_controls.take()
    }

    /// Takes the pending accepted `MicState`, if any.
    pub fn take_mic_state(&mut self) -> Option<MicStateUpdate> {
        self.pending_mic.take()
    }

    /// Takes the pending accepted `BuddySettings`, if any.
    pub fn take_buddy_settings(&mut self) -> Option<BuddySettings> {
        self.pending_buddy.take()
    }

    /// Whether the session negotiated double-press detection.
    #[must_use]
    pub const fn double_press_enabled(&self) -> bool {
        self.negotiated_caps.contains(Capabilities::DOUBLE_PRESS_V1)
    }

    /// Takes the pending accepted `Feedback`, if any.
    pub fn take_feedback(&mut self) -> Option<Feedback> {
        self.pending_feedback.take()
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
        self.rx.clear();
        self.discarding_rx = false;
        self.tracker = SequenceTracker::new();
        self.accepted_session = None;
        self.negotiated_caps = Capabilities::NONE;
        self.hello_caps = None;
        self.pending_action = None;
        self.pending_input.clear();
        self.session_ended = true;
    }

    /// Encodes and writes a device-originated message (`Health`, `Diagnostic`) on the session's sequence.
    ///
    /// # Errors
    /// Returns a [`DispatchError`] if encoding or complete frame delivery fails.
    pub fn emit<T: Transport>(
        &mut self,
        transport: &mut T,
        message: &Message,
    ) -> Result<(), DispatchError<T::Error>> {
        self.send(transport, message)
    }

    /// Processes at most [`RX_BYTES_PER_POLL`] inbound bytes, handling frames as their delimiters
    /// arrive. Partial packets and unread transport bytes continue on the next tick. `now_ms` is
    /// the device uptime (used for `Pong` / `StateReport`).
    ///
    /// # Errors
    /// Returns a [`DispatchError`] if reading, encoding a reply, or complete reply delivery fails.
    pub fn poll<T: Transport>(
        &mut self,
        transport: &mut T,
        device: &mut DeviceState,
        now_ms: u32,
    ) -> Result<(), DispatchError<T::Error>> {
        let mut remaining = RX_BYTES_PER_POLL;
        let mut chunk = [0u8; 128];
        while remaining > 0 {
            let limit = remaining.min(chunk.len());
            let n = transport
                .read(&mut chunk[..limit])
                .map_err(DispatchError::Transport)?;
            if n == 0 {
                break;
            }
            remaining -= n;
            for &b in &chunk[..n] {
                if b == 0 {
                    if !self.rx.is_empty() {
                        let packet = core::mem::take(&mut self.rx);
                        self.handle_packet(&packet, transport, device, now_ms)?;
                    }
                    self.discarding_rx = false;
                } else if !self.discarding_rx && self.rx.push(b).is_err() {
                    self.rx.clear();
                    self.discarding_rx = true;
                    self.rejected = self.rejected.saturating_add(1);
                    self.pending = Some(DeviceDiagnostic::FrameRejected(RejectReason::Framing));
                }
            }
        }
        Ok(())
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
        self.inbound_frames = self.inbound_frames.wrapping_add(1);
        // A different nonce is a new session even when a crashed desktop restarts its sequence at
        // zero before the old session sent Bye. Same-nonce retransmits keep duplicate suppression.
        if let Message::Hello(hello) = message {
            if self.accepted_session != Some(hello.nonce) {
                self.tracker = SequenceTracker::new();
            }
        }
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
                // Whatever is still queued was meant for a session that is over (or for no one).
                transport.discard_unsent();
                self.pending_input.clear();
                let ack = HelloAck {
                    device_caps: self.identity.capabilities,
                    device_id: self.identity.device_id,
                    firmware_version: self.identity.firmware_version,
                    nonce_echo: hello.nonce,
                };
                self.send(transport, &Message::HelloAck(ack))?;
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
                // The identity rides right behind the handshake, once per session, and only to a
                // host that negotiated it (a legacy host never sees an unknown tag).
                if let (true, Some(session)) = (
                    self.negotiated_caps
                        .contains(Capabilities::PRODUCT_IDENTITY_V1),
                    self.accepted_session,
                ) {
                    let product = self.product;
                    self.send(
                        transport,
                        &Message::Identity(Identity {
                            session,
                            product: product.product,
                            board: product.board,
                            hw_rev: product.hw_rev,
                            source: product.source,
                        }),
                    )?;
                }
                // Which exact build this is, right behind the identity and under the same rule: only
                // to a host that negotiated it, so an older desktop never sees the unknown tag.
                if let (true, Some(session)) = (
                    self.negotiated_caps.contains(Capabilities::BUILD_INFO_V1),
                    self.accepted_session,
                ) {
                    let build = BuildId::BUILD;
                    self.send(
                        transport,
                        &Message::BuildInfo(BuildInfo {
                            session,
                            source_id: build.source_id,
                            dirty: build.dirty,
                        }),
                    )?;
                }
            }
            // Every accepted `SetState` is answered with the state now shown, changed or not: a
            // desktop reconnecting to a device already in the desired state must still learn it
            // (row 1.16). Duplicate sequence numbers never reach this arm.
            Message::SetState(set) => {
                let _changed = device.apply(DeviceEvent::SetState(set.desired));
                let report = StateReport {
                    reported: device.current(),
                    elapsed_ms: now_ms,
                };
                self.send(transport, &Message::StateReport(report))?;
            }
            Message::Ping(ping) if self.accepted_session.is_some() => {
                self.send(transport, &Message::Pong(build_pong(ping.t_ms, now_ms)))?;
            }
            // Acknowledge only a reaction that will actually play (docs/product.md, display and buddy): over Busy, Booting or
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
            // An unnegotiated capability MUST stay completely inert: drop it exactly like any
            // message kind this device does not act on (the `_` arm below) — no diagnostic,
            // no render.
            Message::Presentation(presentation)
                if self.negotiated_caps.contains(Capabilities::PRESENTATION_V1) =>
            {
                self.pending_presentation = Some(presentation);
            }
            // Same inertness rule as `Presentation`: unnegotiated means dropped, unseen.
            Message::Status(status)
                if self.negotiated_caps.contains(Capabilities::DESK_STATUS_V1) =>
            {
                self.pending_status = Some(status);
            }
            Message::MediaInfo(update)
                if self.negotiated_caps.contains(Capabilities::MEDIA_INFO_V1) =>
            {
                self.pending_media_info = Some(update);
            }
            Message::ControlLabels(update)
                if self
                    .negotiated_caps
                    .contains(Capabilities::CONTROL_LABELS_V1) =>
            {
                self.pending_controls = Some(update);
            }
            // Settings only count from the accepted session, with the capability negotiated.
            Message::BuddySettings(settings)
                if self.negotiated_caps.contains(Capabilities::IDLE_LIFE_V1)
                    && self.accepted_session == Some(settings.session) =>
            {
                self.pending_buddy = Some(settings);
            }
            // Same inertness rule as `Status`: unnegotiated means dropped, unseen.
            Message::MicState(update)
                if self.negotiated_caps.contains(Capabilities::MIC_STATE_V1) =>
            {
                self.pending_mic = Some(update);
            }
            // Only the accepted session may ask, and only with the capability negotiated.
            Message::InternalsRequest(request)
                if self
                    .negotiated_caps
                    .contains(Capabilities::DEVICE_INTERNALS_V1)
                    && self.accepted_session == Some(request.session) =>
            {
                self.pending_internals = Some(request.session);
            }
            Message::Feedback(feedback)
                if self
                    .negotiated_caps
                    .contains(Capabilities::ACTION_FEEDBACK_V1) =>
            {
                self.pending_feedback = Some(feedback);
            }
            Message::Bye(bye) => {
                // A takeover reason only counts when it was negotiated (an unnegotiated capability
                // stays inert): the link then drops as for any `Bye`, but the device keeps
                // sleeping (latched in `DeviceState`) or the runtime shows Updating.
                let takeover = if self
                    .negotiated_caps
                    .contains(Capabilities::HOST_TAKEOVERS_V1)
                {
                    match bye.reason {
                        ByeReason::HostSleeping => Some(HostTakeover::Sleeping),
                        ByeReason::FirmwareUpdate => Some(HostTakeover::Updating),
                        _ => None,
                    }
                } else {
                    None
                };
                if takeover == Some(HostTakeover::Sleeping) {
                    let _ = device.apply(DeviceEvent::HostSleep);
                }
                self.pending_takeover = takeover;
                // Session closed: drop the link and reset sequence tracking for the next session.
                let _ = device.apply(DeviceEvent::LinkDown);
                self.tracker = SequenceTracker::new();
                self.pending = Some(DeviceDiagnostic::LinkLost);
                self.pending_action = None;
                self.pending_input.clear();
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
        self.send_within(transport, message, 0).map(|_| ())
    }

    /// Like [`Self::send`], but writes nothing (`Ok(false)`) unless the transport has room for the
    /// whole packet and `reserve` more bytes besides.
    fn send_within<T: Transport>(
        &mut self,
        transport: &mut T,
        message: &Message,
        reserve: usize,
    ) -> Result<bool, DispatchError<T::Error>> {
        let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
        encode_message(message, self.version, self.tx_seq, &mut wire)
            .map_err(DispatchError::Encode)?;
        if reserve > 0 && transport.tx_free() < wire.len().saturating_add(reserve) {
            return Ok(false);
        }
        let mut written = 0;
        while written < wire.len() {
            let remaining = wire.len() - written;
            let n = match transport.write(&wire[written..]) {
                Ok(n) => n,
                Err(error) => {
                    transport.discard_unsent();
                    return Err(DispatchError::Transport(error));
                }
            };
            if n == 0 || n > remaining {
                transport.discard_unsent();
                return Err(DispatchError::IncompleteWrite {
                    written: written.saturating_add(n),
                    expected: wire.len(),
                });
            }
            written += n;
        }
        self.tx_seq = self.tx_seq.wrapping_add(1);
        self.outbound_frames = self.outbound_frames.wrapping_add(1);
        Ok(true)
    }
}
