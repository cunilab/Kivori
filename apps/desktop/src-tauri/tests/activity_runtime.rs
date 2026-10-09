//! Runtime activity producers: host-side coverage without Tauri or serial hardware.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::io::{Error, ErrorKind};

use kivori_desktop::activity::{
    ActivityEventKind, ActivityMetadata, ActivityOutcome, RuntimeActivityPlanner,
    RuntimeActivityRequest,
};
use kivori_desktop::device::fsm::{ConnectionManager, ManagerEvent};
use kivori_desktop::device::session::{Session, SessionConfig};
use kivori_desktop::device::transport::SerialLink;
use kivori_desktop::firmware::FlashWorkflow;
use kivori_desktop::orchestrator::Orchestrator;
use kivori_desktop::runtime::device_task::{
    observe_connection_transition, plan_device_request, recover_discovery_open_failure,
    recover_link, ConnectionDeadlines, LinkRecovery,
};
use kivori_desktop::runtime::state::DeviceCommand;
use kivori_model::{CompanionState, ConnectionState, ProtocolVersion};
use kivori_protocol::{
    encode_message, Diagnostic, ErrorCategory, MascotActionApplied, Message, PROTOCOL_MAJOR,
    PROTOCOL_MINOR,
};

#[derive(Default)]
struct FakeLink {
    incoming: VecDeque<u8>,
    outgoing: VecDeque<u8>,
}

impl SerialLink for FakeLink {
    type Error = Infallible;

    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Self::Error> {
        let mut count = 0;
        while count < bytes.len() {
            let Some(byte) = self.incoming.pop_front() else {
                break;
            };
            bytes[count] = byte;
            count += 1;
        }
        Ok(count)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        self.outgoing.extend(bytes.iter().copied());
        Ok(bytes.len())
    }
}

struct FailingHelloLink;

impl SerialLink for FailingHelloLink {
    type Error = Error;

    fn read(&mut self, _bytes: &mut [u8]) -> Result<usize, Self::Error> {
        Ok(0)
    }

    fn write(&mut self, _bytes: &[u8]) -> Result<usize, Self::Error> {
        Err(Error::new(
            ErrorKind::BrokenPipe,
            "test hello write failure",
        ))
    }
}

fn push(link: &mut FakeLink, message: Message, sequence: u16) {
    let mut wire = heapless::Vec::new();
    encode_message(
        &message,
        ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
        sequence,
        &mut wire,
    )
    .expect("test message encodes");
    link.incoming.extend(wire);
}

#[test]
fn opening_a_session_queues_handshake_activity() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();

    session.open(&mut link, &mut manager).expect("open");

    let kinds: Vec<_> = session
        .drain_activity()
        .into_iter()
        .map(|entry| entry.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            ActivityEventKind::ConnectionOpened,
            ActivityEventKind::HandshakeStarted
        ]
    );
}

#[test]
fn safe_device_diagnostics_and_errors_are_queued_in_wire_order() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    push(
        &mut link,
        Message::Diagnostic(Diagnostic {
            category: ErrorCategory::Framing,
            code: 1,
        }),
        0,
    );
    push(
        &mut link,
        Message::Diagnostic(Diagnostic {
            category: ErrorCategory::Checksum,
            code: 2,
        }),
        1,
    );
    push(
        &mut link,
        Message::Diagnostic(Diagnostic {
            category: ErrorCategory::Version,
            code: 3,
        }),
        2,
    );
    push(
        &mut link,
        Message::Diagnostic(Diagnostic {
            category: ErrorCategory::BadPayload,
            code: 4,
        }),
        3,
    );
    push(
        &mut link,
        Message::Diagnostic(Diagnostic {
            category: ErrorCategory::Framing,
            code: 5,
        }),
        4,
    );
    push(
        &mut link,
        Message::Diagnostic(Diagnostic {
            category: ErrorCategory::Io,
            code: 6,
        }),
        5,
    );
    push(
        &mut link,
        Message::Diagnostic(Diagnostic {
            category: ErrorCategory::Io,
            code: 7,
        }),
        6,
    );
    push(
        &mut link,
        Message::Error(kivori_protocol::ErrorReport {
            category: ErrorCategory::Busy,
            code: 9,
        }),
        7,
    );

    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");

    assert_eq!(
        session
            .drain_activity()
            .into_iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>(),
        [
            ActivityEventKind::DeviceDiagnosticFraming,
            ActivityEventKind::DeviceDiagnosticChecksum,
            ActivityEventKind::DeviceDiagnosticVersion,
            ActivityEventKind::DeviceDiagnosticPayload,
            ActivityEventKind::DeviceSequenceGap,
            ActivityEventKind::DeviceDisplayFault,
            ActivityEventKind::DeviceLinkLost,
            ActivityEventKind::DeviceBusy,
        ]
    );
}

