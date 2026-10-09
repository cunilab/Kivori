//! Connection-manager FSM + supporting-logic tests (T086; FR-002/003/005/007, SC-003).

use kivori_desktop::device::connection::{
    build_hello, hash_device_id_short, summarize, verify_handshake, ConnectedDevice,
};
use kivori_desktop::device::discovery::{
    filter_candidates, is_candidate, PortCandidate, DEFAULT_ALLOWLIST, KIVORI_PID, KIVORI_VID,
};
use kivori_desktop::device::fsm::{ConnectionManager, ManagerEvent};
use kivori_desktop::device::heartbeat::HeartbeatMonitor;
use kivori_model::{Capabilities, ConnectionState, ProtocolVersion};
use kivori_protocol::{FirmwareVersion, HandshakeOutcome, HelloAck};

fn device() -> ConnectedDevice {
    ConnectedDevice {
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 2,
            patch: 0,
        },
        protocol_version: ProtocolVersion::new(1, 0),
        device_id_hash_short: "deadbeef".to_string(),
    }
}

#[test]
fn happy_path_reaches_connected_and_can_drive() {
    let mut m = ConnectionManager::new();
    assert_eq!(m.state(), ConnectionState::Disconnected);
    assert!(m.apply(ManagerEvent::PortOpened));
    assert_eq!(m.state(), ConnectionState::Connecting);
    assert!(m.apply(ManagerEvent::HandshakeOk(device())));
    assert_eq!(m.state(), ConnectionState::Connected);
    assert_eq!(m.retry_count(), 0);
    assert!(m.device().is_some());
    assert!(m.state().can_drive_device());
}

#[test]
fn incompatible_is_terminal_with_a_reason() {
    let mut m = ConnectionManager::new();
    m.apply(ManagerEvent::PortOpened);
    assert!(m.apply(ManagerEvent::HandshakeIncompatible { device_major: 2 }));
    assert_eq!(m.state(), ConnectionState::Incompatible);
    assert!(m.incompatible_reason().unwrap().contains("v2"));
    assert!(m.device().is_none());
    // Terminal except for port removal.
    assert!(!m.apply(ManagerEvent::BackoffElapsed));
    assert!(!m.apply(ManagerEvent::HandshakeOk(device())));
    assert!(m.apply(ManagerEvent::PortRemoved));
    assert_eq!(m.state(), ConnectionState::Disconnected);
    assert!(m.incompatible_reason().is_none());
}

#[test]
fn incompatible_is_left_by_opening_another_port_and_clears_the_reason() {
    let mut m = ConnectionManager::new();
    m.apply(ManagerEvent::PortOpened);
    m.apply(ManagerEvent::HandshakeIncompatible { device_major: 2 });
    assert!(m.incompatible_reason().is_some());
    assert!(m.apply(ManagerEvent::PortOpened));
    assert_eq!(m.state(), ConnectionState::Connecting);
    assert!(m.incompatible_reason().is_none());
    assert!(m.apply(ManagerEvent::HandshakeOk(device())));
    assert!(m.state().can_drive_device());
}

#[test]
fn errors_accumulate_retry_count_backoff_preserves_it() {
    let mut m = ConnectionManager::new();
    m.apply(ManagerEvent::PortOpened);
    assert!(m.apply(ManagerEvent::HandshakeTimeout));
    assert_eq!(m.state(), ConnectionState::Error);
    assert_eq!(m.retry_count(), 1);
    assert!(m.apply(ManagerEvent::BackoffElapsed));
    assert_eq!(m.state(), ConnectionState::Disconnected);
    assert_eq!(m.retry_count(), 1, "backoff retry preserves the count");
    m.apply(ManagerEvent::PortOpened);
    assert!(m.apply(ManagerEvent::IoError));
    assert_eq!(m.retry_count(), 2);
}

#[test]
fn heartbeat_timeout_from_connected_goes_to_error() {
    let mut m = ConnectionManager::new();
    m.apply(ManagerEvent::PortOpened);
    m.apply(ManagerEvent::HandshakeOk(device()));
    assert!(m.apply(ManagerEvent::HeartbeatTimeout));
    assert_eq!(m.state(), ConnectionState::Error);
    assert!(!m.state().can_drive_device());
}

#[test]
fn successful_reconnect_resets_retry_count() {
    let mut m = ConnectionManager::new();
    m.apply(ManagerEvent::PortOpened);
    m.apply(ManagerEvent::HandshakeTimeout);
    m.apply(ManagerEvent::BackoffElapsed);
    m.apply(ManagerEvent::PortOpened);
    m.apply(ManagerEvent::HandshakeOk(device()));
    assert_eq!(m.state(), ConnectionState::Connected);
    assert_eq!(m.retry_count(), 0);
}

