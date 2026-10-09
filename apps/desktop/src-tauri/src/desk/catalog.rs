//! The action catalog: every action Kivori can bind, with its slot, scope, how its outcome is
//! verified, what it needs from the user, and whether it runs in a protected context.
//!
//! The catalog is data for the config UI (`list_action_catalog`); `execute` stays the one place
//! that classifies an outcome, and a test holds each entry's `verification` to it. [`ActionToken`]
//! is the desktop-only identity of an action: richer than the wire `ActionKind` (firmware's
//! postcard decoding rejects an unknown variant, so App Volume and macros reuse an existing
//! wire kind and are told apart only here).

use crate::platform::{ActionAvailability, ConfirmationClass, APP_VOLUME_UNSUPPORTED_ON_MAC};

/// Which action an outcome, a status or an activity event is about. Never carries parameters:
/// no shortcut keys, no application path, no app id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionToken {
    Volume,
    PlayPause,
    Mute,
    Shortcut,
    Launch,
    PreviousTrack,
    NextTrack,
    AppVolume,
    AppMute,
    Macro,
}

impl ActionToken {
    /// Every token, in the order of `DESK_ACTIONS` in `lib/ipc/types.ts`.
    pub const ALL: [Self; 10] = [
        Self::Volume,
        Self::PlayPause,
        Self::Mute,
        Self::Shortcut,
        Self::Launch,
        Self::PreviousTrack,
        Self::NextTrack,
        Self::AppVolume,
        Self::AppMute,
        Self::Macro,
    ];

    /// The closed webview token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Volume => "volume",
            Self::PlayPause => "playPause",
            Self::Mute => "mute",
            Self::Shortcut => "shortcut",
            Self::Launch => "launch",
            Self::PreviousTrack => "previousTrack",
            Self::NextTrack => "nextTrack",
            Self::AppVolume => "appVolume",
            Self::AppMute => "appMute",
            Self::Macro => "macro",
        }
    }

    /// What the device is told. The wire has no App Volume, App Mute or macro: they show as the
    /// kind of what they do (a macro's real kind is its first step's, decided when macros land).
    #[must_use]
    pub const fn wire_kind(self) -> kivori_model::desk::ActionKind {
        use kivori_model::desk::ActionKind;
        match self {
            Self::Volume | Self::AppVolume => ActionKind::Volume,
            Self::PlayPause => ActionKind::PlayPause,
            Self::Mute | Self::AppMute => ActionKind::Mute,
            Self::Shortcut | Self::Macro => ActionKind::Shortcut,
            Self::Launch => ActionKind::Launch,
            Self::PreviousTrack => ActionKind::PreviousTrack,
            Self::NextTrack => ActionKind::NextTrack,
        }
    }
}

/// The OS a catalog is described for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Other,
}

impl Os {
    /// The OS this build runs on.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Other
        }
    }
}

/// What the host services report right now. Carried as values so availability stays a pure
/// function that tests drive with the fakes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Services {
    /// The system volume backend's availability (volume and mute).
    pub volume: ActionAvailability,
    /// Whether key input can be synthesized (media keys and shortcuts).
    pub input: ActionAvailability,
    /// The per-app volume backend's availability (App Volume and App Mute).
    pub app_volume: ActionAvailability,
}

impl Services {
    /// What an OS is expected to provide, for callers without a live backend (the IPC command
    /// cannot reach the device thread's services).
    #[must_use]
    pub fn expected(os: Os) -> Self {
        let state = |confirmation| ActionAvailability::Available { confirmation };
        match os {
            Os::Windows => Self {
                volume: state(ConfirmationClass::StateConfirmed),
                input: state(ConfirmationClass::TriggeredUnverified),
                app_volume: state(ConfirmationClass::StateConfirmed),
            },
            Os::MacOs => Self {
                volume: state(ConfirmationClass::StateConfirmed),
                input: state(ConfirmationClass::TriggeredUnverified),
                app_volume: ActionAvailability::Unsupported {
                    reason: APP_VOLUME_UNSUPPORTED_ON_MAC.to_string(),
                },
            },
            Os::Other => {
                let target = std::env::consts::OS;
                Self {
                    volume: ActionAvailability::NotImplementedYet { target },
                    input: ActionAvailability::NotImplementedYet { target },
                    app_volume: ActionAvailability::NotImplementedYet { target },
                }
            }
        }
    }
}

