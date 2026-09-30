//! Desktop session-driver tests (Phase 11 host-side; FR-002/009/012, SC-008). The test plays the
//! *device*: it decodes what the desktop transmits and injects encoded device responses through an
//! in-memory link, exercising the full connect → resync → set-state → reconnect loop.

use std::collections::VecDeque;
use std::convert::Infallible;

use kivori_desktop::device::fsm::ConnectionManager;
use kivori_desktop::device::nonce::{FailingNonceSource, FixedNonceSource};
use kivori_desktop::device::session::{Session, SessionConfig, SessionError};
use kivori_desktop::device::transport::SerialLink;
use kivori_desktop::device::ManagerEvent;
use kivori_desktop::orchestrator::Orchestrator;
use kivori_model::{
    Capabilities, ConnectionState, MascotAction, MascotPersonality, ProtocolVersion, SendableState,
};
use kivori_protocol::{
    decode_message, encode_message, Bye, ByeReason, FirmwareVersion, HelloAck, MascotActionApplied,
    Message, Pong, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

#[derive(Default)]
struct FakeLink {
    to_device: VecDeque<u8>,
    from_device: VecDeque<u8>,
}

impl SerialLink for FakeLink {
    type Error = Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Infallible> {
        let mut n = 0;
        while n < buf.len() {
            match self.from_device.pop_front() {
                Some(byte) => {
                    buf[n] = byte;
                    n += 1;
                }
                None => break,
            }
        }
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        self.to_device.extend(buf.iter().copied());
        Ok(buf.len())
    }
}

/// Encodes a device→desktop message (at `version`, sequence `seq`) into the link.
fn device_push(link: &mut FakeLink, msg: &Message, version: ProtocolVersion, seq: u16) {
    let mut wire: heapless::Vec<u8, { kivori_protocol::MAX_WIRE }> = heapless::Vec::new();
    encode_message(msg, version, seq, &mut wire).expect("encode");
    link.from_device.extend(wire.iter().copied());
}

/// Decodes everything the desktop has transmitted since the last drain.
fn desktop_drain(link: &mut FakeLink) -> Vec<Message> {
    let bytes: Vec<u8> = link.to_device.drain(..).collect();
    let mut out = Vec::new();
    let mut scratch: heapless::Vec<u8, { kivori_protocol::MAX_FRAME }> = heapless::Vec::new();
    for packet in bytes.split(|&b| b == 0) {
        if packet.is_empty() {
            continue;
        }
        if let Ok((_, msg)) = decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
            out.push(msg);
        }
    }
    out
}

fn hello_nonce(msgs: &[Message]) -> u32 {
    for msg in msgs {
        if let Message::Hello(hello) = msg {
            return hello.nonce;
        }
    }
    panic!("expected a Hello, got {msgs:?}");
}

fn wire_version() -> ProtocolVersion {
    ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
}

fn device_ack(nonce: u32) -> Message {
    Message::HelloAck(HelloAck {
        device_caps: Capabilities::MASCOT_INTERACTION,
        device_id: [0x5A; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 4,
            patch: 2,
        },
        nonce_echo: nonce,
    })
}

fn set_state_desired(msgs: &[Message]) -> Option<SendableState> {
    msgs.iter().find_map(|m| match m {
        Message::SetState(s) => Some(s.desired),
        _ => None,
    })
}

/// Drives a full handshake and returns the connected session + peers with the given initial desired.
fn connect(desired: SendableState) -> (FakeLink, Session, ConnectionManager, Orchestrator) {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    orchestrator.set_desired(desired);

    session.open(&mut link, &mut manager).expect("open");
    let nonce = hello_nonce(&desktop_drain(&mut link));
    device_push(&mut link, &device_ack(nonce), wire_version(), 0);
    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");
    (link, session, manager, orchestrator)
}

#[test]
fn handshake_reaches_connected_and_resyncs_desired_state() {
    let (mut link, _session, manager, _orch) = connect(SendableState::Idle);
    assert_eq!(manager.state(), ConnectionState::Connected);
    let device = manager.device().expect("connected device");
    assert_eq!(device.firmware_version.minor, 4);
    assert_eq!(device.device_id_hash_short.len(), 8);

    // On connect the desktop confirms with Ready and resyncs the desired state.
    let sent = desktop_drain(&mut link);
    assert!(
        sent.iter().any(|m| matches!(m, Message::Ready(_))),
        "sent Ready"
    );
    assert_eq!(
        set_state_desired(&sent),
        Some(SendableState::Idle),
        "resync SetState"
    );
}

