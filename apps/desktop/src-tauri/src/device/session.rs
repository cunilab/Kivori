//! The desktop protocol session driver (Phase 11 host-testable core).
//!
//! `Session` is the synchronous, [`SerialLink`]-driven heart of the eventual async run loop: it opens
//! a session (`Hello`), verifies the device's `HelloAck`, drives the [`ConnectionManager`] from wire
//! events, resynchronizes the orchestrator's desired state on connect (FR-009), and answers/emits the
//! heartbeat. It performs no async or timing itself — the caller owns the clock and the task — so it is
//! fully host-testable against an in-memory link (and, in the E2E harness, against the real firmware
//! dispatcher). Malformed inbound frames are dropped without panicking (SC-008).

use crate::activity::{
    category_for_nonce_error, ActivityEventKind, ActivityMetadata, SessionActivity,
};
use crate::device::connection::{build_hello, hash_device_id_short, summarize};
use crate::device::fsm::{ConnectionManager, ManagerEvent};
use crate::device::heartbeat::HeartbeatMonitor;
use crate::device::nonce::{NonceSource, OsNonceSource};
use crate::device::transport::SerialLink;
use crate::orchestrator::Orchestrator;
use kivori_model::desk::{ActionFeedback, ControlLabels, DeskStatus, MediaInfo};
use kivori_model::{
    Capabilities, CompanionState, MascotAction, MascotPersonality, ProtocolVersion, SendableState,
};
use kivori_protocol::{
    decode_frame, decode_message, encode_message, evaluate_hello_ack, Bye, ByeReason,
    ControlLabelsUpdate, Feedback, FirmwareVersion, HandshakeOutcome, Hello, InputEvent,
    MascotActionApplied, MediaInfoUpdate, Message, Ping, PlayMascotAction, Presentation,
    ProtoError, SeqClass, SequenceTracker, SetState, Status, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR,
    PROTOCOL_MINOR,
};

/// Static session parameters (the desktop's advertised identity + compatibility).
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// The desktop application version advertised in `Hello`.
    pub app_version: FirmwareVersion,
    /// The desktop's protocol version (carried in every frame header).
    pub protocol_version: ProtocolVersion,
    /// Capabilities the desktop advertises.
    pub capabilities: Capabilities,
    /// Protocol major versions the desktop supports.
    pub supported_majors: Vec<u16>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            // The release version, from the workspace `Cargo.toml` (one source of truth).
            app_version: FirmwareVersion {
                major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0),
                minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0),
                patch: env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0),
            },
            protocol_version: ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            // The desktop implements mascot interaction, the rotary input receive path, and the
            // presentation send path, so it advertises all three — negotiation (the intersection
            // with whatever the device itself advertises) is what actually gates behaviour.
            capabilities: Capabilities::MASCOT_INTERACTION
                .union(Capabilities::PHYSICAL_INPUT_V1)
                .union(Capabilities::PRESENTATION_V1)
                .union(Capabilities::BUTTON_INPUT_V1)
                .union(Capabilities::DESK_STATUS_V1)
                .union(Capabilities::ACTION_FEEDBACK_V1)
                .union(Capabilities::DOUBLE_PRESS_V1)
                .union(Capabilities::MEDIA_INFO_V1)
                .union(Capabilities::CONTROL_LABELS_V1)
                .union(Capabilities::CONTEXT_BUTTONS_V1),
            supported_majors: vec![PROTOCOL_MAJOR],
        }
    }
}

/// A session failure. Malformed inbound frames are dropped internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError<E> {
    /// The serial link read or write failed.
    Transport(E),
    /// The peer ended this session; the caller must close the serial handle.
    PeerClosed,
    /// A frame could not be completely written.
    WriteZero,
    /// A message could not be encoded within the framing bound.
    Encoding,
    /// The OS could not supply a fresh session nonce. This attempt is aborted (never a panic); the
    /// caller's existing backoff retries the connection.
    NonceUnavailable,
}