/// Which control gesture an entry binds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The knob's rotation.
    Rotate,
    /// A press or hold.
    Discrete,
}

/// What an action acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    System,
    App,
    Media,
    Keyboard,
    Launch,
    Macro,
}

/// How well the outcome can be observed, as the config UI says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verification {
    /// The OS reported the resulting state (`StateConfirmed`).
    Confirmed,
    /// The OS reported a known start with nothing lasting to observe (`ExecutionConfirmed`).
    Started,
    /// Input was dispatched, its effect cannot be seen (`TriggeredUnverified`).
    Unverified,
    /// A macro is only as verified as its weakest step.
    LeastOfSteps,
}

impl Verification {
    /// The UI's word for a [`ConfirmationClass`].
    #[must_use]
    pub const fn of(class: ConfirmationClass) -> Self {
        match class {
            ConfirmationClass::StateConfirmed => Self::Confirmed,
            ConfirmationClass::ExecutionConfirmed => Self::Started,
            ConfirmationClass::TriggeredUnverified => Self::Unverified,
        }
    }

    /// The class `availability` reports. A macro's real class is only known per step, so it
    /// reports the floor.
    const fn class(self) -> ConfirmationClass {
        match self {
            Self::Confirmed => ConfirmationClass::StateConfirmed,
            Self::Started => ConfirmationClass::ExecutionConfirmed,
            Self::Unverified | Self::LeastOfSteps => ConfirmationClass::TriggeredUnverified,
        }
    }
}

/// What the user fills in to bind an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Params {
    None,
    App,
    Shortcut,
    ShortcutPair,
    Target,
    Macro,
}

/// What stops an entry from being offered at all, regardless of services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Support {
    Everywhere,
    /// Listed, but its backend has not landed yet.
    ComingSoon,
}

/// One bindable action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: &'static str,
    /// What `lastAction` and the activity log call it.
    pub token: ActionToken,
    pub slot: Slot,
    pub scope: Scope,
    pub verification: Verification,
    pub params: Params,
    /// Runs while the foreground is a protected surface (it injects no input into it).
    pub runs_when_protected: bool,
    support: Support,
}

#[allow(clippy::too_many_arguments)]
const fn entry(
    id: &'static str,
    token: ActionToken,
    slot: Slot,
    scope: Scope,
    verification: Verification,
    params: Params,
    runs_when_protected: bool,
    support: Support,
) -> CatalogEntry {
    CatalogEntry {
        id,
        token,
        slot,
        scope,
        verification,
        params,
        runs_when_protected,
        support,
    }
}

/// Every action, in the order the picker shows them.
///
/// Macros are listed but unsupported until their backend lands, and App Volume and App Mute are
/// unsupported on macOS: the config UI shows them disabled with a reason rather than hiding them.
pub const CATALOG: [CatalogEntry; 11] = {
    use ActionToken as T;
    use Support::{ComingSoon, Everywhere};
    use Verification::{Confirmed, LeastOfSteps, Started, Unverified};
    [
        entry(
            "systemVolume",
            T::Volume,
            Slot::Rotate,
            Scope::System,
            Confirmed,
            Params::None,
            true,
            Everywhere,
        ),
        entry(
            "appVolume",
            T::AppVolume,
            Slot::Rotate,
            Scope::App,
            Confirmed,
            Params::App,
            true,
            Everywhere,
        ),
        entry(
            "knobShortcuts",
            T::Shortcut,
            Slot::Rotate,
            Scope::Keyboard,
            Unverified,
            Params::ShortcutPair,
            false,
            Everywhere,
        ),
        entry(
            "playPause",
            T::PlayPause,
            Slot::Discrete,
            Scope::Media,
            Unverified,
            Params::None,
            true,
            Everywhere,
        ),
        entry(
            "previousTrack",
            T::PreviousTrack,
            Slot::Discrete,
            Scope::Media,
            Unverified,
            Params::None,
            true,
            Everywhere,
        ),
        entry(
            "nextTrack",
            T::NextTrack,
            Slot::Discrete,
            Scope::Media,
            Unverified,
            Params::None,
            true,
            Everywhere,
        ),
        entry(
            "systemMute",
            T::Mute,
            Slot::Discrete,
            Scope::System,
            Confirmed,
            Params::None,
            true,
            Everywhere,
        ),
        entry(
            "appMute",
            T::AppMute,
            Slot::Discrete,
            Scope::App,
            Confirmed,
            Params::App,
            true,
            Everywhere,
        ),
        entry(
            "shortcut",
            T::Shortcut,
            Slot::Discrete,
            Scope::Keyboard,
            Unverified,
            Params::Shortcut,
            false,
            Everywhere,
        ),
        entry(
            "launch",
            T::Launch,
            Slot::Discrete,
            Scope::Launch,
            Started,
            Params::Target,
            false,
            Everywhere,
        ),
        entry(
            "macro",
            T::Macro,
            Slot::Discrete,
            Scope::Macro,
            LeastOfSteps,
            Params::Macro,
            false,
            ComingSoon,
        ),
    ]
};

