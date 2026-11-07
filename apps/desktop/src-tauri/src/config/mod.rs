//! Persistent user configuration: one JSON file in the per-user, per-machine app data directory.
//!
//! [`ConfigStore`] owns the file (atomic writes, recovery of a bad file, versioned migrations).
//! [`ResolvedConfig`] is what the runtime acts on. File paths never leave this module: callers see
//! only a [`ConfigNotice`] and a fixed error text (ADR-0005).

pub mod migrate;
pub mod profiles;
pub mod resolve;
pub mod schema;
pub mod store;

use crate::desk::actions::Macros;
use crate::desk::profile::{builtins, Profile};

pub use schema::{
    hex_color, parse_hex_color, ActionSpec, BuddySettings, ButtonOverride, ConfigFile,
    CustomProfile, DisplaySettings, Intensity, LedMode, LedSettings, LedSpeed, LedWire, MacroSpec,
    ProfileId, ProfileOverride, ProfileRef, RotateSpec, SecondaryView, SlotSpec, StepSpec,
    UpdateSettings, UpdateSnooze, View, CONFIG_VERSION, LED_DEFAULT_BRIGHTNESS, LED_DEFAULT_COLOR,
    MAX_CUSTOM_PROFILES, MAX_PROFILE_APPS, PROFILE_NAME_MAX,
};
pub use store::{adopt_earlier_config, ConfigError, ConfigNotice, ConfigStore};

/// The config as the device thread applies it: the built-in profiles with the user's overrides
/// layered on, and the display, buddy and LED settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub profiles: Vec<Profile>,
    /// The user's macros by id; bindings hold the same `Arc`s.
    pub macros: Macros,
    pub display: DisplaySettings,
    pub buddy: BuddySettings,
    pub led: LedSettings,
}

impl Default for ResolvedConfig {
    /// The built-ins for this OS and default settings.
    fn default() -> Self {
        Self {
            profiles: builtins(),
            macros: Macros::new(),
            display: DisplaySettings::default(),
            buddy: BuddySettings::default(),
            led: LedSettings::default(),
        }
    }
}
