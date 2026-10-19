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
    connect_with_caps(desired, Capabilities::MASCOT_INTERACTION)
}

fn connect_with_caps(
    desired: SendableState,
    caps: Capabilities,
) -> (FakeLink, Session, ConnectionManager, Orchestrator) {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    orchestrator.set_desired(desired);

    session.open(&mut link, &mut manager).expect("open");
    let nonce = hello_nonce(&desktop_drain(&mut link));
    let Message::HelloAck(mut ack) = device_ack(nonce) else {
        unreachable!()
    };
    ack.device_caps = caps;
    device_push(&mut link, &Message::HelloAck(ack), wire_version(), 0);
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
    assert_eq!(
        session.pump(&mut link, &mut manager, &mut orch),
        Err(SessionError::PeerClosed)
    );
    assert_eq!(manager.state(), ConnectionState::Disconnected);

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

#[test]
fn recovery_bye_disconnects_and_discards_inputs_in_the_same_read() {
    let (mut link, mut session, mut manager, mut orch) =
        connect_with_caps(SendableState::Idle, Capabilities::PHYSICAL_INPUT_V1);
    let nonce = session.current_session().unwrap();
    device_push(&mut link, &detent_event(nonce), wire_version(), 1);
    device_push(
        &mut link,
        &Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        wire_version(),
        2,
    );
    device_push(&mut link, &detent_event(nonce), wire_version(), 3);
    assert!(session.pump(&mut link, &mut manager, &mut orch).is_err());
    assert_eq!(manager.state(), ConnectionState::Disconnected);
    assert_eq!(session.current_session(), None);
    assert!(session.take_input_events().is_empty());
    device_push(&mut link, &device_ack(nonce), wire_version(), 4);
    session.pump(&mut link, &mut manager, &mut orch).unwrap();
    assert_eq!(manager.state(), ConnectionState::Disconnected);
}

#[test]
fn recovery_zero_writes_never_report_a_successful_hello() {
    struct Cutoff(usize);
    impl SerialLink for Cutoff {
        type Error = Infallible;
        fn read(&mut self, _: &mut [u8]) -> Result<usize, Self::Error> {
            Ok(0)
        }
        fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
            let n = bytes.len().min(self.0);
            self.0 -= n;
            Ok(n)
        }
    }
    for allowed in [0, 3] {
        let mut session = Session::new(SessionConfig::default());
        assert!(
            session
                .open(&mut Cutoff(allowed), &mut ConnectionManager::new())
                .is_err(),
            "a zero write after {allowed} bytes cannot mean delivery"
        );
        assert_eq!(session.current_session(), None);
    }
}

#[test]
fn recovery_oversized_frame_resynchronizes_at_the_delimiter() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orch = Orchestrator::new();
    session.open(&mut link, &mut manager).unwrap();
    let nonce = hello_nonce(&desktop_drain(&mut link));
    link.from_device
        .extend(std::iter::repeat_n(1, kivori_protocol::MAX_WIRE * 20));
    link.from_device.push_back(0);
    device_push(&mut link, &device_ack(nonce), wire_version(), 0);
    for _ in 0..100 {
        session.pump(&mut link, &mut manager, &mut orch).unwrap();
        if manager.state() == ConnectionState::Connected {
            break;
        }
    }
    assert_eq!(session.current_session(), Some(nonce));
    assert_eq!(manager.state(), ConnectionState::Connected);
}

#[test]
fn recovery_a_burst_of_valid_frames_is_not_dropped_by_the_rx_bound() {
    let (mut link, mut session, mut manager, mut orch) =
        connect_with_caps(SendableState::Idle, Capabilities::PHYSICAL_INPUT_V1);
    let nonce = session.current_session().unwrap();
    for seq in 1..=300 {
        device_push(&mut link, &detent_event(nonce), wire_version(), seq);
    }
    let mut count = 0;
    for _ in 0..100 {
        session.pump(&mut link, &mut manager, &mut orch).unwrap();
        count += session.take_input_events().len();
        if link.from_device.is_empty() {
            break;
        }
    }
    assert_eq!(count, 300);
}

