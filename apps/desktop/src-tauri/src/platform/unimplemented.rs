//! The honest fallback for targets Kivori has not implemented a backend for.
//!
//! This reports `NotImplementedYet`, never "unsupported": macOS Core Audio and
//! Linux PipeWire can both change master volume. Claiming otherwise would tell a
//! user their machine cannot do something it can.

use super::{ActionAvailability, BackendError, VolumeBackend};

#[derive(Debug, Clone, Copy)]
pub struct UnimplementedVolumeBackend {
    target: &'static str,
}

impl UnimplementedVolumeBackend {
    pub const fn new(target: &'static str) -> Self {
        Self { target }
    }
}

impl VolumeBackend for UnimplementedVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        ActionAvailability::NotImplementedYet {
            target: self.target,
        }
    }

    fn read(&self) -> Result<u8, BackendError> {
        Err(BackendError::NoEndpoint)
    }

    fn set(&self, _percent: u8) -> Result<u8, BackendError> {
        Err(BackendError::NoEndpoint)
    }
}
