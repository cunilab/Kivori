//! The honest fallback for targets Kivori has not implemented a backend for.
//!
//! This reports `NotImplementedYet`, never "unsupported": macOS Core Audio and
//! Linux PipeWire can both change master volume. Claiming otherwise would tell a
//! user their machine cannot do something it can.

use super::{
    ActionAvailability, ActionError, BackendError, InputSynth, MediaObserver, Shortcut,
    VolumeBackend,
};
use kivori_model::desk::MediaStatus;

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

    fn read_mute(&self) -> Result<bool, BackendError> {
        Err(BackendError::NoEndpoint)
    }

    fn set_mute(&self, _muted: bool) -> Result<bool, BackendError> {
        Err(BackendError::NoEndpoint)
    }
}

/// Key synthesis for a target with no implementation: every request says so.
#[derive(Debug, Clone, Copy)]
pub struct UnimplementedInputSynth {
    pub target: &'static str,
}

impl InputSynth for UnimplementedInputSynth {
    fn send_media_play_pause(&self) -> Result<(), ActionError> {
        Err(ActionError::NotImplemented {
            target: self.target,
        })
    }

    fn send_shortcut(&self, _shortcut: &Shortcut) -> Result<(), ActionError> {
        Err(ActionError::NotImplemented {
            target: self.target,
        })
    }
}

/// Media observation for a target that cannot observe playback.
#[derive(Debug, Clone, Copy)]
pub struct NoMediaObserver;

impl MediaObserver for NoMediaObserver {
    fn status(&self) -> Option<MediaStatus> {
        None
    }
}