#[test]
fn close_writes_a_bye_frame_and_ends_the_session() {
    let (mut link, mut session, _manager, _orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link);
    assert!(session.current_session().is_some());

    session
        .close(&mut link, ByeReason::Shutdown)
        .expect("close");

    let sent = desktop_drain(&mut link);
    assert!(
        sent.iter()
            .any(|m| matches!(m, Message::Bye(bye) if bye.reason == ByeReason::Shutdown)),
        "close wrote Bye(Shutdown): {sent:?}"
    );
    assert_eq!(session.current_session(), None);
    // Closing again, with no session, writes nothing.
    session
        .close(&mut link, ByeReason::Shutdown)
        .expect("close again");
    assert!(desktop_drain(&mut link).is_empty());
}

#[test]
fn takeover_byes_are_sent_only_to_a_device_that_negotiated_them() {
    let (mut link, mut session, _manager, _orch) = connect(SendableState::Idle);
    assert!(!session.supports_host_takeovers());
    let _ = desktop_drain(&mut link);
    session
        .close(&mut link, ByeReason::HostSleeping)
        .expect("close");
    assert!(
        desktop_drain(&mut link).is_empty(),
        "an older device never sees a reason it does not know"
    );
    assert_eq!(session.current_session(), None);

    let caps = Capabilities::MASCOT_INTERACTION.union(Capabilities::HOST_TAKEOVERS_V1);
    let (mut link, mut session, _manager, _orch) = connect_with_caps(SendableState::Idle, caps);
    assert!(session.supports_host_takeovers());
    let _ = desktop_drain(&mut link);
    session
        .close(&mut link, ByeReason::FirmwareUpdate)
        .expect("close");
    assert!(desktop_drain(&mut link)
        .iter()
        .any(|m| matches!(m, Message::Bye(bye) if bye.reason == ByeReason::FirmwareUpdate)));
}

fn feedback_kinds(msgs: &[Message]) -> Vec<kivori_model::desk::FeedbackKind> {
    msgs.iter()
        .filter_map(|m| match m {
            Message::Feedback(f) => Some(f.kind),
            _ => None,
        })
        .collect()
}

#[test]
fn triggered_feedback_goes_out_as_unverified_unless_quiet_ack_was_negotiated() {
    use kivori_model::desk::{ActionFeedback, ActionKind, FeedbackKind};
    let triggered = ActionFeedback {
        action: ActionKind::PlayPause,
        kind: FeedbackKind::Triggered,
    };

    let base = Capabilities::MASCOT_INTERACTION.union(Capabilities::ACTION_FEEDBACK_V1);
    let (mut link, mut session, _manager, _orch) = connect_with_caps(SendableState::Idle, base);
    let _ = desktop_drain(&mut link);
    assert!(session.send_feedback(&mut link, triggered).expect("send"));
    assert_eq!(
        feedback_kinds(&desktop_drain(&mut link)),
        [FeedbackKind::Unverified],
        "a device without bit 11 keeps today's Unverified"
    );

    let caps = base.union(Capabilities::QUIET_ACK_V1);
    let (mut link, mut session, _manager, _orch) = connect_with_caps(SendableState::Idle, caps);
    let _ = desktop_drain(&mut link);
    assert!(session.send_feedback(&mut link, triggered).expect("send"));
    assert_eq!(
        feedback_kinds(&desktop_drain(&mut link)),
        [FeedbackKind::Triggered]
    );
}

fn internals_request_session(msgs: &[Message]) -> Option<u32> {
    msgs.iter().find_map(|m| match m {
        Message::InternalsRequest(request) => Some(request.session),
        _ => None,
    })
}

fn sample_internals() -> kivori_model::internals::DeviceInternals {
    kivori_model::internals::DeviceInternals {
        app_image_bytes: Some(700_000),
        fps: Some(30),
        ..Default::default()
    }
}

#[test]
fn internals_are_requested_only_from_a_device_that_negotiated_them() {
    let (mut link, mut session, _manager, _orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link);
    assert!(!session.request_internals(&mut link).expect("request"));
    assert!(desktop_drain(&mut link).is_empty(), "nothing sent");

    let (mut link, mut session, _manager, _orch) = connect_with_caps(
        SendableState::Idle,
        Capabilities::MASCOT_INTERACTION.union(Capabilities::DEVICE_INTERNALS_V1),
    );
    let _ = desktop_drain(&mut link);
    assert!(session.request_internals(&mut link).expect("request"));
    assert!(internals_request_session(&desktop_drain(&mut link)).is_some());
}