/// The desktop side of a device session.
pub struct Session {
    config: SessionConfig,
    tx_seq: u16,
    inbound: SequenceTracker,
    rx: Vec<u8>,
    discard_rx: bool,
    sent_hello: Option<Hello>,
    nonce_source: Box<dyn NonceSource>,
    /// The nonce of the currently established session, if any. Connection-scoped: `None` until a
    /// handshake is accepted, and cleared on every path out of `Connected` (including `Bye`) so a
    /// stale nonce can never be mistaken for a fresh one (no-stale-replay guarantee).
    current_session: Option<u32>,
    heartbeat: HeartbeatMonitor,
    reported: Option<CompanionState>,
    /// The capability set negotiated at the last accepted handshake. Connection-scoped, exactly
    /// like `current_session`.
    negotiated_caps: Capabilities,
    /// `InputEvent`s decoded by `pump()`, awaiting [`Session::take_input_events`].
    pending_inputs: Vec<InputEvent>,
    last_mascot_action_applied: Option<MascotActionApplied>,
    connection_generation: u32,
    activity: Vec<SessionActivity>,
}

impl Session {
    /// Creates a session with the given configuration (nothing is sent until [`Session::open`]).
    ///
    /// Uses [`OsNonceSource`] for session-nonce freshness. See [`Session::with_nonce_source`] to
    /// inject a deterministic source in tests.
    #[must_use]
    pub fn new(config: SessionConfig) -> Self {
        Self::with_nonce_source(config, Box::new(OsNonceSource))
    }

    /// Creates a session with an injected nonce source (tests only — production code should use
    /// [`Session::new`], which always uses [`OsNonceSource`]).
    #[must_use]
    pub fn with_nonce_source(config: SessionConfig, source: Box<dyn NonceSource>) -> Self {
        Self {
            config,
            tx_seq: 0,
            inbound: SequenceTracker::new(),
            rx: Vec::new(),
            discard_rx: false,
            sent_hello: None,
            nonce_source: source,
            current_session: None,
            heartbeat: HeartbeatMonitor::default(),
            reported: None,
            negotiated_caps: Capabilities::NONE,
            pending_inputs: Vec::new(),
            last_mascot_action_applied: None,
            connection_generation: 0,
            activity: Vec::new(),
        }
    }

    /// Opens a session on a freshly-connected port: marks the manager `Connecting` and sends `Hello`.
    ///
    /// # Errors
    /// [`SessionError::Transport`] if the write fails. [`SessionError::NonceUnavailable`] if the OS
    /// entropy source could not supply a fresh nonce for this attempt — the caller's existing
    /// backoff/retry handles recovery; this never panics.
    pub fn open<L: SerialLink>(
        &mut self,
        link: &mut L,
        manager: &mut ConnectionManager,
    ) -> Result<(), SessionError<L::Error>> {
        manager.apply(ManagerEvent::PortOpened);
        self.connection_generation = self.connection_generation.wrapping_add(1).max(1);
        self.rx.clear();
        self.inbound = SequenceTracker::new();
        self.heartbeat = HeartbeatMonitor::default();
        self.reported = None;
        self.clear_session_identity();
        self.pending_inputs.clear();
        self.last_mascot_action_applied = None;
        self.activity.clear();
        let nonce = match self.nonce_source.next_nonce() {
            Ok(nonce) => nonce,
            Err(error) => {
                // No session identity means no no-stale-replay guarantee, so refuse to open a
                // session rather than proceed without one. This aborts THIS attempt only.
                self.observe(
                    ActivityEventKind::SessionNonceUnavailable,
                    Some(ActivityMetadata::HostDiagnostic {
                        category: category_for_nonce_error(&error),
                    }),
                );
                return Err(SessionError::NonceUnavailable);
            }
        };
        let hello = build_hello(self.config.app_version, self.config.capabilities, nonce);
        self.sent_hello = Some(hello);
        self.send(link, &Message::Hello(hello))?;
        self.observe(ActivityEventKind::ConnectionOpened, None);
        self.observe(ActivityEventKind::HandshakeStarted, None);
        Ok(())
    }

