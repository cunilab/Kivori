//! The honest fallback for targets Kivori has not implemented a backend for.
//!
//! This reports `NotImplementedYet`, never "unsupported": macOS Core Audio and
//! Linux PipeWire can both change master volume. Claiming otherwise would tell a
//! user their machine cannot do something it can.

use super::{
    ActionAvailability, ActionError, AppVolumeBackend, AppVolumeError, BackendError, InputSynth,
    MediaObserver, Shortcut, VolumeBackend,
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

/// Per-app volume for a target with no implementation.
#[derive(Debug, Clone, Copy)]
pub struct UnimplementedAppVolumeBackend {
    target: &'static str,
}

impl UnimplementedAppVolumeBackend {
    pub const fn new(target: &'static str) -> Self {
        Self { target }
    }
}

impl AppVolumeBackend for UnimplementedAppVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        ActionAvailability::NotImplementedYet {
            target: self.target,
        }
    }

    fn read(&self, _app: &str) -> Result<u8, AppVolumeError> {
        Err(AppVolumeError::NoSession)
    }

    fn set(&self, _app: &str, _percent: u8) -> Result<u8, AppVolumeError> {
        Err(AppVolumeError::NoSession)
    }

    fn read_mute(&self, _app: &str) -> Result<bool, AppVolumeError> {
        Err(AppVolumeError::NoSession)
    }

    fn set_mute(&self, _app: &str, _muted: bool) -> Result<bool, AppVolumeError> {
        Err(AppVolumeError::NoSession)
    }
}

/// Key synthesis for a target with no implementation: every request says so.
#[derive(Debug, Clone, Copy)]
pub struct UnimplementedInputSynth {
    pub target: &'static str,
}

impl InputSynth for UnimplementedInputSynth {
    fn send_media_key(&self, _key: super::MediaKey) -> Result<(), ActionError> {
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

/// Focus observation for a target without a backend: always unknown, so only General applies.
#[derive(Debug, Clone, Copy)]
pub struct NoForeground;

impl super::ForegroundObserver for NoForeground {
    fn foreground(&self) -> super::Foreground {
        super::Foreground::Unknown
    }
}

/// Targets without a sleep/lock source never emit a host event: the device simply never hears
/// "goodnight", and its host-silence timeout still shows Offline.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
pub fn start_host_events(
    _tx: std::sync::mpsc::Sender<super::host_events::HostSignal>,
) -> Result<super::host_events::HostEventsGuard, super::host_events::HostEventsError> {
    Ok(super::host_events::HostEventsGuard::inert())
}