#[test]
fn the_latest_internals_reply_is_cached_and_forgotten_with_the_connection() {
    let (mut link, mut session, mut manager, mut orch) = connect_with_caps(
        SendableState::Idle,
        Capabilities::MASCOT_INTERACTION.union(Capabilities::DEVICE_INTERNALS_V1),
    );
    let received_at = std::time::Instant::now();
    session.set_host_instant(received_at);
    let _ = desktop_drain(&mut link);
    assert_eq!(session.internals(), None);
    assert_eq!(session.internals_received_at(), None);
    assert!(session.request_internals(&mut link).expect("request"));
    let nonce = internals_request_session(&desktop_drain(&mut link)).expect("request sent");

    // A snapshot for some other session is dropped unseen.
    device_push(
        &mut link,
        &Message::Internals(kivori_protocol::InternalsReport {
            session: nonce.wrapping_add(1),
            internals: sample_internals(),
        }),
        wire_version(),
        1,
    );
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert_eq!(session.internals(), None);
    assert_eq!(session.internals_received_at(), None);

    device_push(
        &mut link,
        &Message::Internals(kivori_protocol::InternalsReport {
            session: nonce,
            internals: sample_internals(),
        }),
        wire_version(),
        2,
    );
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert_eq!(session.internals(), Some(sample_internals()));
    assert_eq!(session.internals_received_at(), Some(received_at));

    // Equal values are still a new measurement; the injected receipt clock is authoritative.
    let next_at = received_at + std::time::Duration::from_secs(2);
    session.set_host_instant(next_at);
    device_push(
        &mut link,
        &Message::Internals(kivori_protocol::InternalsReport {
            session: nonce,
            internals: sample_internals(),
        }),
        wire_version(),
        3,
    );
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert_eq!(session.internals_received_at(), Some(next_at));

    // The device ends the session: the snapshot describes a connection that is gone.
    device_push(
        &mut link,
        &Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        wire_version(),
        4,
    );
    let _ = session.pump(&mut link, &mut manager, &mut orch);
    assert_eq!(session.internals(), None);
    assert_eq!(session.internals_received_at(), None);
}

#[test]
fn an_unnegotiated_internals_reply_is_ignored() {
    let (mut link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link);
    device_push(
        &mut link,
        &Message::Internals(kivori_protocol::InternalsReport {
            session: 1,
            internals: sample_internals(),
        }),
        wire_version(),
        1,
    );
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert_eq!(session.internals(), None);
    assert_eq!(session.internals_received_at(), None);
}

fn buddy_settings(msgs: &[Message]) -> Vec<kivori_protocol::BuddySettings> {
    msgs.iter()
        .filter_map(|m| match m {
            Message::BuddySettings(settings) => Some(*settings),
            _ => None,
        })
        .collect()
}

#[test]
fn buddy_settings_go_only_to_a_device_that_runs_idle_life() {
    let (mut link, mut session, _manager, _orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link);
    assert!(!session.supports_idle_life());
    assert!(!session
        .send_buddy_settings(&mut link, MascotPersonality::Calm, true, 3)
        .expect("send"));
    assert!(desktop_drain(&mut link).is_empty(), "nothing sent");

    let (mut link, mut session, _manager, _orch) = connect_with_caps(
        SendableState::Idle,
        Capabilities::MASCOT_INTERACTION.union(Capabilities::IDLE_LIFE_V1),
    );
    let _ = desktop_drain(&mut link);
    assert!(session.supports_idle_life());
    assert!(session
        .send_buddy_settings(&mut link, MascotPersonality::Calm, false, 2)
        .expect("send"));
    let sent = buddy_settings(&desktop_drain(&mut link));
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].session, session.current_session().unwrap());
    assert_eq!(sent[0].personality, MascotPersonality::Calm);
    assert!(!sent[0].self_play);
    assert_eq!(sent[0].brightness, 2);
}

#[test]
fn mic_state_goes_only_to_a_device_that_negotiated_it() {
    use kivori_model::desk::MicState;
    let state = MicState {
        muted: Some(true),
        in_use: None,
    };
    let mics = |msgs: &[Message]| -> Vec<kivori_protocol::MicStateUpdate> {
        msgs.iter()
            .filter_map(|m| match m {
                Message::MicState(update) => Some(*update),
                _ => None,
            })
            .collect()
    };
    let (mut link, mut session, _manager, _orch) = connect(SendableState::Idle);
    let _ = desktop_drain(&mut link);
    assert!(!session.send_mic_state(&mut link, state).expect("send"));
    assert!(desktop_drain(&mut link).is_empty(), "nothing sent");

    let (mut link, mut session, _manager, _orch) = connect_with_caps(
        SendableState::Idle,
        Capabilities::MASCOT_INTERACTION.union(Capabilities::MIC_STATE_V1),
    );
    let _ = desktop_drain(&mut link);
    assert!(session.send_mic_state(&mut link, state).expect("send"));
    let sent = mics(&desktop_drain(&mut link));
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].session, session.current_session().unwrap());
    assert_eq!(sent[0].mic, state);
}