    /// The device's most recently reported companion state (`None` until the first `StateReport`).
    #[must_use]
    pub fn reported(&self) -> Option<CompanionState> {
        self.reported
    }

    /// The nonce of the currently established session, if any.
    #[must_use]
    pub const fn current_session(&self) -> Option<u32> {
        self.current_session
    }

    /// The capability set negotiated at the last accepted handshake (`Capabilities::NONE` when no
    /// session is established).
    #[must_use]
    pub const fn negotiated_caps(&self) -> Capabilities {
        self.negotiated_caps
    }

    /// Drains every `InputEvent` decoded by [`Session::pump`] since the last call.
    pub fn take_input_events(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.pending_inputs)
    }

    /// Whether both peers negotiated transient mascot interactions for this connection.
    #[must_use]
    pub fn supports_mascot_interaction(&self) -> bool {
        self.negotiated_caps
            .contains(Capabilities::MASCOT_INTERACTION)
    }

    /// Most recent device acknowledgment for a social action in this connection.
    #[must_use]
    pub const fn last_mascot_action_applied(&self) -> Option<MascotActionApplied> {
        self.last_mascot_action_applied
    }

    /// Monotonic identity for the current within-process port session.
    #[must_use]
    pub const fn connection_generation(&self) -> u32 {
        self.connection_generation
    }

    /// Drains observations created while parsing safe session events.
    ///
    /// Recording and Tauri emission remain owned by the device task.
    pub fn drain_activity(&mut self) -> Vec<SessionActivity> {
        std::mem::take(&mut self.activity)
    }

    /// Reads and handles all currently-available inbound frames, driving `manager`/`orchestrator` and
    /// auto-responding (`Ready` + a resync `SetState` on connect; heartbeat bookkeeping on `Pong`).
    ///
    /// # Errors
    /// [`SessionError::Transport`] if a read/write fails.
    pub fn pump<L: SerialLink>(
        &mut self,
        link: &mut L,
        manager: &mut ConnectionManager,
        orchestrator: &mut Orchestrator,
    ) -> Result<(), SessionError<L::Error>> {
        let mut chunk = [0u8; 256];
        // Bound both retained bytes and work so a continuously readable peer cannot starve timers.
        for _ in 0..16 {
            let n = match link.read(&mut chunk) {
                Ok(n) => n,
                Err(error) => {
                    self.clear_session_identity();
                    return Err(SessionError::Transport(error));
                }
            };
            if n == 0 {
                break;
            }
            for &byte in &chunk[..n] {
                if byte == 0 {
                    if !self.discard_rx && !self.rx.is_empty() {
                        let packet = self.rx.clone();
                        self.rx.clear();
                        self.handle(&packet, link, manager, orchestrator)?;
                    }
                    self.rx.clear();
                    self.discard_rx = false;
                } else if !self.discard_rx {
                    if self.rx.len() == MAX_WIRE {
                        self.rx.clear();
                        self.discard_rx = true;
                    } else {
                        self.rx.push(byte);
                    }
                }
            }
        }
        Ok(())
    }

    /// Sets the desired companion state and, if the link is `Connected`, transmits it (FR-012).
    ///
    /// # Errors
    /// [`SessionError::Transport`] if the write fails.
    pub fn set_desired<L: SerialLink>(
        &mut self,
        link: &mut L,
        manager: &ConnectionManager,
        orchestrator: &mut Orchestrator,
        state: SendableState,
    ) -> Result<(), SessionError<L::Error>> {
        orchestrator.set_desired(state);
        if manager.state().can_drive_device() {
            self.transmit_set_state(link, state)?;
        }
        Ok(())
    }

    /// Sends one transient social reaction when the connection negotiated support.
    ///
    /// Returns `Ok(false)` without writing when disconnected or paired with older firmware.
    pub fn play_mascot_action<L: SerialLink>(
        &mut self,
        link: &mut L,
        manager: &ConnectionManager,
        action: MascotAction,
        personality: MascotPersonality,
        seed: u32,
    ) -> Result<bool, SessionError<L::Error>> {
        if !manager.state().can_drive_device() || !self.supports_mascot_interaction() {
            return Ok(false);
        }
        self.send(
            link,
            &Message::PlayMascotAction(PlayMascotAction {
                action,
                personality,
                seed,
            }),
        )?;
        Ok(true)
    }

    /// Sends a heartbeat `Ping` and records it as pending (see [`Session::heartbeat_timed_out`]).
    ///
    /// # Errors
    /// [`SessionError::Transport`] if the write fails.
    pub fn send_ping<L: SerialLink>(
        &mut self,
        link: &mut L,
        t_ms: u32,
    ) -> Result<(), SessionError<L::Error>> {
        self.heartbeat.on_ping_sent();
        self.send(link, &Message::Ping(Ping { t_ms }))
    }

    /// Sends a `Presentation` to the device (Slice 002 return path).
    ///
    /// Callers MUST check [`Session::negotiated_caps`] for `PRESENTATION_V1` before calling this —
    /// an unnegotiated capability must produce no encode at all, not merely go unacted-on at the
    /// device (see `runtime::device_task`).
    ///
    /// # Errors
    /// [`SessionError::Transport`] if the write fails.
    pub fn send_presentation<L: SerialLink>(
        &mut self,
        link: &mut L,
        presentation: Presentation,
    ) -> Result<(), SessionError<L::Error>> {
        self.send(link, &Message::Presentation(presentation))
    }

    /// Sends the desk status for the current session. Returns `Ok(false)` without encoding
    /// anything when there is no session or `DESK_STATUS_V1` was not negotiated.
    ///
    /// # Errors
    /// [`SessionError::Transport`] if the write fails.
    pub fn send_status<L: SerialLink>(
        &mut self,
        link: &mut L,
        status: DeskStatus,
    ) -> Result<bool, SessionError<L::Error>> {
        let Some(session) = self.current_session else {
            return Ok(false);
        };
        if !self.negotiated_caps.contains(Capabilities::DESK_STATUS_V1) {
            return Ok(false);
        }
        self.send(link, &Message::Status(Status { session, status }))?;
        Ok(true)
    }

    /// Sends now-playing text (`None` clears it) for the current session. Returns `Ok(false)`
    /// without encoding anything when there is no session or `MEDIA_INFO_V1` was not negotiated.
    ///
    /// # Errors
    /// [`SessionError::Transport`] if the write fails.
    pub fn send_media_info<L: SerialLink>(
        &mut self,
        link: &mut L,
        info: Option<MediaInfo>,
    ) -> Result<bool, SessionError<L::Error>> {
        let Some(session) = self.current_session else {
            return Ok(false);
        };
        if !self.negotiated_caps.contains(Capabilities::MEDIA_INFO_V1) {
            return Ok(false);
        }
        self.send(link, &Message::MediaInfo(MediaInfoUpdate { session, info }))?;
        Ok(true)
    }

    /// Sends the control labels for the current session. Returns `Ok(false)` without encoding
    /// anything when there is no session or `CONTROL_LABELS_V1` was not negotiated.
    ///
    /// # Errors
    /// [`SessionError::Transport`] if the write fails.
    pub fn send_control_labels<L: SerialLink>(
        &mut self,
        link: &mut L,
        labels: ControlLabels,
    ) -> Result<bool, SessionError<L::Error>> {
        let Some(session) = self.current_session else {
            return Ok(false);
        };
        if !self
            .negotiated_caps
            .contains(Capabilities::CONTROL_LABELS_V1)
        {
            return Ok(false);
        }
        self.send(
            link,
            &Message::ControlLabels(ControlLabelsUpdate { session, labels }),
        )?;
        Ok(true)
    }

    /// Sends one action outcome for the current session. Returns `Ok(false)` without encoding
    /// anything when there is no session or `ACTION_FEEDBACK_V1` was not negotiated.
    ///
    /// # Errors
    /// [`SessionError::Transport`] if the write fails.
    pub fn send_feedback<L: SerialLink>(
        &mut self,
        link: &mut L,
        feedback: ActionFeedback,
    ) -> Result<bool, SessionError<L::Error>> {
        let Some(session) = self.current_session else {
            return Ok(false);
        };
        if !self
            .negotiated_caps
            .contains(Capabilities::ACTION_FEEDBACK_V1)
        {
            return Ok(false);
        }
        self.send(
            link,
            &Message::Feedback(Feedback {
                session,
                action: feedback.action,
                kind: feedback.kind,
            }),
        )?;
        Ok(true)
    }

    /// Whether the heartbeat has missed its threshold (the caller then raises `HeartbeatTimeout`).
    #[must_use]
    pub fn heartbeat_timed_out(&self) -> bool {
        self.heartbeat.timed_out()
    }

    fn handle<L: SerialLink>(
        &mut self,
        packet: &[u8],
        link: &mut L,
        manager: &mut ConnectionManager,
        orchestrator: &mut Orchestrator,
    ) -> Result<(), SessionError<L::Error>> {
        let mut scratch: heapless::Vec<u8, MAX_FRAME> = heapless::Vec::new();
        match decode_message(packet, &mut scratch, &self.config.supported_majors) {
            Ok((header, message)) => {
                match self.inbound.classify(header.seq) {
                    SeqClass::Duplicate => return Ok(()),
                    SeqClass::Gap(skipped) => self.observe(
                        ActivityEventKind::ProtocolSequenceGap,
                        Some(ActivityMetadata::ProtocolSequenceGap { skipped }),
                    ),
                    SeqClass::First | SeqClass::Ok => {}
                }
                self.handle_message(header.version, message, link, manager, orchestrator)?;
            }
            Err(ProtoError::UnsupportedVersion) => {
                // A supported-major gate rejected the frame. Read just the header to learn the
                // device's major and surface incompatibility (the payload layout is not trusted).
                let mut header_scratch: heapless::Vec<u8, MAX_FRAME> = heapless::Vec::new();
                if let Ok((header, _payload)) = decode_frame(packet, &mut header_scratch) {
                    self.inbound.classify(header.seq);
                    if manager.apply(ManagerEvent::HandshakeIncompatible {
                        device_major: header.version.major,
                    }) {
                        self.clear_session_identity();
                        self.observe(ActivityEventKind::IncompatibleFirmware, None);
                        self.send(
                            link,
                            &Message::Bye(Bye {
                                reason: ByeReason::IncompatibleVersion,
                            }),
                        )?;
                    }
                }
            }
            Err(error) => {
                let mut header_scratch: heapless::Vec<u8, MAX_FRAME> = heapless::Vec::new();
                let safe_header = decode_frame(packet, &mut header_scratch)
                    .ok()
                    .map(|(header, _)| header);
                self.observe(
                    ActivityEventKind::ProtocolMalformedFrame,
                    Some(ActivityMetadata::ProtocolMalformed {
                        category: malformed_category(error),
                        payload_len: safe_header.map(|header| header.payload_len),
                        sequence: safe_header.map(|header| header.seq),
                    }),
                );
            }
        }
        Ok(())
    }

    fn handle_message<L: SerialLink>(
        &mut self,
        device_version: ProtocolVersion,
        message: Message,
        link: &mut L,
        manager: &mut ConnectionManager,
        orchestrator: &mut Orchestrator,
    ) -> Result<(), SessionError<L::Error>> {
        match message {
            Message::HelloAck(ack) => {
                let Some(sent) = self.sent_hello else {
                    return Ok(()); // unsolicited HelloAck — ignore
                };
                match evaluate_hello_ack(
                    &sent,
                    &ack,
                    device_version,
                    self.config.protocol_version,
                    &self.config.supported_majors,
                ) {
                    HandshakeOutcome::Compatible(ready) => {
                        // Session identity is scoped to this connection: the nonce we sent becomes
                        // the current session only once the handshake is accepted.
                        self.current_session = Some(sent.nonce);
                        // The handshake is over. Retiring the outstanding `Hello` makes any later
                        // `HelloAck` take the unsolicited path above and be ignored, as protocol
                        // contract section 8 requires of a message invalid for the current phase.
                        // Left set, a stray or duplicate ack would be re-evaluated and a nonce
                        // mismatch would take the `BadNonce` arm, tearing down a live session.
                        self.sent_hello = None;
                        manager.apply(ManagerEvent::HandshakeOk(summarize(&ack, device_version)));
                        self.negotiated_caps = ready.negotiated_caps;
                        self.observe(ActivityEventKind::HandshakeSucceeded, None);
                        self.observe(
                            ActivityEventKind::DeviceNegotiated,
                            Some(ActivityMetadata::Negotiated {
                                firmware_major: ack.firmware_version.major,
                                firmware_minor: ack.firmware_version.minor,
                                firmware_patch: ack.firmware_version.patch,
                                protocol_major: device_version.major,
                                protocol_minor: device_version.minor,
                                device_id_hash_short: hash_device_id_short(&ack.device_id),
                                capabilities: ready.negotiated_caps.bits(),
                            }),
                        );
                        self.send(link, &Message::Ready(ready))?;
                        // Resynchronize the device to our desired state on (re)connect (FR-009).
                        let desired = orchestrator.resync_state();
                        self.transmit_set_state(link, desired)?;
                    }
                    HandshakeOutcome::Incompatible { device_major } => {
                        self.clear_session_identity();
                        if manager.apply(ManagerEvent::HandshakeIncompatible { device_major }) {
                            self.observe(ActivityEventKind::IncompatibleFirmware, None);
                            self.send(
                                link,
                                &Message::Bye(Bye {
                                    reason: ByeReason::IncompatibleVersion,
                                }),
                            )?;
                        }
                    }
                    HandshakeOutcome::BadNonce => {
                        // Identity not confirmed — treat as a failed handshake.
                        self.clear_session_identity();
                        manager.apply(ManagerEvent::HandshakeTimeout);
                        self.observe(ActivityEventKind::HandshakeTimedOut, None);
                    }
                }
            }
            Message::Pong(_) => self.heartbeat.on_pong(),
            Message::StateReport(report) if self.reported != Some(report.reported) => {
                self.reported = Some(report.reported);
                self.observe(
                    if report.reported == orchestrator.desired().to_companion() {
                        ActivityEventKind::StateSynchronized
                    } else {
                        ActivityEventKind::DeviceStateObserved
                    },
                    Some(ActivityMetadata::DeviceState {
                        reported: report.reported,
                    }),
                );
            }
            Message::MascotActionApplied(applied) => {
                self.last_mascot_action_applied = Some(applied);
                self.observe(
                    ActivityEventKind::SocialActionApplied,
                    Some(ActivityMetadata::Action {
                        state: None,
                        personality: Some(applied.personality),
                        self_play: None,
                        action: Some(applied.action),
                        seed: Some(applied.seed),
                        applied_at_ms: Some(applied.applied_at_ms),
                        autonomous: None,
                    }),
                );
            }
            Message::Diagnostic(diagnostic) => self.observe(
                device_diagnostic_kind(diagnostic.code),
                Some(ActivityMetadata::DeviceDiagnostic {
                    category: diagnostic.category,
                    code: diagnostic.code,
                }),
            ),
            Message::Error(error) => self.observe(
                match error.category {
                    kivori_protocol::ErrorCategory::Busy => ActivityEventKind::DeviceBusy,
                    kivori_protocol::ErrorCategory::Timeout => ActivityEventKind::DeviceTimedOut,
                    _ => ActivityEventKind::DeviceError,
                },
                Some(ActivityMetadata::DeviceDiagnostic {
                    category: error.category,
                    code: error.code,
                }),
            ),
            // Unnegotiated input stays inert, mirroring the firmware Presentation gate.
            Message::InputEvent(event)
                if self
                    .negotiated_caps
                    .contains(Capabilities::PHYSICAL_INPUT_V1) =>
            {
                self.pending_inputs.push(event);
            }
            // The device is ending the session: no connection, no session identity.
            Message::Bye(_) => {
                self.clear_session_identity();
                manager.apply(ManagerEvent::PortRemoved);
                return Err(SessionError::PeerClosed);
            }
            // `Ready` and the remaining device→desktop kinds are observed by the UI layer, not here.
            _ => {}
        }
        Ok(())
    }

    fn transmit_set_state<L: SerialLink>(
        &mut self,
        link: &mut L,
        desired: SendableState,
    ) -> Result<(), SessionError<L::Error>> {
        self.send(
            link,
            &Message::SetState(SetState {
                desired,
                at_ms: None,
            }),
        )
    }

    fn send<L: SerialLink>(
        &mut self,
        link: &mut L,
        message: &Message,
    ) -> Result<(), SessionError<L::Error>> {
        let mut wire: heapless::Vec<u8, MAX_WIRE> = heapless::Vec::new();
        if encode_message(
            message,
            self.config.protocol_version,
            self.tx_seq,
            &mut wire,
        )
        .is_err()
        {
            self.clear_session_identity();
            return Err(SessionError::Encoding);
        }
        self.tx_seq = self.tx_seq.wrapping_add(1);
        let mut sent = 0;
        while sent < wire.len() {
            let n = match link.write(&wire[sent..]) {
                Ok(n) => n,
                Err(error) => {
                    self.clear_session_identity();
                    return Err(SessionError::Transport(error));
                }
            };
            if n == 0 {
                self.clear_session_identity();
                return Err(SessionError::WriteZero);
            }
            sent += n;
        }
        Ok(())
    }

    /// Drops every observation and input scoped to the ended connection.
    fn clear_session_identity(&mut self) {
        self.current_session = None;
        self.sent_hello = None;
        self.negotiated_caps = Capabilities::NONE;
        self.pending_inputs.clear();
        self.rx.clear();
        self.discard_rx = false;
        self.reported = None;
        self.last_mascot_action_applied = None;
        self.heartbeat = HeartbeatMonitor::default();
    }

    fn observe(&mut self, kind: ActivityEventKind, metadata: Option<ActivityMetadata>) {
        self.activity.push(SessionActivity::new(kind, metadata));
    }
}

