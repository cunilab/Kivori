//! Platform capability boundary.
//!
//! Three orthogonal concepts, deliberately never collapsed (design spec section 6):
//!   * BackendAvailability — a fact about THIS Kivori build and its runtime;
//!   * ConfirmationClass   — how well an executed action's outcome can be observed;
//!   * ActionAvailability  — the derived value the UI consumes.
//!
//! A `PlatformCapability` type (a fact about the MACHINE) is intentionally absent:
//! every action in Slice 002 is one the OS can perform, so such a type would have
//! no producer. It arrives with the first genuinely OS-restricted action.

pub mod unimplemented;
#[cfg(windows)]
pub mod windows;

use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendAvailability {
    Available,
    /// Kivori has no backend compiled for this OS. NOT the same as unsupported.
    NotImplemented {
        target: &'static str,
    },
    /// Implemented, but a runtime dependency is missing right now.
    RuntimeUnavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationClass {
    StateConfirmed,
    ExecutionConfirmed,
    TriggeredUnverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionAvailability {
    Available { confirmation: ConfirmationClass },
    NotImplementedYet { target: &'static str },
    RuntimeUnavailable { reason: String },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// No default render endpoint exists.
    NoEndpoint,
    /// The write was accepted but the resulting state could not be read back.
    ReadBackUnavailable,
    Os(String),
}

/// Master output volume, 0..=100.
pub trait VolumeBackend: Send + Sync {
    fn availability(&self) -> ActionAvailability;
    fn read(&self) -> Result<u8, BackendError>;
    /// Apply a value and return the value actually observed afterwards.
    ///
    /// Returning the read-back rather than `()` is what makes StateConfirmed
    /// structurally honest: the confirmed value comes from the OS, never the request.
    fn set(&self, percent: u8) -> Result<u8, BackendError>;
}

#[derive(Debug)]
pub struct FakeVolumeBackend {
    state: Mutex<u8>,
    availability: ActionAvailability,
    quantise_step: Option<u8>,
    readable_after_write: bool,
}

impl FakeVolumeBackend {
    pub fn new(initial: u8) -> Self {
        Self {
            state: Mutex::new(initial),
            availability: ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            quantise_step: None,
            readable_after_write: true,
        }
    }

    /// Models a device whose driver snaps to coarse steps.
    pub fn quantised(initial: u8, step: u8) -> Self {
        Self {
            quantise_step: Some(step),
            ..Self::new(initial)
        }
    }

    pub fn with_availability(availability: ActionAvailability) -> Self {
        Self {
            availability,
            ..Self::new(0)
        }
    }

    pub fn unreadable_after_write(initial: u8) -> Self {
        Self {
            readable_after_write: false,
            ..Self::new(initial)
        }
    }

    /// Simulates a change made outside Kivori (the Windows flyout, a media key).
    pub fn external_change(&self, percent: u8) {
        *self.state.lock().expect("fake backend mutex") = percent;
    }
}

impl VolumeBackend for FakeVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        self.availability.clone()
    }

    fn read(&self) -> Result<u8, BackendError> {
        if !matches!(self.availability, ActionAvailability::Available { .. }) {
            return Err(BackendError::NoEndpoint);
        }
        Ok(*self.state.lock().expect("fake backend mutex"))
    }

    fn set(&self, percent: u8) -> Result<u8, BackendError> {
        if !matches!(self.availability, ActionAvailability::Available { .. }) {
            return Err(BackendError::NoEndpoint);
        }
        let applied = match self.quantise_step {
            Some(step) if step > 0 => (percent / step) * step,
            _ => percent,
        };
        *self.state.lock().expect("fake backend mutex") = applied;
        if !self.readable_after_write {
            return Err(BackendError::ReadBackUnavailable);
        }
        Ok(applied)
    }
}
