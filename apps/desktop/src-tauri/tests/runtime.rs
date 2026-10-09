//! Host-testable runtime coverage (T123): DTO projections + redaction, the sendable production
//! boundary, the activity ring, and AppState construction/shutdown. The Tauri window/tray behaviour
//! and the serial device loop are exercised manually (no GUI / no hardware in host CI).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kivori_desktop::activity::{ActivityEventKind, ActivityLog, ActivityMetadata};
use kivori_desktop::device::connection::ConnectedDevice;
use kivori_desktop::device::fsm::{ConnectionManager, ManagerEvent};
use kivori_desktop::ipc::dto;
use kivori_desktop::orchestrator::Orchestrator;
use kivori_desktop::platform::host_events::HostPresence;
use kivori_desktop::runtime::state::{AppState, DeviceCommand};
use kivori_model::{
    CompanionState, ConnectionState, MascotAction, MascotPersonality, ProtocolVersion,
    SendableState,
};
use kivori_protocol::FirmwareVersion;

fn connected_device() -> ConnectedDevice {
    ConnectedDevice {
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 4,
            patch: 2,
        },
        protocol_version: ProtocolVersion::new(1, 0),
        device_id_hash_short: "deadbeef".to_string(),
    }
}

#[test]
fn initial_status_is_disconnected_idle() {
    let dto = dto::initial_status();
    assert_eq!(dto.connection, "disconnected");
    assert_eq!(dto.desired, "idle");
    assert_eq!(dto.reported, None);
    assert!(dto.device.is_none());
    assert_eq!(dto.retry_count, 0);
    assert!(!dto.mascot_interaction);
}

#[test]
fn connected_status_projects_all_three_axes() {
    let mut manager = ConnectionManager::new();
    manager.apply(ManagerEvent::PortOpened);
    manager.apply(ManagerEvent::HandshakeOk(connected_device()));
    let mut orchestrator = Orchestrator::new();
    orchestrator.set_desired(SendableState::Busy);

    let dto = dto::connection_status(
        &manager,
        &orchestrator,
        Some(CompanionState::Happy),
        true,
        None,
        7,
        HostPresence::Active,
    );
    assert_eq!(dto.connection, "connected");
    assert_eq!(dto.desired, "busy");
    assert_eq!(dto.reported.as_deref(), Some("happy"));
    assert!(dto.mascot_interaction);
    assert_eq!(dto.connection_generation, 7);
    let device = dto.device.expect("device present when connected");
    assert_eq!(device.firmware_version, "1.4.2");
    assert_eq!(device.device_id_hash_short, "deadbeef");
    assert_eq!(device.protocol_version.major, 1);
}

#[test]
fn host_presence_projects_and_the_override_does_not_change_the_users_choice() {
    let mut orchestrator = Orchestrator::new();
    orchestrator.set_desired(SendableState::Busy);
    orchestrator.set_host_override(Some(SendableState::Sleeping));
    let dto = dto::connection_status(
        &ConnectionManager::new(),
        &orchestrator,
        None,
        false,
        None,
        0,
        HostPresence::Locked,
    );
    assert_eq!(dto.host, "locked");
    assert_eq!(dto.desired, "busy");
    assert_eq!(dto::initial_status().host, "active");
}

#[test]
fn incompatible_status_carries_reason() {
    let mut manager = ConnectionManager::new();
    manager.apply(ManagerEvent::PortOpened);
    manager.apply(ManagerEvent::HandshakeIncompatible { device_major: 2 });
    let dto = dto::connection_status(
        &manager,
        &Orchestrator::new(),
        None,
        false,
        None,
        1,
        HostPresence::Active,
    );
    assert_eq!(dto.connection, "incompatible");
    assert!(dto.incompatible_reason.unwrap().contains("v2"));
    assert!(dto.device.is_none());
}

#[test]
fn app_info_reports_supported_protocol_and_studio_flag() {
    let dev = dto::app_info(true);
    assert!(dev.device_studio_enabled);
    assert_eq!(dev.supported_majors, vec![1]);
    assert_eq!(dev.protocol_version.major, 1);
    assert!(!dto::app_info(false).device_studio_enabled);
}

