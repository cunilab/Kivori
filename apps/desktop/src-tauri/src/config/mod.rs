//! Persistent user configuration: one JSON file in the per-user, per-machine app data directory.
//!
//! [`ConfigStore`] owns the file (atomic writes, recovery of a bad file, versioned migrations).
//! [`ResolvedConfig`] is what the runtime acts on. File paths never leave this module: callers see
//! only a [`ConfigNotice`] and a fixed error text (ADR-0005).

pub mod migrate;
pub mod schema;
pub mod store;

use std::sync::Arc;

pub use schema::{
    BuddySettings, ConfigFile, DisplaySettings, Intensity, SecondaryView, View, CONFIG_VERSION,
};
pub use store::{ConfigError, ConfigNotice, ConfigStore};

/// The config as the device thread applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResolvedConfig {
    pub display: DisplaySettings,
    pub buddy: BuddySettings,
}

impl ResolvedConfig {
    #[must_use]
    pub fn of(file: &ConfigFile) -> Arc<Self> {
        Arc::new(Self {
            display: file.display,
            buddy: file.buddy,
        })
    }
}