fn identity_message(session: u32, product: u16, board: u16) -> Message {
    Message::Identity(kivori_protocol::Identity {
        session,
        product,
        board,
        hw_rev: Some(2),
        source: kivori_protocol::IdentitySource::Chip,
    })
}

#[test]
fn a_device_without_the_identity_capability_is_the_legacy_pala() {
    use kivori_desktop::identity::{DeviceIdentity, IdentitySource};
    let (_link, session, _manager, _orch) = connect(SendableState::Idle);
    let identity = session.identity().expect("legacy identity");
    assert_eq!(identity, DeviceIdentity::legacy());
    assert_eq!(identity.source, IdentitySource::Legacy);
    assert_eq!(identity.hw_rev, None);
    assert_eq!(identity.label(), "Kivori Pip on C3 SuperMini");
}

#[test]
fn identity_is_pending_until_the_message_arrives_then_scoped_to_the_connection() {
    use kivori_desktop::identity::IdentitySource;
    let (mut link, mut session, mut manager, mut orch) = connect_with_caps(
        SendableState::Idle,
        Capabilities::MASCOT_INTERACTION.union(Capabilities::PRODUCT_IDENTITY_V1),
    );
    assert_eq!(session.identity(), None, "negotiated but not yet reported");
    let nonce = session.current_session().expect("session");

    // An identity for another session is dropped unseen.
    device_push(
        &mut link,
        &identity_message(nonce.wrapping_add(1), 1, 1),
        wire_version(),
        1,
    );
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert_eq!(session.identity(), None);

    device_push(&mut link, &identity_message(nonce, 1, 1), wire_version(), 2);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    let identity = session.identity().expect("identity");
    assert_eq!(
        (identity.hw_rev, identity.source),
        (Some(2), IdentitySource::Chip)
    );
    assert!(identity.resolve().is_some());

    device_push(
        &mut link,
        &Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        wire_version(),
        3,
    );
    let _ = session.pump(&mut link, &mut manager, &mut orch);
    assert_eq!(session.identity(), None, "gone with the connection");
}

#[test]
fn an_unknown_product_is_never_guessed_and_has_no_effective_support() {
    let (mut link, mut session, mut manager, mut orch) = connect_with_caps(
        SendableState::Idle,
        Capabilities::MASCOT_INTERACTION.union(Capabilities::PRODUCT_IDENTITY_V1),
    );
    let nonce = session.current_session().expect("session");
    device_push(&mut link, &identity_message(nonce, 9, 1), wire_version(), 1);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    let identity = session.identity().expect("reported identity");
    assert!(identity.resolve().is_none());
    assert_eq!(identity.product_name(), "Unknown product (9)");
    assert!(
        !session.supports_mascot_interaction(),
        "no feature is driven"
    );
}

#[test]
fn an_unnegotiated_identity_message_is_ignored() {
    let (mut link, mut session, mut manager, mut orch) = connect(SendableState::Idle);
    let nonce = session.current_session().expect("session");
    device_push(&mut link, &identity_message(nonce, 9, 9), wire_version(), 1);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert_eq!(
        session.identity().map(|i| i.product),
        Some(1),
        "still the legacy identity"
    );
}