#[test]
fn illegal_events_are_ignored() {
    let mut m = ConnectionManager::new();
    assert!(!m.apply(ManagerEvent::HandshakeOk(device())));
    assert_eq!(m.state(), ConnectionState::Disconnected);
    assert!(!m.apply(ManagerEvent::PortRemoved));
    assert_eq!(m.state(), ConnectionState::Disconnected);
}

#[test]
fn port_removed_from_connected_clears_device() {
    let mut m = ConnectionManager::new();
    m.apply(ManagerEvent::PortOpened);
    m.apply(ManagerEvent::HandshakeOk(device()));
    assert!(m.apply(ManagerEvent::PortRemoved));
    assert_eq!(m.state(), ConnectionState::Disconnected);
    assert!(m.device().is_none());
    assert_eq!(m.retry_count(), 0);
}

#[test]
fn handshake_verify_accepts_good_nonce_and_supported_major() {
    let sent = build_hello(
        FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        Capabilities::NONE,
        0x1234,
    );
    let ack = HelloAck {
        device_caps: Capabilities::NONE,
        device_id: [7; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 3,
            patch: 0,
        },
        nonce_echo: 0x1234,
    };
    let outcome = verify_handshake(
        &sent,
        &ack,
        ProtocolVersion::new(1, 2),
        ProtocolVersion::new(1, 0),
        &[1],
    );
    assert!(matches!(outcome, HandshakeOutcome::Compatible(_)));

    let summary = summarize(&ack, ProtocolVersion::new(1, 2));
    assert_eq!(summary.firmware_version.major, 1);
    assert_eq!(summary.protocol_version, ProtocolVersion::new(1, 2));
    assert_eq!(summary.device_id_hash_short.len(), 8);
}

#[test]
fn handshake_verify_rejects_bad_nonce_and_incompatible_major() {
    let sent = build_hello(
        FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        Capabilities::NONE,
        0x1234,
    );
    let ack_bad_nonce = HelloAck {
        device_caps: Capabilities::NONE,
        device_id: [1; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        nonce_echo: 0x9999,
    };
    assert!(matches!(
        verify_handshake(
            &sent,
            &ack_bad_nonce,
            ProtocolVersion::new(1, 0),
            ProtocolVersion::new(1, 0),
            &[1]
        ),
        HandshakeOutcome::BadNonce
    ));

    let ack_incompatible = HelloAck {
        nonce_echo: 0x1234,
        ..ack_bad_nonce
    };
    assert!(matches!(
        verify_handshake(
            &sent,
            &ack_incompatible,
            ProtocolVersion::new(2, 0),
            ProtocolVersion::new(1, 0),
            &[1]
        ),
        HandshakeOutcome::Incompatible { device_major: 2 }
    ));
}

#[test]
fn device_id_hash_is_short_stable_and_distinct() {
    let a = hash_device_id_short(&[0xAB; 16]);
    assert_eq!(a, hash_device_id_short(&[0xAB; 16]), "stable");
    assert_eq!(a.len(), 8, "short hex token");
    assert_ne!(
        a,
        hash_device_id_short(&[0x01; 16]),
        "distinct inputs differ"
    );
}

#[test]
fn heartbeat_times_out_after_threshold_and_resets_on_pong() {
    let mut hb = HeartbeatMonitor::new(3);
    assert!(!hb.timed_out());
    hb.on_ping_sent(0);
    hb.on_ping_sent(0);
    assert!(!hb.timed_out());
    hb.on_ping_sent(0);
    assert!(hb.timed_out());
    hb.on_pong(0, 0);
    assert!(!hb.timed_out());
    assert_eq!(hb.misses(), 0);
}

#[test]
fn discovery_filters_by_vid_pid_allowlist() {
    let ports = vec![
        PortCandidate::new("/dev/ttyACM0", Some(KIVORI_VID), Some(KIVORI_PID)),
        PortCandidate::new("COM3", Some(0x1234), Some(0x5678)),
        PortCandidate::new("/dev/ttyS0", None, None),
    ];
    let hits = filter_candidates(&ports, DEFAULT_ALLOWLIST);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].port_name, "/dev/ttyACM0");
    assert!(is_candidate(&ports[0], DEFAULT_ALLOWLIST));
    assert!(!is_candidate(&ports[1], DEFAULT_ALLOWLIST));
    assert!(!is_candidate(&ports[2], DEFAULT_ALLOWLIST));
}