#[test]
fn sendable_boundary_rejects_device_originated_and_unknown() {
    // The production state boundary accepts only the four sendable tokens (FR-014/015).
    assert_eq!(dto::sendable_from_token("idle"), Some(SendableState::Idle));
    assert_eq!(dto::sendable_from_token("busy"), Some(SendableState::Busy));
    assert_eq!(dto::sendable_from_token("booting"), None);
    assert_eq!(dto::sendable_from_token("offline"), None);
    assert_eq!(dto::sendable_from_token("nonsense"), None);
    // Companion parsing (preview only) accepts all six.
    assert_eq!(
        dto::companion_from_token("booting"),
        Some(CompanionState::Booting)
    );
    assert_eq!(
        dto::companion_from_token("offline"),
        Some(CompanionState::Offline)
    );
    assert_eq!(dto::companion_from_token("nope"), None);
    assert_eq!(
        dto::mascot_action_from_token("tickle"),
        Some(MascotAction::Tickle)
    );
    assert_eq!(dto::mascot_action_from_token("unknown"), None);
    assert_eq!(
        dto::mascot_personality_from_token("calm"),
        Some(MascotPersonality::Calm)
    );
    assert_eq!(dto::mascot_personality_from_token("unknown"), None);
}

#[test]
fn activity_event_projection_uses_allowlisted_metadata() {
    let log = ActivityLog::new(1);
    let event = log.record(
        ActivityEventKind::ConnectionStateChanged,
        Some(ActivityMetadata::Connection {
            state: ConnectionState::Error,
            retry_count: 2,
            elapsed_ms: 750,
        }),
    );
    let activity = dto::activity_event(&event);
    assert_eq!(
        activity.event_type,
        dto::ActivityEventTypeDto::ConnectionStateChanged
    );
    assert_eq!(activity.summary, "Connection changed to error.");
    assert_eq!(
        activity.metadata.expect("connection metadata").retry_count,
        2
    );
}

#[test]
fn activity_log_keeps_recent_within_capacity() {
    let log = ActivityLog::new(3);
    for state in [
        ConnectionState::Connecting,
        ConnectionState::Connected,
        ConnectionState::Error,
        ConnectionState::Disconnected,
        ConnectionState::Connecting,
    ] {
        log.record(
            ActivityEventKind::ConnectionStateChanged,
            Some(ActivityMetadata::Connection {
                state,
                retry_count: 0,
                elapsed_ms: 0,
            }),
        );
    }
    let recent = log.recent(10);
    assert_eq!(recent.len(), 3, "capacity caps the ring");
    assert_eq!(
        recent.first().unwrap().summary(),
        "Connection changed to error.",
        "oldest surviving entry"
    );
    assert_eq!(
        recent.last().unwrap().summary(),
        "Connection changed to connecting.",
        "newest entry last"
    );
    assert_eq!(log.recent(1).len(), 1, "limit honoured");
}

/// Builds an AppState around a stand-in device thread that honours the cancellation contract.
fn app_state_with_dummy_thread() -> (AppState, Arc<AtomicBool>) {
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel::<DeviceCommand>();
    let thread_cancel = Arc::clone(&cancel);
    let thread = std::thread::spawn(move || {
        while !thread_cancel.load(Ordering::SeqCst) {
            let _ = rx.recv_timeout(Duration::from_millis(5));
        }
    });
    let status = Arc::new(Mutex::new(dto::initial_status()));
    let activity_log = Arc::new(ActivityLog::new(8));
    let state = AppState::new(true, status, activity_log, tx, Arc::clone(&cancel), thread);
    (state, cancel)
}

#[test]
fn app_state_relays_commands_then_shuts_down_the_thread() {
    let (state, cancel) = app_state_with_dummy_thread();
    assert_eq!(state.status_snapshot().connection, "disconnected");
    state
        .send_command(DeviceCommand::SetDesired(SendableState::Busy))
        .expect("command relayed while running");

    state.shutdown();
    assert!(
        cancel.load(Ordering::SeqCst),
        "shutdown signalled cancellation"
    );
    // The thread has joined and dropped the receiver, so further sends fail cleanly.
    assert!(
        state.send_command(DeviceCommand::Refresh).is_err(),
        "commands fail once the device thread has stopped"
    );
}