#[test]
fn effective_support_waits_for_identity_and_resyncs_the_latest_desired_state() {
    let caps = Capabilities::PRODUCT_IDENTITY_V1
        .union(Capabilities::MASCOT_INTERACTION)
        .union(Capabilities::PHYSICAL_INPUT_V1)
        .union(Capabilities::DEVICE_INTERNALS_V1);
    let (mut link, mut session, mut manager, mut orch) =
        connect_with_caps(SendableState::Idle, caps);
    let nonce = session.current_session().expect("session");
    assert_eq!(session.negotiated_caps(), caps);
    assert_eq!(session.effective_caps(), Capabilities::NONE);
    assert!(!session.supports_mascot_interaction());
    assert!(!session.request_internals(&mut link).expect("request"));
    session
        .set_desired(&mut link, &manager, &mut orch, SendableState::Busy)
        .expect("desired");
    assert!(desktop_drain(&mut link)
        .iter()
        .all(|message| matches!(message, Message::Ready(_))));

    device_push(&mut link, &detent_event(nonce), wire_version(), 1);
    device_push(&mut link, &identity_message(nonce, 1, 1), wire_version(), 2);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert!(
        session.take_input_events().is_empty(),
        "early input must never execute later"
    );
    assert_eq!(session.effective_caps(), caps);
    assert!(session.supports_mascot_interaction());
    assert!(
        matches!(desktop_drain(&mut link).as_slice(), [Message::SetState(state)] if state.desired == SendableState::Busy)
    );

    // Identity cannot change underneath a live session, even with the same nonce.
    device_push(&mut link, &identity_message(nonce, 9, 1), wire_version(), 3);
    device_push(&mut link, &detent_event(nonce), wire_version(), 4);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert_eq!(session.identity().expect("identity").product, 1);
    assert_eq!(session.take_input_events().len(), 1);
    assert!(desktop_drain(&mut link).is_empty());
    session
        .close(&mut link, ByeReason::Shutdown)
        .expect("close");
    assert_eq!(session.effective_caps(), Capabilities::NONE);
}

#[test]
fn unknown_and_identity_only_firmware_never_receive_normal_state() {
    for (product, caps) in [
        (
            9,
            Capabilities::PRODUCT_IDENTITY_V1
                .union(Capabilities::MASCOT_INTERACTION)
                .union(Capabilities::PHYSICAL_INPUT_V1),
        ),
        (1, Capabilities::PRODUCT_IDENTITY_V1),
    ] {
        let (mut link, mut session, mut manager, mut orch) =
            connect_with_caps(SendableState::Idle, caps);
        let nonce = session.current_session().expect("session");
        desktop_drain(&mut link);
        device_push(
            &mut link,
            &identity_message(nonce, product, 1),
            wire_version(),
            1,
        );
        device_push(&mut link, &detent_event(nonce), wire_version(), 2);
        session
            .pump(&mut link, &mut manager, &mut orch)
            .expect("pump");
        session
            .set_desired(&mut link, &manager, &mut orch, SendableState::Busy)
            .expect("desired");
        assert!(desktop_drain(&mut link).is_empty());
        assert!(session.take_input_events().is_empty());
        assert!(!session.supports_mascot_interaction());
        assert_eq!(
            session.negotiated_caps(),
            caps,
            "negotiation remains available for diagnostics"
        );
        assert_eq!(
            session.effective_caps(),
            if product == 9 {
                Capabilities::NONE
            } else {
                Capabilities::PRODUCT_IDENTITY_V1
            }
        );
    }
}

#[test]
fn every_feature_writer_is_inert_for_pending_and_unknown_identity() {
    use kivori_model::desk::{
        ActionFeedback, ActionKind, ControlLabels, DeskStatus, FeedbackKind, MicState,
    };
    use kivori_protocol::Presentation;
    let caps = SessionConfig::default().capabilities;
    let (mut link, mut session, mut manager, mut orch) =
        connect_with_caps(SendableState::Idle, caps);
    let nonce = session.current_session().expect("session");
    desktop_drain(&mut link);
    for unknown in [false, true] {
        if unknown {
            device_push(&mut link, &identity_message(nonce, 9, 1), wire_version(), 1);
            session
                .pump(&mut link, &mut manager, &mut orch)
                .expect("pump");
        }
        assert!(!session
            .send_status(&mut link, DeskStatus::UNKNOWN)
            .expect("status"));
        assert!(!session.send_media_info(&mut link, None).expect("media"));
        assert!(!session
            .send_mic_state(&mut link, MicState::UNKNOWN)
            .expect("mic"));
        assert!(!session
            .send_control_labels(&mut link, ControlLabels::default())
            .expect("labels"));
        assert!(!session
            .send_feedback(
                &mut link,
                ActionFeedback {
                    action: ActionKind::PlayPause,
                    kind: FeedbackKind::Unverified
                }
            )
            .expect("feedback"));
        assert!(!session.request_internals(&mut link).expect("internals"));
        assert!(!session
            .send_buddy_settings(&mut link, MascotPersonality::Calm, true, 3)
            .expect("settings"));
        assert!(!session
            .play_mascot_action(
                &mut link,
                &manager,
                MascotAction::Greet,
                MascotPersonality::Calm,
                1
            )
            .expect("action"));
        session
            .send_presentation(
                &mut link,
                Presentation {
                    session: nonce,
                    revision: 1,
                    primary: kivori_model::presentation::PrimaryState::Idle,
                    value: None,
                    transient_ms: 0,
                },
            )
            .expect("presentation");
        assert!(
            desktop_drain(&mut link).is_empty(),
            "no feature bytes for pending/unknown identity"
        );
    }
}