fn device_diagnostic_kind(code: u16) -> ActivityEventKind {
    match code {
        1 => ActivityEventKind::DeviceDiagnosticFraming,
        2 => ActivityEventKind::DeviceDiagnosticChecksum,
        3 => ActivityEventKind::DeviceDiagnosticVersion,
        4 => ActivityEventKind::DeviceDiagnosticPayload,
        5 => ActivityEventKind::DeviceSequenceGap,
        6 => ActivityEventKind::DeviceDisplayFault,
        7 => ActivityEventKind::DeviceLinkLost,
        _ => ActivityEventKind::DeviceDiagnosticUnknown,
    }
}

fn malformed_category(error: ProtoError) -> crate::activity::ProtocolMalformedCategory {
    match error {
        ProtoError::BadCrc => crate::activity::ProtocolMalformedCategory::Checksum,
        ProtoError::UnsupportedVersion => crate::activity::ProtocolMalformedCategory::Version,
        ProtoError::PayloadTooLarge | ProtoError::Postcard => {
            crate::activity::ProtocolMalformedCategory::Payload
        }
        ProtoError::BufferOverflow
        | ProtoError::Cobs
        | ProtoError::TooShort
        | ProtoError::BadMagic
        | ProtoError::LengthMismatch => crate::activity::ProtocolMalformedCategory::Framing,
    }
}
#[cfg(test)]
mod tests {
    //! `HandshakeOutcome::Incompatible` from `evaluate_hello_ack` can never actually be produced
    //! through the wire path: `decode_message` (see `handle`, above) already gates every inbound
    //! frame on `self.config.supported_majors` — the exact same list `evaluate_hello_ack` checks —
    //! before `handle_message` ever runs. A frame whose major would trigger `Incompatible` here is
    //! rejected earlier as `ProtoError::UnsupportedVersion` and handled by the *other*,
    //! frame-level incompatible path in `handle()` instead. This is a structural fact of the
    //! existing dual-gate design, not something introduced here — no test built on `Session`'s
    //! public wire API (`open`/`pump`) can reach this branch, confirmed empirically. This unit
    //! test exercises the private `handle_message` directly (module-private access) so the
    //! `current_session` invariant is still proven at the point that decides it.