#[test]
fn malformed_and_gapped_frames_are_reported_but_duplicates_and_healthy_traffic_are_suppressed() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    link.incoming.extend([0x02, 0xff, 0x00]);
    push(
        &mut link,
        Message::Pong(kivori_protocol::Pong {
            t_ms_echo: 1,
            uptime_ms: 2,
        }),
        3,
    );
    push(
        &mut link,
        Message::Health(kivori_protocol::Health { free_bytes: 1024 }),
        5,
    );
    // The duplicate must repeat the most recently accepted sequence. Reusing 3 here would be a
    // forward sequence gap after 5, not a duplicate under the protocol's tracker.
    push(
        &mut link,
        Message::Health(kivori_protocol::Health { free_bytes: 1024 }),
        5,
    );

    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");

    assert_eq!(
        session
            .drain_activity()
            .into_iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>(),
        [
            ActivityEventKind::ProtocolMalformedFrame,
            ActivityEventKind::ProtocolSequenceGap
        ]
    );
}

#[test]
fn action_acknowledgement_and_changed_state_are_queued_in_wire_order() {
    let mut link = FakeLink::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    push(
        &mut link,
        Message::MascotActionApplied(MascotActionApplied {
            action: kivori_model::MascotAction::Pet,
            personality: kivori_model::MascotPersonality::Cozy,
            seed: 7,
            applied_at_ms: 120,
        }),
        0,
    );
    push(
        &mut link,
        Message::StateReport(kivori_protocol::StateReport {
            reported: CompanionState::Happy,
            elapsed_ms: 121,
        }),
        1,
    );
    push(
        &mut link,
        Message::StateReport(kivori_protocol::StateReport {
            reported: CompanionState::Happy,
            elapsed_ms: 122,
        }),
        2,
    );

    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump");

    assert_eq!(
        session
            .drain_activity()
            .into_iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>(),
        [
            ActivityEventKind::SocialActionApplied,
            ActivityEventKind::DeviceStateObserved
        ]
    );
}

#[test]
fn planner_retains_failure_retry_and_recovery_across_intervening_states() {
    let mut planner = RuntimeActivityPlanner::new();
    assert_eq!(
        planner.attempt().kind,
        ActivityEventKind::ConnectionAttempted
    );
    let failure = planner.failure(ActivityEventKind::HeartbeatTimedOut, true);
    assert_eq!(
        failure.iter().map(|entry| entry.kind).collect::<Vec<_>>(),
        [
            ActivityEventKind::HeartbeatTimedOut,
            ActivityEventKind::ConnectionRetryScheduled
        ]
    );
    assert_eq!(
        planner.recovered().map(|entry| entry.kind),
        Some(ActivityEventKind::ConnectionRecovered)
    );
}

#[test]
fn serial_open_failure_enters_backoff_and_records_one_specific_recovery() {
    let mut planner = RuntimeActivityPlanner::new();
    let mut manager = ConnectionManager::new();
    let mut link = None;
    let mut port = None;
    let mut retry = None;
    let mut deadlines = ConnectionDeadlines::new();
    let flash = FlashWorkflow::new(true, 512);
    let mut activity = Vec::new();

    recover_discovery_open_failure(
        &mut planner,
        &mut manager,
        LinkRecovery::new(&mut link, &mut port, &mut retry, &mut deadlines, &flash),
        |observation| activity.push(observation.kind),
    );

    assert_eq!(manager.state(), ConnectionState::Error);
    assert_eq!(manager.retry_count(), 1);
    assert!(
        retry.is_some(),
        "a serial-open failure must back off before retrying"
    );
    assert_eq!(
        activity,
        [
            ActivityEventKind::ConnectionIoFailure,
            ActivityEventKind::ConnectionRetryScheduled,
        ]
    );
}