#[test]
fn reactions_require_current_identity_and_rejected_requests_are_never_replayed() {
    let caps = Capabilities::PRODUCT_IDENTITY_V1.union(Capabilities::MASCOT_INTERACTION);
    let (mut link, mut session, mut manager, mut orch) =
        connect_with_caps(SendableState::Idle, caps);
    let nonce = session.current_session().unwrap();
    desktop_drain(&mut link);
    assert!(!session
        .play_mascot_action(
            &mut link,
            &manager,
            MascotAction::Greet,
            MascotPersonality::Calm,
            41,
        )
        .unwrap());
    assert!(link.to_device.is_empty());

    device_push(&mut link, &identity_message(nonce, 1, 1), wire_version(), 1);
    session.pump(&mut link, &mut manager, &mut orch).unwrap();
    let resync = desktop_drain(&mut link);
    assert!(matches!(resync.as_slice(), [Message::SetState(_)]));
    assert!(session
        .play_mascot_action(
            &mut link,
            &manager,
            MascotAction::Greet,
            MascotPersonality::Calm,
            42,
        )
        .unwrap());
    assert!(matches!(
        desktop_drain(&mut link).as_slice(),
        [Message::PlayMascotAction(action)]
            if action.action == MascotAction::Greet && action.seed == 42
    ));

    session.close(&mut link, ByeReason::Shutdown).unwrap();
    desktop_drain(&mut link);
    manager.apply(ManagerEvent::PortRemoved);
    session.open(&mut link, &mut manager).unwrap();
    let nonce = hello_nonce(&desktop_drain(&mut link));
    let Message::HelloAck(mut ack) = device_ack(nonce) else {
        unreachable!()
    };
    ack.device_caps = caps;
    device_push(&mut link, &Message::HelloAck(ack), wire_version(), 0);
    session.pump(&mut link, &mut manager, &mut orch).unwrap();
    desktop_drain(&mut link);
    assert_eq!(session.identity(), None);
    assert_eq!(session.effective_caps(), Capabilities::NONE);
    assert!(!session
        .play_mascot_action(
            &mut link,
            &manager,
            MascotAction::Greet,
            MascotPersonality::Calm,
            43,
        )
        .unwrap());
    assert!(link.to_device.is_empty());

    // A known product with an unknown board cannot inherit the previous session's support.
    device_push(
        &mut link,
        &identity_message(nonce, 1, 99),
        wire_version(),
        1,
    );
    session.pump(&mut link, &mut manager, &mut orch).unwrap();
    assert!(!session
        .play_mascot_action(
            &mut link,
            &manager,
            MascotAction::Greet,
            MascotPersonality::Calm,
            44,
        )
        .unwrap());
    assert!(link.to_device.is_empty());
}

