//! The persisted config file (version 1). Everything the user can change lives here and nowhere else.
//!
//! The file is strict (`deny_unknown_fields`): an unknown field is treated as corruption, backed up
//! and reported, rather than silently dropped on the next save. Fields added during M2 stay in
//! version 1 with `#[serde(default)]`; once the beta ships, any field change bumps [`CONFIG_VERSION`].

use std::collections::BTreeMap;

use kivori_model::desk::DisplayMode;
use kivori_model::MascotPersonality;
use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

/// The schema version this build reads and writes.
pub const CONFIG_VERSION: u32 = 1;

/// The whole config file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigFile {
    pub version: u32,
    /// Sparse per-profile overrides of the built-in profiles.
    #[serde(default)]
    pub profiles: BTreeMap<ProfileId, ProfileOverride>,
    /// User macros. Empty until the macro slice fills it in.
    #[serde(default)]
    pub macros: Vec<MacroSpec>,
    #[serde(default)]
    pub display: DisplaySettings,
    #[serde(default)]
    pub buddy: BuddySettings,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            profiles: BTreeMap::new(),
            macros: Vec::new(),
            display: DisplaySettings::default(),
            buddy: BuddySettings::default(),
        }
    }
}

impl ConfigFile {
    /// Checks the rules serde cannot express.
    ///
    /// # Errors
    /// A short, path-free reason when the file is not acceptable.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != CONFIG_VERSION {
            return Err("unsupported config version");
        }
        self.display.validate()
    }
}

/// A built-in profile a user override is keyed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileId {
    General,
    Browser,
    Code,
    Media,
    Zoom,
    Teams,
}

impl ProfileId {
    pub const ALL: [Self; 6] = [
        Self::General,
        Self::Browser,
        Self::Code,
        Self::Media,
        Self::Zoom,
        Self::Teams,
    ];

    /// The lowercase token used in the file and over IPC.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Browser => "browser",
            Self::Code => "code",
            Self::Media => "media",
            Self::Zoom => "zoom",
            Self::Teams => "teams",
        }
    }

    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.token() == token)
    }
}

/// One profile's overrides. Every field is sparse: `None` inherits the built-in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotate: Option<RotateSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub press: Option<SlotSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<SlotSpec>,
    #[serde(default, skip_serializing_if = "buttons_untouched")]
    pub buttons: [ButtonOverride; 3],
}

fn buttons_untouched(buttons: &[ButtonOverride; 3]) -> bool {
    buttons.iter().all(|b| *b == ButtonOverride::default())
}

impl ProfileOverride {
    /// Nothing overridden: the entry can be dropped from the file.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A contextual button's overrides. The middle button's Hold is reserved for the profile pin.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ButtonOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub press: Option<SlotSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<SlotSpec>,
}

/// A replacement for one slot: the whole slot, not a patch of the built-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlotSpec {
    /// `null` = explicitly unbound.
    #[serde(default)]
    pub action: Option<ActionSpec>,
    /// What the device calls it; `None` = the action's own label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The discrete actions a slot can be bound to (App Mute and Macro arrive in later slices).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ActionSpec {
    PlayPause,
    PreviousTrack,
    NextTrack,
    SystemMute,
    Shortcut { keys: String },
    Launch { target: String },
}

/// What the knob does (App Volume arrives in a later slice).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RotateSpec {
    SystemVolume,
    Shortcuts {
        cw: String,
        ccw: String,
        label: String,
    },
}

/// One user macro. No field exists yet; the macro slice adds them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MacroSpec {}

/// A device view, mirrored here so `kivori-model` stays untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    Buddy,
    Clock,
    Volume,
    Media,
    System,
}

impl View {
    #[must_use]
    pub const fn mode(self) -> DisplayMode {
        match self {
            Self::Buddy => DisplayMode::Buddy,
            Self::Clock => DisplayMode::Clock,
            Self::Volume => DisplayMode::Volume,
            Self::Media => DisplayMode::Media,
            Self::System => DisplayMode::System,
        }
    }

    #[must_use]
    pub const fn from_mode(mode: DisplayMode) -> Self {
        match mode {
            DisplayMode::Buddy => Self::Buddy,
            DisplayMode::Clock => Self::Clock,
            DisplayMode::Volume => Self::Volume,
            DisplayMode::Media => Self::Media,
            DisplayMode::System => Self::System,
        }
    }
}

/// What a double press shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryView {
    /// Toggle between the default view and this one.
    View(View),
    /// Step through every view in turn (the M1 behaviour).
    Cycle,
}

/// Serialized as the view's lowercase token, or `"cycle"`.
impl Serialize for SecondaryView {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Cycle => serializer.serialize_str("cycle"),
            Self::View(view) => view.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SecondaryView {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let token = String::deserialize(deserializer)?;
        if token == "cycle" {
            return Ok(Self::Cycle);
        }
        serde_json::from_value::<View>(serde_json::Value::String(token))
            .map(Self::View)
            .map_err(|_| de::Error::custom("unknown secondary view"))
    }
}

/// Which view the device starts on and what a double press does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplaySettings {
    pub default_view: View,
    pub secondary_view: SecondaryView,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            default_view: View::Buddy,
            secondary_view: SecondaryView::View(View::System),
        }
    }
}

impl DisplaySettings {
    /// A double press that would toggle a view with itself would do nothing, so it is refused.
    ///
    /// # Errors
    /// A short reason when the secondary view equals the default view.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self.secondary_view {
            SecondaryView::View(view) if view == self.default_view => {
                Err("the double-press view must differ from the default view")
            }
            _ => Ok(()),
        }
    }
}

/// How lively the buddy is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Intensity {
    Low,
    Normal,
    High,
}

impl Intensity {
    /// Low, Normal and High are the calm, cozy and playful personalities.
    #[must_use]
    pub const fn personality(self) -> MascotPersonality {
        match self {
            Self::Low => MascotPersonality::Calm,
            Self::Normal => MascotPersonality::Cozy,
            Self::High => MascotPersonality::Playful,
        }
    }
}

/// The buddy's ambient behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuddySettings {
    /// Whether the buddy plays reactions on its own while idle.
    pub reactions: bool,
    pub intensity: Intensity,
}

impl Default for BuddySettings {
    fn default() -> Self {
        Self {
            reactions: true,
            intensity: Intensity::Normal,
        }
    }
}