    use super::*;
    use crate::device::nonce::FixedNonceSource;
    use kivori_protocol::HelloAck;

    struct NullLink;

    impl SerialLink for NullLink {
        type Error = std::convert::Infallible;

        fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
            Ok(0)
        }

        fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            Ok(buf.len())
        }
    }

    #[test]
    fn handle_message_clears_current_session_on_incompatible_outcome() {
        let mut session = Session::with_nonce_source(
            SessionConfig::default(),
            Box::new(FixedNonceSource::new(vec![7])),
        );
        // Simulate an already-accepted session: a Hello was sent and its nonce is the live
        // session identity (exactly the state `Compatible` leaves behind).
        session.sent_hello = Some(build_hello(
            session.config.app_version,
            session.config.capabilities,
            7,
        ));
        session.current_session = Some(7);

        let ack = HelloAck {
            device_caps: Capabilities::NONE,
            device_id: [0u8; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            nonce_echo: 7, // correct nonce: evaluate_hello_ack must not short-circuit to BadNonce
        };
        // Not in `SessionConfig::default()`'s `supported_majors` (`[PROTOCOL_MAJOR]`) — only
        // reachable by calling `handle_message` directly, since `decode_message` would reject a
        // real wire frame at this major before `handle_message` ever saw it.
        let incompatible_version = ProtocolVersion::new(99, 0);

        let mut link = NullLink;
        let mut manager = ConnectionManager::new();
        let mut orchestrator = Orchestrator::new();

        session
            .handle_message(
                incompatible_version,
                Message::HelloAck(ack),
                &mut link,
                &mut manager,
                &mut orchestrator,
            )
            .expect("handle_message");

        assert_eq!(
            session.current_session, None,
            "an Incompatible handshake outcome must clear the live session identity"
        );
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn recovery_delimiterless_input_bounds_memory_and_read_work() {
        struct Flood {
            remaining: usize,
            reads: usize,
        }
        impl SerialLink for Flood {
            type Error = core::convert::Infallible;
            fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Self::Error> {
                self.reads += 1;
                let n = bytes.len().min(self.remaining);
                bytes[..n].fill(1);
                self.remaining -= n;
                Ok(n)
            }
            fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
                Ok(bytes.len())
            }
        }
        let mut session = Session::new(SessionConfig::default());
        let mut flood = Flood {
            remaining: 1024 * 1024,
            reads: 0,
        };
        session
            .pump(
                &mut flood,
                &mut ConnectionManager::new(),
                &mut Orchestrator::new(),
            )
            .unwrap();
        assert!(
            session.rx.len() <= MAX_WIRE,
            "retained bytes must respect the framing bound"
        );
        assert!(
            flood.reads <= 64,
            "one pump must yield while a sender still has bytes"
        );
    }
}