impl CatalogEntry {
    /// Whether this action can run here and now, for `os` with `services` as reported.
    #[must_use]
    pub fn availability(&self, os: Os, services: &Services) -> ActionAvailability {
        // The one OS-restricted scope: macOS has no per-app volume, however the services report.
        if self.scope == Scope::App && os == Os::MacOs {
            return ActionAvailability::Unsupported {
                reason: APP_VOLUME_UNSUPPORTED_ON_MAC.to_string(),
            };
        }
        if self.support == Support::ComingSoon {
            return ActionAvailability::Unsupported {
                reason: "Coming soon".to_string(),
            };
        }
        let backend = match self.scope {
            Scope::System => &services.volume,
            Scope::Media | Scope::Keyboard => &services.input,
            Scope::App => &services.app_volume,
            // A launch needs no synthesized input.
            Scope::Launch | Scope::Macro => {
                return ActionAvailability::Available {
                    confirmation: self.verification.class(),
                }
            }
        };
        match backend {
            ActionAvailability::Available { .. } => ActionAvailability::Available {
                confirmation: self.verification.class(),
            },
            other => other.clone(),
        }
    }
}

/// [`CATALOG`] with each entry's availability for `os`.
#[must_use]
pub fn catalog(os: Os, services: &Services) -> Vec<(CatalogEntry, ActionAvailability)> {
    CATALOG
        .iter()
        .map(|entry| (*entry, entry.availability(os, services)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk::actions::{execute, Action, Platform};
    use crate::platform::{
        FakeAppVolumeBackend, FakeInputSynth, FakeMediaObserver, FakeVolumeBackend, Shortcut,
        VolumeBackend,
    };
    use kivori_model::desk::FeedbackKind;
    use kivori_model::input::Direction;
    use std::sync::Arc;

    fn fakes() -> Platform {
        Platform {
            volume: Arc::new(FakeVolumeBackend::new(50)),
            app_volume: Arc::new(FakeAppVolumeBackend::new().with_session("spotify.exe", 50)),
            synth: Arc::new(FakeInputSynth::new(Ok(()))),
            media: Arc::new(FakeMediaObserver::default()),
            launch: |_| Ok(()),
        }
    }

    fn entry_named(id: &str) -> &'static CatalogEntry {
        CATALOG.iter().find(|e| e.id == id).expect("catalog entry")
    }

    /// The action each runnable discrete entry binds, with example parameters.
    fn example(id: &str) -> Option<Action> {
        Some(match id {
            "playPause" => Action::PlayPause,
            "previousTrack" => Action::PreviousTrack,
            "nextTrack" => Action::NextTrack,
            "systemMute" => Action::ToggleMute,
            "shortcut" | "knobShortcuts" => Action::Shortcut("Ctrl+M".parse::<Shortcut>().unwrap()),
            "launch" => Action::Launch("Calculator".into()),
            "appMute" => Action::AppMute {
                app: "spotify.exe".into(),
            },
            "appVolume" => Action::AppVolumeStep {
                app: "spotify.exe".into(),
                direction: Direction::Cw,
            },
            _ => return None,
        })
    }

    #[test]
    fn each_entrys_verification_is_what_execute_classifies() {
        let mut checked = 0;
        for entry in &CATALOG {
            let Some(action) = example(entry.id) else {
                continue;
            };
            let class = match execute(&action, &fakes()).kind {
                FeedbackKind::StateConfirmed => ConfirmationClass::StateConfirmed,
                FeedbackKind::ExecutionConfirmed => ConfirmationClass::ExecutionConfirmed,
                FeedbackKind::Unverified => ConfirmationClass::TriggeredUnverified,
                other => panic!("{}: the fakes should succeed, got {other:?}", entry.id),
            };
            assert_eq!(entry.verification, Verification::of(class), "{}", entry.id);
            assert_eq!(entry.token, action.token(), "{}", entry.id);
            checked += 1;
        }
        assert_eq!(checked, 9, "every entry with a backend is checked");
    }

    #[test]
    fn system_volume_is_confirmed_by_the_read_back() {
        use crate::action::{execute_volume, Outcome};
        let outcome = execute_volume(&FakeVolumeBackend::new(50), 52);
        assert_eq!(outcome, Outcome::StateConfirmed { volume_percent: 52 });
        assert_eq!(
            entry_named("systemVolume").verification,
            Verification::Confirmed
        );
    }

    #[test]
    fn runs_when_protected_is_what_is_system_says() {
        for entry in &CATALOG {
            if let Some(action) = example(entry.id) {
                assert_eq!(
                    entry.runs_when_protected,
                    action.is_system(),
                    "{}",
                    entry.id
                );
            }
        }
        // Not an Action: the knob's volume is allowed in a protected context (profile.rs).
        assert!(entry_named("systemVolume").runs_when_protected);
        assert!(!entry_named("macro").runs_when_protected);
    }

    #[test]
    fn macros_are_unsupported_on_every_os() {
        let services = Services::expected(Os::Windows);
        for os in [Os::Windows, Os::MacOs, Os::Other] {
            assert_eq!(
                entry_named("macro").availability(os, &services),
                ActionAvailability::Unsupported {
                    reason: "Coming soon".into()
                },
                "macro on {os:?}"
            );
        }
    }

    #[test]
    fn app_volume_and_mute_are_available_on_windows_and_unsupported_on_macos() {
        for id in ["appVolume", "appMute"] {
            assert_eq!(
                entry_named(id).availability(Os::Windows, &Services::expected(Os::Windows)),
                ActionAvailability::Available {
                    confirmation: ConfirmationClass::StateConfirmed
                },
                "{id}"
            );
            // Whatever the services claim, macOS never offers it.
            for services in [
                Services::expected(Os::MacOs),
                Services::expected(Os::Windows),
            ] {
                assert_eq!(
                    entry_named(id).availability(Os::MacOs, &services),
                    ActionAvailability::Unsupported {
                        reason: APP_VOLUME_UNSUPPORTED_ON_MAC.into()
                    },
                    "{id}"
                );
            }
            assert!(matches!(
                entry_named(id).availability(Os::Other, &Services::expected(Os::Other)),
                ActionAvailability::NotImplementedYet { .. }
            ));
        }
    }

    #[test]
    fn availability_follows_the_services() {
        let services = Services::expected(Os::MacOs);
        assert_eq!(
            entry_named("launch").availability(Os::MacOs, &services),
            ActionAvailability::Available {
                confirmation: ConfirmationClass::ExecutionConfirmed
            }
        );
        let no_endpoint = Services {
            volume: FakeVolumeBackend::with_availability(ActionAvailability::Unknown)
                .availability(),
            ..services.clone()
        };
        assert_eq!(
            entry_named("systemMute").availability(Os::MacOs, &no_endpoint),
            ActionAvailability::Unknown
        );
        let no_input = Services {
            input: ActionAvailability::NotImplementedYet { target: "linux" },
            ..services
        };
        assert_eq!(
            entry_named("shortcut").availability(Os::Other, &no_input),
            ActionAvailability::NotImplementedYet { target: "linux" }
        );
        // Media keys need input too, not the volume backend.
        assert!(matches!(
            entry_named("playPause").availability(Os::Other, &no_input),
            ActionAvailability::NotImplementedYet { .. }
        ));
    }

    #[test]
    fn tokens_are_unique() {
        let mut tokens: Vec<_> = ActionToken::ALL.iter().map(|t| t.token()).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), ActionToken::ALL.len());
    }
}