#[test]
fn runtime_display_and_flash_admission_follow_current_session_support() {
    use kivori_desktop::firmware::FlashFailure;
    use kivori_desktop::firmware_bundle::FirmwareBundle;
    use kivori_desktop::runtime::device_task::{admits_display_mode, select_ordinary_flash_image};

    let mut bytes = vec![0; 52];
    bytes[..6].copy_from_slice(b"\x7fELF\x01\x01");
    bytes[18] = 243;
    bytes.extend_from_slice(b"KIVORI-FW-VERSION:0.1.0\0KIVORI-FW-IDENTITY:p=1;b=1;v=0.1.0;m=0.1\0");
    let images = FirmwareBundle::parse(&[&bytes]).unwrap();
    let empty = FirmwareBundle::parse(&[]).unwrap();
    let display_caps = Capabilities::PRODUCT_IDENTITY_V1.union(Capabilities::DESK_STATUS_V1);
    for (identity, caps, display, flash) in [
        (None, display_caps, false, Err(FlashFailure::Unknown)),
        (
            Some((9, 1)),
            display_caps,
            false,
            Err(FlashFailure::ImageUnavailable),
        ),
        (
            Some((1, 99)),
            display_caps,
            false,
            Err(FlashFailure::ImageUnavailable),
        ),
        (
            Some((1, 1)),
            Capabilities::PRODUCT_IDENTITY_V1,
            false,
            Ok(()),
        ),
        (Some((1, 1)), display_caps, true, Ok(())),
        // Legacy state sync is usable without advertising a live display feature.
        (None, Capabilities::NONE, false, Ok(())),
    ] {
        let (mut link, mut session, mut manager, mut orch) =
            connect_with_caps(SendableState::Idle, caps);
        let nonce = session.current_session().unwrap();
        desktop_drain(&mut link);
        if let Some((product, board)) = identity {
            device_push(
                &mut link,
                &identity_message(nonce, product, board),
                wire_version(),
                1,
            );
            session.pump(&mut link, &mut manager, &mut orch).unwrap();
        }
        assert_eq!(
            admits_display_mode(manager.state(), session.effective_caps(), false),
            display
        );
        assert!(!admits_display_mode(
            manager.state(),
            session.effective_caps(),
            true
        ));
        let selected = select_ordinary_flash_image(manager.state(), session.identity(), &images);
        assert_eq!(
            selected.map(|image| {
                assert_eq!(image.bytes(), bytes);
            }),
            flash
        );
        if flash.is_ok() {
            assert_eq!(
                select_ordinary_flash_image(manager.state(), session.identity(), &empty)
                    .unwrap_err(),
                FlashFailure::ImageUnavailable
            );
        }
        for state in [
            ConnectionState::Disconnected,
            ConnectionState::Connecting,
            ConnectionState::Incompatible,
            ConnectionState::Error,
        ] {
            assert!(!admits_display_mode(state, display_caps, false));
            assert_eq!(
                select_ordinary_flash_image(state, session.identity(), &images).unwrap_err(),
                FlashFailure::Unknown
            );
        }
    }
}

#[test]
fn desk_session_starts_after_identity_and_invalidates_cached_output() {
    use kivori_desktop::desk::DeskRuntime;
    use kivori_desktop::platform::system::NoSystemProbe;
    use kivori_desktop::platform::{
        FakeAppVolumeBackend, FakeForeground, FakeInputSynth, FakeMediaObserver, FakeMicBackend,
        FakeVolumeBackend, OsServices,
    };
    use kivori_desktop::runtime::device_task::{synchronize_session, RotaryPipeline};
    use std::sync::Arc;
    use std::time::Duration;

    for (product, caps, starts_at_ready, starts_at_identity) in [
        (1, Capabilities::DESK_STATUS_V1, true, true),
        (
            1,
            Capabilities::DESK_STATUS_V1.union(Capabilities::PRODUCT_IDENTITY_V1),
            false,
            true,
        ),
        (
            9,
            Capabilities::DESK_STATUS_V1.union(Capabilities::PRODUCT_IDENTITY_V1),
            false,
            false,
        ),
        (1, Capabilities::PRODUCT_IDENTITY_V1, false, false),
    ] {
        let (mut link, mut session, mut manager, mut orch) =
            connect_with_caps(SendableState::Idle, caps);
        let volume = Arc::new(FakeVolumeBackend::new(20));
        let mut rotary = RotaryPipeline::new(&*volume);
        let mut desk = DeskRuntime::new(OsServices {
            volume,
            app_volume: Arc::new(FakeAppVolumeBackend::new()),
            mic: Arc::new(FakeMicBackend::new()),
            synth: Arc::new(FakeInputSynth::new(Ok(()))),
            media: Arc::new(FakeMediaObserver::default()),
            system: Box::new(NoSystemProbe),
            clock: || None,
            foreground: Arc::new(FakeForeground::default()),
        });
        let mut active = None;
        synchronize_session(&mut active, &session, &manager, &mut rotary, &mut desk);
        assert_eq!(active.is_some(), starts_at_ready);
        // Polling before identity consumes cached output; starting the usable session must resend it.
        let _ = desk.tick(Duration::ZERO, &mut |_| {});
        let cached = desk.tick(Duration::from_millis(1), &mut |_| {});
        assert!(cached.status.is_none());
        assert!(cached.media_info.is_none());
        let nonce = session.current_session().expect("session");
        device_push(
            &mut link,
            &identity_message(nonce, product, 1),
            wire_version(),
            1,
        );
        session
            .pump(&mut link, &mut manager, &mut orch)
            .expect("pump");
        synchronize_session(&mut active, &session, &manager, &mut rotary, &mut desk);
        assert_eq!(active.is_some(), starts_at_identity);
        let output = desk.tick(Duration::from_millis(2), &mut |_| {});
        let newly_started = !starts_at_ready && starts_at_identity;
        assert_eq!(output.status.is_some(), newly_started);
        assert_eq!(output.media_info.is_some(), newly_started);
        assert_eq!(output.controls.is_some(), newly_started);
    }
}