#[test]
fn resync_transmits_the_current_desired_not_the_default() {
    let (mut link, _session, manager, _orch) = connect(SendableState::Happy);
    assert_eq!(manager.state(), ConnectionState::Connected);
    assert_eq!(
        set_state_desired(&desktop_drain(&mut link)),
        Some(SendableState::Happy)
    );
}

#[test]
fn set_desired_transmits_only_when_connected() {
    let (mut link, mut session, manager, mut orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link); // clear the connect traffic

    session
        .set_desired(&mut link, &manager, &mut orch, SendableState::Busy)
        .expect("set_desired");
    assert_eq!(
        set_state_desired(&desktop_drain(&mut link)),
        Some(SendableState::Busy)
    );
    assert_eq!(orch.desired(), SendableState::Busy);
}

#[test]
fn negotiated_social_action_is_sent_and_applied_ack_is_retained() {
    let (mut link, mut session, manager, mut orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link);

    assert!(session.supports_mascot_interaction());
    assert!(session
        .play_mascot_action(
            &mut link,
            &manager,
            MascotAction::Pet,
            MascotPersonality::Cozy,
            99,
        )
        .expect("send action"));
    assert!(desktop_drain(&mut link).iter().any(|message| matches!(
        message,
        Message::PlayMascotAction(action)
            if action.action == MascotAction::Pet
                && action.personality == MascotPersonality::Cozy
                && action.seed == 99
    )));

    device_push(
        &mut link,
        &Message::MascotActionApplied(MascotActionApplied {
            action: MascotAction::Pet,
            personality: MascotPersonality::Cozy,
            seed: 99,
            applied_at_ms: 7_500,
        }),
        wire_version(),
        1,
    );
    session
        .pump(&mut link, &mut ConnectionManager::new(), &mut orch)
        .expect("pump applied ack");
    assert_eq!(
        session.last_mascot_action_applied(),
        Some(MascotActionApplied {
            action: MascotAction::Pet,
            personality: MascotPersonality::Cozy,
            seed: 99,
            applied_at_ms: 7_500,
        })
    );
}

#[test]
fn incompatible_major_is_surfaced_and_bye_sent() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();

    session.open(&mut link, &mut manager).expect("open");
    let nonce = hello_nonce(&desktop_drain(&mut link));
    // Device answers at an unsupported major version (2.x).
    device_push(&mut link, &device_ack(nonce), ProtocolVersion::new(2, 0), 0);
    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");

    assert_eq!(manager.state(), ConnectionState::Incompatible);
    assert!(manager.incompatible_reason().unwrap().contains("v2"));
    assert!(
        desktop_drain(&mut link)
            .iter()
            .any(|m| matches!(m, Message::Bye(_))),
        "sent Bye on incompatible"
    );
}

#[test]
fn bad_nonce_fails_the_handshake() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();

    session.open(&mut link, &mut manager).expect("open");
    let _real_nonce = hello_nonce(&desktop_drain(&mut link));
    device_push(&mut link, &device_ack(0xBAD_BAD), wire_version(), 0);
    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");

    assert_eq!(
        manager.state(),
        ConnectionState::Error,
        "unconfirmed identity → error"
    );
    assert_eq!(
        session.current_session(),
        None,
        "a failed handshake leaves no session identity behind"
    );
}

#[test]
fn heartbeat_pong_clears_the_miss_counter() {
    let (mut link, mut session, _manager, _orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link);

    session.send_ping(&mut link, 111).expect("ping");
    session.send_ping(&mut link, 222).expect("ping");
    assert!(!session.heartbeat_timed_out());

    device_push(
        &mut link,
        &Message::Pong(Pong {
            t_ms_echo: 111,
            uptime_ms: 5000,
        }),
        wire_version(),
        1,
    );
    let mut manager = ConnectionManager::new();
    let mut orch = Orchestrator::new();
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert!(!session.heartbeat_timed_out());
}

#[test]
fn missed_heartbeats_reach_the_timeout_threshold() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    for t in 0..3 {
        session.send_ping(&mut link, t).expect("ping");
    }
    assert!(session.heartbeat_timed_out(), "3 unanswered pings time out");
}