#[test]
fn hello_write_failure_enters_backoff_and_records_one_specific_recovery() {
    let mut planner = RuntimeActivityPlanner::new();
    let mut manager = ConnectionManager::new();
    let mut session = Session::new(SessionConfig::default());
    let mut failed_link = FailingHelloLink;
    assert!(session.open(&mut failed_link, &mut manager).is_err());
    assert_eq!(manager.state(), ConnectionState::Connecting);

    let mut link = None;
    let mut port = None;
    let mut retry = None;
    let mut deadlines = ConnectionDeadlines::new();
    let flash = FlashWorkflow::new(true, 512);
    let mut activity = Vec::new();

    recover_discovery_open_failure(
        &mut planner,
        &mut manager,
        LinkRecovery::new(&mut link, &mut port, &mut retry, &mut deadlines, &flash),
        |observation| activity.push(observation.kind),
    );

    assert_eq!(manager.state(), ConnectionState::Error);
    assert_eq!(manager.retry_count(), 1);
    assert!(
        retry.is_some(),
        "a hello-write failure must back off before retrying"
    );
    assert_eq!(
        activity,
        [
            ActivityEventKind::ConnectionIoFailure,
            ActivityEventKind::ConnectionRetryScheduled,
        ]
    );
}

#[test]
fn production_recovery_emits_retry_only_when_it_sets_a_retry_deadline() {
    let mut idle_planner = RuntimeActivityPlanner::new();
    let mut idle_manager = ConnectionManager::new();
    assert!(idle_manager.apply(ManagerEvent::PortOpened));
    let mut idle_link = None;
    let mut idle_port = Some("COM7".to_string());
    let mut idle_retry = None;
    let mut idle_deadlines = ConnectionDeadlines::new();
    let idle_flash = FlashWorkflow::new(true, 512);
    let mut idle_activity = Vec::new();

    recover_link(
        &mut idle_planner,
        &mut idle_manager,
        ManagerEvent::IoError,
        LinkRecovery::new(
            &mut idle_link,
            &mut idle_port,
            &mut idle_retry,
            &mut idle_deadlines,
            &idle_flash,
        ),
        |observation| idle_activity.push(observation.kind),
    );

    assert_eq!(
        idle_activity,
        [
            ActivityEventKind::ConnectionIoFailure,
            ActivityEventKind::ConnectionRetryScheduled,
        ]
    );
    assert!(idle_retry.is_some());

    let mut reconnect_planner = RuntimeActivityPlanner::new();
    let mut reconnect_manager = ConnectionManager::new();
    assert!(reconnect_manager.apply(ManagerEvent::PortOpened));
    let mut reconnect_link = None;
    let mut reconnect_port = Some("COM7".to_string());
    let mut reconnect_retry = None;
    let mut reconnect_deadlines = ConnectionDeadlines::new();
    let mut reconnect_flash = FlashWorkflow::new(true, 512);
    reconnect_flash.drain_activity();
    reconnect_flash
        .request(true, Some("COM7"), Some("deadbeef"))
        .unwrap();
    reconnect_flash.finish(Ok(()));
    let mut reconnect_activity = Vec::new();

    recover_link(
        &mut reconnect_planner,
        &mut reconnect_manager,
        ManagerEvent::IoError,
        LinkRecovery::new(
            &mut reconnect_link,
            &mut reconnect_port,
            &mut reconnect_retry,
            &mut reconnect_deadlines,
            &reconnect_flash,
        ),
        |observation| reconnect_activity.push(observation.kind),
    );

    assert_eq!(reconnect_activity, [ActivityEventKind::ConnectionIoFailure]);
    assert!(reconnect_retry.is_none());
}

#[test]
fn production_transition_adapter_keeps_recovery_pending_until_connected() {
    let mut planner = RuntimeActivityPlanner::new();
    let mut manager = ConnectionManager::new();
    assert!(manager.apply(ManagerEvent::PortOpened));
    let mut link = None;
    let mut port = Some("COM7".to_string());
    let mut retry = None;
    let mut deadlines = ConnectionDeadlines::new();
    let flash = FlashWorkflow::new(true, 512);
    recover_link(
        &mut planner,
        &mut manager,
        ManagerEvent::IoError,
        LinkRecovery::new(&mut link, &mut port, &mut retry, &mut deadlines, &flash),
        |_| {},
    );

    let mut previous = ConnectionState::Connecting;
    let mut transition_activity = Vec::new();
    for current in [
        ConnectionState::Error,
        ConnectionState::Disconnected,
        ConnectionState::Connecting,
        ConnectionState::Connected,
    ] {
        assert!(observe_connection_transition(
            &mut planner,
            &mut previous,
            current,
            |observation| transition_activity.push(observation.kind),
        ));
    }

    assert_eq!(
        transition_activity,
        [ActivityEventKind::ConnectionRecovered]
    );
}