#[test]
fn input_requires_the_specific_control_and_gesture_capabilities() {
    for (extra, control, kind, accepted) in [
        (
            Capabilities::NONE,
            ControlId::Rotary,
            InputKind::GestureStarted,
            true,
        ),
        (
            Capabilities::NONE,
            ControlId::Button,
            InputKind::Press,
            false,
        ),
        (
            Capabilities::BUTTON_INPUT_V1,
            ControlId::Button,
            InputKind::Press,
            true,
        ),
        (
            Capabilities::BUTTON_INPUT_V1,
            ControlId::Button,
            InputKind::DoublePress,
            false,
        ),
        (
            Capabilities::BUTTON_INPUT_V1.union(Capabilities::DOUBLE_PRESS_V1),
            ControlId::Button,
            InputKind::DoublePress,
            true,
        ),
        (
            Capabilities::NONE,
            ControlId::ContextButton(0),
            InputKind::Press,
            false,
        ),
        (
            Capabilities::CONTEXT_BUTTONS_V1,
            ControlId::ContextButton(0),
            InputKind::Hold,
            true,
        ),
    ] {
        let (mut link, mut session, mut manager, mut orch) = connect_with_caps(
            SendableState::Idle,
            Capabilities::PHYSICAL_INPUT_V1.union(extra),
        );
        let nonce = session.current_session().expect("session");
        device_push(
            &mut link,
            &Message::InputEvent(InputEvent {
                session: nonce,
                gesture_id: 1,
                control,
                kind,
                device_ms: 0,
            }),
            wire_version(),
            1,
        );
        session
            .pump(&mut link, &mut manager, &mut orch)
            .expect("pump");
        assert_eq!(
            session.take_input_events().len(),
            usize::from(accepted),
            "{control:?}/{kind:?}"
        );
    }
}

#[test]
fn preview_mirror_never_stages_state_for_an_unresolved_or_later_device() {
    let caps = Capabilities::PRODUCT_IDENTITY_V1.union(Capabilities::DESK_STATUS_V1);
    let (mut link, mut session, mut manager, mut orch) =
        connect_with_caps(SendableState::Idle, caps);
    let nonce = session.current_session().expect("session");
    desktop_drain(&mut link);
    session
        .mirror_desired(&mut link, &manager, &mut orch, SendableState::Busy)
        .expect("pending mirror");
    assert_eq!(orch.user_desired(), SendableState::Idle);
    assert!(desktop_drain(&mut link).is_empty());
    device_push(&mut link, &identity_message(nonce, 1, 1), wire_version(), 1);
    session
        .pump(&mut link, &mut manager, &mut orch)
        .expect("pump");
    assert_eq!(
        set_state_desired(&desktop_drain(&mut link)),
        Some(SendableState::Idle)
    );
    session
        .mirror_desired(&mut link, &manager, &mut orch, SendableState::Happy)
        .expect("live mirror");
    assert_eq!(
        set_state_desired(&desktop_drain(&mut link)),
        Some(SendableState::Happy)
    );
    session
        .close(&mut link, ByeReason::Shutdown)
        .expect("close");
    desktop_drain(&mut link);
    session
        .mirror_desired(&mut link, &manager, &mut orch, SendableState::Busy)
        .expect("offline mirror");
    assert_eq!(orch.user_desired(), SendableState::Happy);
    assert!(desktop_drain(&mut link).is_empty());
    for (product, caps) in [(9, caps), (1, Capabilities::PRODUCT_IDENTITY_V1)] {
        let (mut link, mut session, mut manager, mut orch) =
            connect_with_caps(SendableState::Idle, caps);
        let nonce = session.current_session().expect("session");
        desktop_drain(&mut link);
        device_push(
            &mut link,
            &identity_message(nonce, product, 1),
            wire_version(),
            1,
        );
        session
            .pump(&mut link, &mut manager, &mut orch)
            .expect("pump");
        session
            .mirror_desired(&mut link, &manager, &mut orch, SendableState::Busy)
            .expect("unsupported mirror");
        assert_eq!(orch.user_desired(), SendableState::Idle);
        assert!(desktop_drain(&mut link).is_empty());
    }
}