#[test]
fn malformed_device_bytes_are_dropped_without_panic() {
    let (mut link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    assert_eq!(manager.state(), ConnectionState::Connected);

    link.from_device.extend([0x02, 0xFF, 0x00, 0x99, 0x00]);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump survives garbage");
    assert_eq!(
        manager.state(),
        ConnectionState::Connected,
        "state unchanged by garbage"
    );
}

#[test]
fn reconnect_resyncs_the_within_process_desired_state() {
    let (mut link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link);

    // User sets Busy while connected.
    session
        .set_desired(&mut link, &manager, &mut orch, SendableState::Busy)
        .expect("set_desired");
    let _ = desktop_drain(&mut link);

    // Unplug (the run loop would raise this on I/O loss), then reconnect.
    manager.apply(ManagerEvent::PortRemoved);
    assert_eq!(manager.state(), ConnectionState::Disconnected);

    session.open(&mut link, &mut manager).expect("reopen");
    assert_eq!(session.connection_generation(), 2);
    let nonce = hello_nonce(&desktop_drain(&mut link));
    device_push(&mut link, &device_ack(nonce), wire_version(), 0);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");

    assert_eq!(manager.state(), ConnectionState::Connected);
    // The device is resynced to Busy (desired survives within the process).
    assert_eq!(
        set_state_desired(&desktop_drain(&mut link)),
        Some(SendableState::Busy)
    );
}

/// A link whose `read` always fails, used to simulate an I/O loss on an already-connected session.
#[derive(Default)]
struct AlwaysFailingReadLink;

impl SerialLink for AlwaysFailingReadLink {
    type Error = &'static str;

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Err("simulated read failure")
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        Ok(buf.len())
    }
}

#[test]
fn a_nonce_unavailable_source_fails_the_attempt_and_never_reaches_connected() {
    let mut link = FakeLink::default();
    let mut session =
        Session::with_nonce_source(SessionConfig::default(), Box::new(FailingNonceSource));
    let mut manager = ConnectionManager::new();

    let result = session.open(&mut link, &mut manager);

    assert_eq!(
        result,
        Err(SessionError::NonceUnavailable),
        "no OS entropy → the connection attempt fails, it does not panic"
    );
    assert_ne!(
        manager.state(),
        ConnectionState::Connected,
        "a session with no nonce must never reach Connected"
    );
    // No Hello was ever sent — the failure is before any wire traffic.
    assert!(desktop_drain(&mut link).is_empty());
}

#[test]
fn current_session_is_none_before_handshake_and_set_after_it_is_accepted() {
    let mut link = FakeLink::default();
    let mut session = Session::with_nonce_source(
        SessionConfig::default(),
        Box::new(FixedNonceSource::new(vec![42])),
    );
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();

    assert_eq!(session.current_session(), None);

    session.open(&mut link, &mut manager).expect("open");
    assert_eq!(
        session.current_session(),
        None,
        "not yet connection-scoped identity until the handshake is accepted"
    );

    device_push(&mut link, &device_ack(42), wire_version(), 0);
    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");

    assert_eq!(manager.state(), ConnectionState::Connected);
    assert_eq!(session.current_session(), Some(42));
}

