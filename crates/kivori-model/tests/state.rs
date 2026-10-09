//! T018 — state-axis legality: only four states are sendable; `booting`/`offline` are
//! device-originated; connection transitions accept valid paths and reject invalid ones; cold-start
//! desired state is `idle` (FR-012, FR-014, FR-015, FR-005).

use kivori_model::connection::{ConnectionEvent, ConnectionState};
use kivori_model::state::{CompanionState, LifecycleState, NotSendable, SendableState};

#[test]
fn exactly_four_sendable_and_six_companion_states() {
    assert_eq!(SendableState::ALL.len(), 4);
    assert_eq!(CompanionState::ALL.len(), 6);
}

#[test]
fn booting_and_offline_are_not_sendable() {
    assert_eq!(
        SendableState::try_from(CompanionState::Booting),
        Err(NotSendable(CompanionState::Booting))
    );
    assert_eq!(
        SendableState::try_from(CompanionState::Offline),
        Err(NotSendable(CompanionState::Offline))
    );
    assert!(CompanionState::Booting.is_device_originated());
    assert!(CompanionState::Offline.is_device_originated());
    assert!(CompanionState::Booting.as_sendable().is_none());
    assert!(CompanionState::Offline.as_sendable().is_none());
}

#[test]
fn every_sendable_round_trips_through_companion() {
    for s in SendableState::ALL {
        let c = CompanionState::from(s);
        assert!(!c.is_device_originated());
        assert_eq!(SendableState::try_from(c), Ok(s));
    }
}

#[test]
fn cold_start_desired_state_is_idle() {
    assert_eq!(SendableState::default(), SendableState::Idle);
}

#[test]
fn lifecycle_states_are_device_originated() {
    assert!(LifecycleState::Booting
        .to_companion()
        .is_device_originated());
    assert!(LifecycleState::Offline
        .to_companion()
        .is_device_originated());
}

#[test]
fn connection_accepts_valid_transitions() {
    use ConnectionEvent as E;
    use ConnectionState as S;
    assert_eq!(
        S::Disconnected.transition(E::PortOpened),
        Some(S::Connecting)
    );
    assert_eq!(S::Connecting.transition(E::HandshakeOk), Some(S::Connected));
    assert_eq!(
        S::Connecting.transition(E::HandshakeIncompatible),
        Some(S::Incompatible)
    );
    assert_eq!(
        S::Connecting.transition(E::HandshakeTimeout),
        Some(S::Error)
    );
    assert_eq!(S::Connecting.transition(E::IoError), Some(S::Error));
    assert_eq!(S::Connected.transition(E::IoError), Some(S::Error));
    assert_eq!(S::Connected.transition(E::HeartbeatTimeout), Some(S::Error));
    assert_eq!(
        S::Connected.transition(E::PortRemoved),
        Some(S::Disconnected)
    );
    assert_eq!(
        S::Error.transition(E::BackoffElapsed),
        Some(S::Disconnected)
    );
    assert_eq!(
        S::Incompatible.transition(E::PortRemoved),
        Some(S::Disconnected)
    );
}

#[test]
fn connection_rejects_invalid_transitions() {
    use ConnectionEvent as E;
    use ConnectionState as S;
    assert_eq!(S::Disconnected.transition(E::HandshakeOk), None);
    assert_eq!(S::Disconnected.transition(E::BackoffElapsed), None);
    assert_eq!(S::Connected.transition(E::PortOpened), None);
    assert_eq!(S::Connected.transition(E::HandshakeOk), None);
    assert_eq!(S::Incompatible.transition(E::HandshakeOk), None);
    assert_eq!(S::Incompatible.transition(E::IoError), None);
    assert_eq!(S::Error.transition(E::HandshakeOk), None);
}

#[test]
fn incompatible_moves_on_to_another_candidate_port() {
    use ConnectionEvent as E;
    use ConnectionState as S;
    assert_eq!(
        S::Incompatible.transition(E::PortOpened),
        Some(S::Connecting)
    );
}

#[test]
fn only_connected_can_drive_device() {
    assert!(ConnectionState::Connected.can_drive_device());
    assert!(!ConnectionState::Connecting.can_drive_device());
    assert!(!ConnectionState::Disconnected.can_drive_device());
    assert!(!ConnectionState::Incompatible.can_drive_device());
    assert!(!ConnectionState::Error.can_drive_device());
    assert_eq!(ConnectionState::default(), ConnectionState::Disconnected);
}
