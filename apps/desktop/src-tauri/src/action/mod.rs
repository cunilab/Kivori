//! Action identity, the Slice 002 binding table, and typed execution outcomes.

pub mod gesture_value;
pub mod volume;

use crate::platform::{ActionAvailability, BackendError, VolumeBackend};
use kivori_protocol::message::ControlId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionId {
    MasterVolume,
}

/// Typed execution outcome.
///
/// Execution MUST NOT collapse into `bool success` (user-story-contract US2).
/// Slice 002 produces StateConfirmed, TriggeredUnverified, and Failed; the other
/// arms exist because later slices produce them and the distinction is contractual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Running,
    StateConfirmed { volume_percent: u8 },
    ExecutionConfirmed,
    TriggeredUnverified,
    Failed { reason: String },
}

/// The entire Slice 002 binding table: one Global bidirectional rotary binding.
///
/// A bidirectional `Rotate` binding owns BOTH directions as one logical control
/// (user-story-contract invariant 54).
pub const fn resolve_binding(control: ControlId) -> Option<ActionId> {
    match control {
        ControlId::Rotary => Some(ActionId::MasterVolume),
    }
}

/// Execute a volume target against a backend and classify the outcome honestly.
pub fn execute_volume(backend: &dyn VolumeBackend, target_percent: u8) -> Outcome {
    match backend.availability() {
        ActionAvailability::Available { .. } => {}
        ActionAvailability::NotImplementedYet { target } => {
            return Outcome::Failed {
                reason: format!("no volume backend implemented for {target}"),
            }
        }
        ActionAvailability::RuntimeUnavailable { reason } => return Outcome::Failed { reason },
        ActionAvailability::Unknown => {
            return Outcome::Failed {
                reason: "backend availability unknown".to_string(),
            }
        }
    }

    match backend.set(target_percent) {
        Ok(observed) => Outcome::StateConfirmed {
            volume_percent: observed,
        },
        // The write landed but its result cannot be observed: never claim success.
        Err(BackendError::ReadBackUnavailable) => Outcome::TriggeredUnverified,
        Err(BackendError::NoEndpoint) => Outcome::Failed {
            reason: "no default render endpoint".to_string(),
        },
        Err(BackendError::Os(reason)) => Outcome::Failed { reason },
    }
}