#[test]
fn current_session_is_cleared_on_a_received_bye() {
    let (mut link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    assert_eq!(manager.state(), ConnectionState::Connected);
    assert!(session.current_session().is_some());

    device_push(
        &mut link,
        &Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        wire_version(),
        1,
    );
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump handles Bye");

    assert_eq!(
        session.current_session(),
        None,
        "a received Bye ends the session's identity"
    );
}

#[test]
fn current_session_is_cleared_when_the_link_fails() {
    let (_link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    assert!(session.current_session().is_some());

    let mut failing_link = AlwaysFailingReadLink;
    let result = session.pump(&mut failing_link, &mut manager, &mut orch);

    assert!(result.is_err(), "the read failure must surface as an error");
    assert_eq!(
        session.current_session(),
        None,
        "an I/O failure must not leave a stale session identity behind"
    );
}

/// `sent_hello` is not cleared after acceptance (pre-existing Feature 001 behaviour, out of scope
/// here), so an already-`Connected` session still evaluates a later stray/duplicate `HelloAck`
/// frame. A frame at an unsupported major is intercepted by `decode_message`'s own gate before it
/// ever reaches `evaluate_hello_ack` — both check the identical `supported_majors` list — so this
/// is a frame-level incompatibility, not a `HandshakeOutcome::Incompatible` from the handshake
/// evaluation. The FSM has no legal `Connected -> Incompatible` transition (only
/// `Connecting -> Incompatible`), so the manager stays `Connected` and the stray frame is
/// effectively dropped: the live session identity must survive it.
///
/// (`HandshakeOutcome::Incompatible`'s own `current_session = None`, in `handle_message`, is
/// therefore unreachable through `Session`'s public wire path for any `SessionConfig` — confirmed
/// empirically while writing this test. It is covered directly, at the unit that decides it, by
/// `handle_message_clears_current_session_on_incompatible_outcome` in `src/device/session.rs`.)
#[test]
fn a_stray_unsupported_major_frame_after_connect_is_dropped_and_session_identity_survives() {
    let (mut link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    assert_eq!(manager.state(), ConnectionState::Connected);
    let nonce = session
        .current_session()
        .expect("a connected session has a current nonce");

    // A stray frame at an unsupported major — `decode_message` rejects it before any handshake
    // re-evaluation happens.
    device_push(&mut link, &device_ack(nonce), ProtocolVersion::new(2, 0), 1);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump survives the frame it cannot decode for this session");

    assert_eq!(
        manager.state(),
        ConnectionState::Connected,
        "no legal Connected -> Incompatible transition exists; the frame is dropped"
    );
    assert_eq!(
        session.current_session(),
        Some(nonce),
        "a dropped frame must not disturb the live session identity"
    );
}

/// Unlike an unsupported major, a bad nonce is NOT caught by `decode_message` (which only checks
/// the protocol major) — the frame decodes fine and reaches `evaluate_hello_ack`.
///
/// This test previously asserted the opposite of what it asserts now: that a stray bad-nonce
/// `HelloAck` arriving AFTER the handshake clears the live session. That was the defect, not the
/// contract. Protocol contract section 8 requires a message invalid for the current phase to be
/// rejected, and the handshake phase is over once `Connected` is reached — so the ack is
/// unsolicited and must be ignored, never re-evaluated as a handshake where a nonce mismatch
/// tears down session identity, capabilities and any open gesture. The `BadNonce` clearing site
/// itself stays covered by `bad_nonce_fails_the_handshake`, where it is still reachable.
#[test]
fn a_stray_hello_ack_while_connected_does_not_tear_down_the_session() {
    let (mut link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    assert_eq!(manager.state(), ConnectionState::Connected);
    let established = session.current_session().expect("handshake accepted");
    let caps = session.negotiated_caps();
    let _ = desktop_drain(&mut link); // clear the connect traffic

    // A stray HelloAck echoing the wrong nonce (it can never match the original Hello's nonce).
    device_push(&mut link, &device_ack(0xBAD_BAD), wire_version(), 1);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");

    assert_eq!(
        manager.state(),
        ConnectionState::Connected,
        "a stray HelloAck must not tear down a healthy session"
    );
    assert_eq!(
        session.current_session(),
        Some(established),
        "the live session identity survives an unsolicited ack"
    );
    assert_eq!(session.negotiated_caps(), caps, "capabilities survive");
    assert!(
        desktop_drain(&mut link).is_empty(),
        "an unsolicited HelloAck is ignored, not answered"
    );
}

// --- capability gating (finding 1, task-11 review round 1): a desktop that never negotiated
// `PHYSICAL_INPUT_V1` must never execute an `InputEvent` — not queue it, not hand it to
// `InputIngress`, not reach a backend write. Mirrors the firmware's own receive-side gate on
// `Presentation` (`proto.rs`).

use kivori_protocol::{ControlId, InputEvent, InputKind};

fn detent_event(session: u32) -> Message {
    Message::InputEvent(InputEvent {
        session,
        gesture_id: 1,
        control: ControlId::Rotary,
        kind: InputKind::GestureStarted,
        device_ms: 0,
    })
}

#[test]
fn input_events_are_dropped_without_a_negotiated_physical_input_capability() {
    // `connect()` negotiates `Capabilities::NONE` (the device in `device_ack()` advertises none).
    let (mut link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    assert!(
        !session
            .negotiated_caps()
            .contains(Capabilities::PHYSICAL_INPUT_V1),
        "test setup: PHYSICAL_INPUT_V1 must not be negotiated here"
    );
    let nonce = session.current_session().expect("connected session");

    device_push(&mut link, &detent_event(nonce), wire_version(), 1);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");

    assert!(
        session.take_input_events().is_empty(),
        "an InputEvent must never be queued for execution without a negotiated capability"
    );
}

#[test]
fn input_events_are_accepted_once_physical_input_v1_is_negotiated() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();

    session.open(&mut link, &mut manager).expect("open");
    let nonce = hello_nonce(&desktop_drain(&mut link));
    device_push(
        &mut link,
        &Message::HelloAck(HelloAck {
            device_caps: Capabilities::PHYSICAL_INPUT_V1,
            device_id: [0x5A; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 4,
                patch: 2,
            },
            nonce_echo: nonce,
        }),
        wire_version(),
        0,
    );
    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");
    assert!(session
        .negotiated_caps()
        .contains(Capabilities::PHYSICAL_INPUT_V1));

    device_push(&mut link, &detent_event(nonce), wire_version(), 1);
    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");

    assert_eq!(
        session.take_input_events(),
        vec![InputEvent {
            session: nonce,
            gesture_id: 1,
            control: ControlId::Rotary,
            kind: InputKind::GestureStarted,
            device_ms: 0,
        }],
        "a negotiated capability must let the InputEvent reach execution"
    );
}
