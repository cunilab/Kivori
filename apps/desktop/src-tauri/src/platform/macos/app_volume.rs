//! Per-app volume on macOS: not offered. Core Audio has no per-process volume control that
//! another app may use, so this is `Unsupported` (a machine fact), never a fallback to the
//! system volume (invariant 19).

use crate::platform::{
    ActionAvailability, AppVolumeBackend, AppVolumeError, APP_VOLUME_UNSUPPORTED_ON_MAC,
};

#[derive(Debug, Clone, Copy)]
pub struct MacAppVolumeBackend;

impl AppVolumeBackend for MacAppVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        ActionAvailability::Unsupported {
            reason: APP_VOLUME_UNSUPPORTED_ON_MAC.to_string(),
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
