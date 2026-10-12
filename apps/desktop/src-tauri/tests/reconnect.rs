//! Reconnect backoff + within-process desired-state resync tests (T087; FR-008/009/010; US3).

use kivori_desktop::device::connection::ConnectedDevice;
use kivori_desktop::device::fsm::{ConnectionManager, ManagerEvent};
use kivori_desktop::device::reconnect::{base_delay_ms, with_jitter, Backoff, BASE_MS, MAX_MS};
use kivori_desktop::orchestrator::Orchestrator;
use kivori_model::{ConnectionState, ProtocolVersion, SendableState};
use kivori_protocol::{FirmwareVersion, PROTOCOL_MAJOR, PROTOCOL_MINOR};

fn device() -> ConnectedDevice {
    ConnectedDevice {
        firmware_version: FirmwareVersion {
            major: 0,
            minor: 1,
            patch: 0,
        },
        protocol_version: ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
        device_id_hash_short: "abcd1234".to_string(),
    }
}

#[test]
fn backoff_is_exponential_and_capped() {
    assert_eq!(base_delay_ms(0), BASE_MS);
    assert_eq!(base_delay_ms(0), 250);
    assert_eq!(base_delay_ms(1), 500);
    assert_eq!(base_delay_ms(2), 1000);
    assert_eq!(base_delay_ms(3), 2000);
    assert_eq!(base_delay_ms(4), 4000);
    assert_eq!(base_delay_ms(5), MAX_MS, "8000 capped to 5000");
    assert_eq!(base_delay_ms(99), MAX_MS, "large counts never overflow");
}

#[test]
fn backoff_sequencer_advances_and_resets() {
    let mut b = Backoff::new();
    assert_eq!(b.next_delay().as_millis(), 250);
    assert_eq!(b.next_delay().as_millis(), 500);
    assert_eq!(b.next_delay().as_millis(), 1000);
    assert_eq!(b.attempt(), 3);
    b.reset();
    assert_eq!(b.attempt(), 0);
    assert_eq!(b.next_delay().as_millis(), 250);
}

#[test]
fn jitter_is_symmetric_and_bounded() {
    let base = 1000;
    assert_eq!(with_jitter(base, 500), 1000, "center = no change");
    assert_eq!(with_jitter(base, 0), 750, "-25%");
    assert_eq!(with_jitter(base, 1000), 1250, "+25%");
    for permille in [0u16, 250, 500, 750, 1000, 2000] {
        let d = with_jitter(base, permille);
        assert!(
            (750..=1250).contains(&d),
            "jitter stays within ±25% for {permille}"
        );
    }
}

#[test]
fn cold_start_orchestrator_defaults_to_idle() {
    // A fresh process always starts Idle — desired state is not persisted (clarified).
    assert_eq!(Orchestrator::new().desired(), SendableState::Idle);
}

#[test]
fn desired_state_survives_reconnect_within_process() {
    let mut orch = Orchestrator::new();
    orch.set_desired(SendableState::Busy);

    // Simulate an unplug/replug cycle on the connection manager; the orchestrator is untouched.
    let mut m = ConnectionManager::new();
    m.apply(ManagerEvent::PortOpened);
    m.apply(ManagerEvent::HandshakeOk(device()));
    m.apply(ManagerEvent::PortRemoved);
    m.apply(ManagerEvent::PortOpened);
    m.apply(ManagerEvent::HandshakeOk(device()));
    assert_eq!(m.state(), ConnectionState::Connected);

    // On reconnect, the state to resync is still Busy (within-process; no persistence).
    assert_eq!(orch.resync_state(), SendableState::Busy);
}

#[test]
fn rapid_unplug_replug_is_stable() {
    let mut m = ConnectionManager::new();
    for _ in 0..50 {
        m.apply(ManagerEvent::PortOpened);
        m.apply(ManagerEvent::HandshakeOk(device()));
        m.apply(ManagerEvent::PortRemoved);
    }
    assert_eq!(m.state(), ConnectionState::Disconnected);
    assert_eq!(m.retry_count(), 0);
    // Still connects cleanly afterwards.
    m.apply(ManagerEvent::PortOpened);
    m.apply(ManagerEvent::HandshakeOk(device()));
    assert_eq!(m.state(), ConnectionState::Connected);
}