#[test]
fn planner_builds_distinct_closed_request_metadata_in_order() {
    let planner = RuntimeActivityPlanner::new();
    let cue = kivori_protocol::PlayMascotAction {
        action: kivori_model::MascotAction::Pet,
        personality: kivori_model::MascotPersonality::Playful,
        seed: 44,
    };
    let requests = [
        plan_device_request(
            &planner,
            &DeviceCommand::SetDesired(kivori_model::SendableState::Busy),
            None,
        ),
        plan_device_request(
            &planner,
            &DeviceCommand::MirrorDesired(kivori_model::SendableState::Busy),
            None,
        ),
        // A saved config is announced by the device thread, and only if the buddy changed.
        plan_device_request(
            &planner,
            &DeviceCommand::ApplyConfig(Default::default()),
            None,
        ),
        planner.requests(RuntimeActivityRequest::CompanionConfiguration {
            personality: kivori_model::MascotPersonality::Playful,
            self_play: false,
        }),
        plan_device_request(
            &planner,
            &DeviceCommand::PlayMascotAction(kivori_model::MascotAction::Pet),
            Some(&cue),
        ),
        planner.requests(RuntimeActivityRequest::SocialAction {
            action: cue.action,
            personality: cue.personality,
            seed: cue.seed,
            autonomous: true,
        }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    assert_eq!(
        requests.iter().map(|entry| entry.kind).collect::<Vec<_>>(),
        [
            ActivityEventKind::StateRequested,
            ActivityEventKind::MirroredStateRequested,
            ActivityEventKind::PersonalityConfigured,
            ActivityEventKind::SelfPlayConfigured,
            ActivityEventKind::ManualSocialActionRequested,
            ActivityEventKind::AutonomousSocialActionRequested
        ]
    );
    assert!(matches!(
        requests[0].metadata,
        Some(ActivityMetadata::Action {
            state: Some(kivori_model::SendableState::Busy),
            ..
        })
    ));
    assert!(matches!(
        requests[4].metadata,
        Some(ActivityMetadata::Action {
            action: Some(kivori_model::MascotAction::Pet),
            seed: Some(44),
            autonomous: Some(false),
            ..
        })
    ));
}

#[test]
fn busy_and_timeout_have_closed_outcomes() {
    let log = kivori_desktop::activity::ActivityLog::new(2);
    assert_eq!(
        log.record(ActivityEventKind::DeviceBusy, None).outcome(),
        ActivityOutcome::Busy
    );
    assert_eq!(
        log.record(ActivityEventKind::DeviceTimedOut, None)
            .outcome(),
        ActivityOutcome::TimedOut
    );
}

#[test]
fn recovery_attributes_failures_to_the_port_and_handshake_timeouts_demote_it() {
    use core::time::Duration;
    use kivori_desktop::device::{CandidateRotator, PortCandidate, DEFAULT_ALLOWLIST};

    let ports = [
        PortCandidate::new("silent", Some(0x303A), Some(0x1001)),
        PortCandidate::new("kivori", Some(0x303A), Some(0x1001)),
    ];
    let mut rotator = CandidateRotator::new();
    let mut now = Duration::ZERO;
    let flash = FlashWorkflow::new(true, 512);
    let _ = rotator.next(&ports, DEFAULT_ALLOWLIST, now);
    for _ in 0..3 {
        // Pretend the silent port is retried each time it comes up.
        now += Duration::from_secs(30);
        let mut planner = RuntimeActivityPlanner::new();
        let mut manager = ConnectionManager::new();
        assert!(manager.apply(ManagerEvent::PortOpened));
        let mut link = None;
        let mut port = Some("silent".to_string());
        let mut retry = None;
        let mut deadlines = ConnectionDeadlines::new();
        recover_link(
            &mut planner,
            &mut manager,
            ManagerEvent::HandshakeTimeout,
            LinkRecovery::new(&mut link, &mut port, &mut retry, &mut deadlines, &flash)
                .with_rotator(&mut rotator, now),
            |_| {},
        );
        assert!(port.is_none());
    }
    // Both ports are enumerated; the silent one is demoted and backing off, Kivori is next.
    let picked = rotator.next(&ports, DEFAULT_ALLOWLIST, now);
    assert_eq!(picked.as_deref(), Some("kivori"));
    now += Duration::from_secs(60);
    assert_eq!(
        rotator.next(&ports, DEFAULT_ALLOWLIST, now).as_deref(),
        Some("kivori")
    );
}
