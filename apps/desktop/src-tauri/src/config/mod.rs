//! Persistent user configuration: one JSON file in the per-user, per-machine app data directory.
//!
//! [`ConfigStore`] owns the file (atomic writes, recovery of a bad file, versioned migrations).
//! [`ResolvedConfig`] is what the runtime acts on. File paths never leave this module: callers see
//! only a [`ConfigNotice`] and a fixed error text (ADR-0005).

pub mod migrate;
pub mod resolve;
pub mod schema;
pub mod store;

use crate::desk::profile::{builtins, Profile};

pub use schema::{
    ActionSpec, BuddySettings, ButtonOverride, ConfigFile, DisplaySettings, Intensity, ProfileId,
    ProfileOverride, RotateSpec, SecondaryView, SlotSpec, View, CONFIG_VERSION,
};
pub use store::{ConfigError, ConfigNotice, ConfigStore};

/// The config as the device thread applies it: the built-in profiles with the user's overrides
/// layered on, and the display and buddy settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub profiles: Vec<Profile>,
    pub display: DisplaySettings,
    pub buddy: BuddySettings,
}

impl Default for ResolvedConfig {
    /// The built-ins for this OS and default settings.
    fn default() -> Self {
        Self {
            profiles: builtins(),
            display: DisplaySettings::default(),
            buddy: BuddySettings::default(),
        }
    }
}
