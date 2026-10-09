//! Built-in profiles and the context that picks one (docs/product.md, context and profiles;
//! permissions and protected contexts).
//!
//! [`Context`] is pure: it is fed the observed foreground with an injected time. A new foreground
//! commits after [`STABLE_FOR`]; deliberate input commits a pending app at once (invariant 8);
//! Protected commits at once and beats everything, a pinned profile included (invariant 9).

use std::time::Duration;

use kivori_model::desk::{ContextMood, ControlLabels, MediaText};

use super::actions::{Action, Bindings, ButtonSlots, Slot};
use crate::config::ProfileId;
use crate::input::LogicalInput;
use crate::platform::{Foreground, Shortcut};

/// A new foreground owns the controls after it has been stable this long (product: 300-500 ms).
pub const STABLE_FOR: Duration = Duration::from_millis(400);

/// What the knob does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotateBinding {
    /// Master volume (the rotary volume loop).
    Volume,
    /// One shortcut per detent, one per direction (invariant 54: the binding owns both).
    Shortcuts {
        label: String,
        cw: Shortcut,
        ccw: Shortcut,
    },
    /// One app's volume (`app` is a lowercase foreground-style id), through the knob gesture path.
    AppVolume { app: String, label: String },
}

impl RotateBinding {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            RotateBinding::Volume => "Volume",
            RotateBinding::Shortcuts { label, .. } | RotateBinding::AppVolume { label, .. } => {
                label
            }
        }
    }
}

/// One profile: which apps it matches and what every control does there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    /// Lowercase foreground ids, Windows executables and macOS bundle ids in one list.
    pub ids: Vec<String>,
    pub mood: ContextMood,
    pub rotate: RotateBinding,
    pub bindings: Bindings,
}

fn shortcut(text: &str) -> Shortcut {
    text.parse()
        .unwrap_or_else(|e| panic!("built-in shortcut {text:?}: {e}"))
}

/// The built-in profiles, General first (the fallback). `mac` picks Command-based shortcuts.
#[must_use]
pub fn builtins_for(mac: bool) -> Vec<Profile> {
    let os = |windows: &str, macos: &str| shortcut(if mac { macos } else { windows });
    let named = |name: &str, s: Shortcut| ButtonSlots {
        press: Slot::named(Action::Shortcut(s), name),
        hold: Slot::default(),
    };
    let with_buttons = |buttons: [ButtonSlots; 3]| Bindings {
        buttons,
        ..Bindings::default()
    };
    let ids = |ids: &[&str]| ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    let general = Profile {
        id: ProfileId::General,
        name: "General".into(),
        ids: Vec::new(),
        mood: ContextMood::Neutral,
        rotate: RotateBinding::Volume,
        bindings: Bindings::default(),
    };
    vec![
        general.clone(),
        Profile {
            id: ProfileId::Browser,
            name: "Browser".into(),
            ids: ids(&[
                "chrome.exe",
                "msedge.exe",
                "firefox.exe",
                "brave.exe",
                "com.google.chrome",
                "com.apple.safari",
                "org.mozilla.firefox",
                "com.microsoft.edgemac",
                "com.brave.browser",
            ]),
            rotate: RotateBinding::Shortcuts {
                label: "Tabs".into(),
                cw: shortcut("Ctrl+Tab"),
                ccw: shortcut("Ctrl+Shift+Tab"),
            },
            bindings: with_buttons([
                named("Back", os("Alt+Left", "Meta+[")),
                named("Reload", os("Ctrl+R", "Meta+R")),
                named("New tab", os("Ctrl+T", "Meta+T")),
            ]),
            ..general.clone()
        },
        Profile {
            id: ProfileId::Code,
            name: "Code".into(),
            ids: ids(&["code.exe", "com.microsoft.vscode"]),
            bindings: with_buttons([
                named("Terminal", shortcut("Ctrl+`")),
                named("Run", shortcut("F5")),
                named("Git", shortcut("Ctrl+Shift+G")),
            ]),
            ..general.clone()
        },
        Profile {
            id: ProfileId::Media,
            name: "Media".into(),
            ids: ids(&["spotify.exe", "com.spotify.client", "com.apple.music"]),
            ..general.clone()
        },
        Profile {
            id: ProfileId::Zoom,
            name: "Zoom".into(),
            ids: ids(&["zoom.exe", "us.zoom.xos"]),
            mood: ContextMood::Meeting,
            bindings: with_buttons([
                named("Mic", os("Alt+A", "Meta+Shift+A")),
                named("Video", os("Alt+V", "Meta+Shift+V")),
                named("Leave", os("Alt+Q", "Meta+W")),
            ]),
            ..general.clone()
        },
        Profile {
            id: ProfileId::Teams,
            name: "Teams".into(),
            ids: ids(&["ms-teams.exe", "teams.exe", "com.microsoft.teams2"]),
            mood: ContextMood::Meeting,
            bindings: with_buttons([
                named("Mic", os("Ctrl+Shift+M", "Meta+Shift+M")),
                named("Video", os("Ctrl+Shift+O", "Meta+Shift+O")),
                named("Leave", os("Ctrl+Shift+H", "Meta+Shift+H")),
            ]),
            ..general
        },
    ]
}

/// The built-in profiles for this OS.
#[must_use]
pub fn builtins() -> Vec<Profile> {
    builtins_for(cfg!(target_os = "macos"))
}

/// The index of the profile matching foreground `id`, General (0) when none does.
#[must_use]
pub fn match_profile(profiles: &[Profile], id: &str) -> usize {
    profiles
        .iter()
        .position(|p| p.ids.iter().any(|known| known.eq_ignore_ascii_case(id)))
        .unwrap_or(0)
}

/// The contextual button whose Hold cycles the pinned profile.
pub const PIN_BUTTON: u8 = 1;

/// What a discrete input does under the active context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// Unbound: nothing happens.
    Nothing,
    Run(Action),
    /// Bound, but suspended in a protected context: it can't run and says so (invariant 19).
    Suspended(Action),
    /// The pin button's Hold: cycle the pinned profile.
    CyclePin,
}

/// Which profile owns the controls right now.
#[derive(Debug)]
pub struct Context {
    profiles: Vec<Profile>,
    committed: Foreground,
    pending: Option<(Foreground, Duration)>,
    /// The profile of the last committed app (or General for Unknown); kept under Protected.
    app_profile: usize,
    pinned: Option<usize>,
}

impl Context {
    #[must_use]
    pub fn new(profiles: Vec<Profile>) -> Self {
        assert!(!profiles.is_empty(), "General is always there");
        Self {
            profiles,
            committed: Foreground::Unknown,
            pending: None,
            app_profile: 0,
            pinned: None,
        }
    }

    /// One foreground observation at `now`. Protected commits at once; anything else commits once
    /// it has been observed unchanged for [`STABLE_FOR`].
    pub fn observe(&mut self, foreground: Foreground, now: Duration) {
        if foreground == self.committed {
            self.pending = None;
            return;
        }
        if foreground == Foreground::Protected {
            return self.commit(foreground);
        }
        match &self.pending {
            Some((seen, since)) if *seen == foreground => {
                if now.saturating_sub(*since) >= STABLE_FOR {
                    self.commit(foreground);
                }
            }
            _ => self.pending = Some((foreground, now)),
        }
    }

    /// Deliberate input: a pending app commits at once (invariant 8). A pending Unknown does not
    /// (General is never a stand-in during a race). Call after [`Self::observe`], so Protected is
    /// classified first.
    pub fn commit_pending(&mut self) {
        if let Some((app @ Foreground::App { .. }, _)) = &self.pending {
            let app = app.clone();
            self.commit(app);
        }
    }

    fn commit(&mut self, foreground: Foreground) {
        match &foreground {
            Foreground::App { id, .. } => self.app_profile = match_profile(&self.profiles, id),
            Foreground::Unknown => self.app_profile = 0,
            Foreground::Protected => {}
        }
        self.committed = foreground;
        self.pending = None;
    }

    /// Swaps in re-resolved profiles (same count and order: the config only overrides bindings).
    /// The pin and the committed app's profile are kept; `pending` is left alone.
    pub fn set_profiles(&mut self, profiles: Vec<Profile>) {
        assert!(!profiles.is_empty(), "General is always there");
        self.profiles = profiles;
        let last = self.profiles.len() - 1;
        self.pinned = self.pinned.map(|i| i.min(last));
        self.app_profile = match &self.committed {
            Foreground::App { id, .. } => match_profile(&self.profiles, id),
            Foreground::Unknown => 0,
            // Protected keeps the last app's profile.
            Foreground::Protected => self.app_profile.min(last),
        };
    }

    /// Auto -> each profile in order -> Auto.
    pub fn cycle_pin(&mut self) {
        self.pinned = match self.pinned {
            None => Some(0),
            Some(i) if i + 1 < self.profiles.len() => Some(i + 1),
            Some(_) => None,
        };
    }

    /// The most recent focus observation, committed or not: where synthesized keys land now.
    #[must_use]
    pub fn latest(&self) -> &Foreground {
        self.pending
            .as_ref()
            .map_or(&self.committed, |(seen, _)| seen)
    }

    #[must_use]
    pub fn protected(&self) -> bool {
        self.committed == Foreground::Protected
    }

    #[must_use]
    pub const fn pinned(&self) -> bool {
        self.pinned.is_some()
    }

    /// The profile whose bindings apply (under Protected, with custom actions suspended).
    #[must_use]
    pub fn profile(&self) -> &Profile {
        &self.profiles[self.pinned.unwrap_or(self.app_profile)]
    }

    /// The knob's binding, `None` while it is suspended (a shortcut knob in a protected context).
    #[must_use]
    pub fn rotate(&self) -> Option<&RotateBinding> {
        let rotate = &self.profile().rotate;
        // Volume (system or one app's) changes no input, so it runs in a protected context.
        let runs = matches!(
            rotate,
            RotateBinding::Volume | RotateBinding::AppVolume { .. }
        );
        (!self.protected() || runs).then_some(rotate)
    }

    /// What a discrete input does now.
    #[must_use]
    pub fn resolve(&self, input: &LogicalInput) -> Resolved {
        if matches!(
            input,
            LogicalInput::ButtonHold {
                button: PIN_BUTTON,
                ..
            }
        ) {
            return Resolved::CyclePin;
        }
        match super::bound_action(&self.profile().bindings, input) {
            None => Resolved::Nothing,
            Some(action) if self.protected() && !action.is_system() => {
                Resolved::Suspended(action.clone())
            }
            Some(action) => Resolved::Run(action.clone()),
        }
    }

    /// The device legend: labels and bindings come from the same state, so they switch together.
    #[must_use]
    pub fn labels(&self) -> ControlLabels {
        let profile = self.profile();
        let protected = self.protected();
        let bindings = &profile.bindings;
        let label = |slot: &Slot| {
            match &slot.action {
                // Unbound, or suspended in a protected context: nothing to show.
                None => MediaText::default(),
                Some(action) if protected && !action.is_system() => MediaText::default(),
                Some(_) => MediaText::from_text(&slot.device_label()),
            }
        };
        let name = if protected {
            "Protected"
        } else if self.pinned.is_none() && self.app_profile == 0 {
            // The Auto/General fallback keeps the screen calm.
            ""
        } else {
            profile.name.as_str()
        };
        ControlLabels {
            rotate: MediaText::from_text(self.rotate().map_or("", RotateBinding::label)),
            press: label(&bindings.press),
            hold: label(&bindings.hold),
            buttons: std::array::from_fn(|i| label(&bindings.buttons[i].press)),
            profile: MediaText::from_text(name),
            pinned: self.pinned(),
            mood: if protected {
                ContextMood::Neutral
            } else {
                profile.mood
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn app(id: &str) -> Foreground {
        Foreground::App {
            id: id.into(),
            name: id.into(),
        }
    }

    fn text(t: MediaText) -> String {
        t.as_latin1().iter().map(|&b| char::from(b)).collect()
    }

    fn name(ctx: &Context) -> &str {
        &ctx.profile().name
    }

    #[test]
    fn every_listed_id_matches_its_profile_on_both_oses() {
        for mac in [false, true] {
            let profiles = builtins_for(mac);
            let expect = [
                ("chrome.exe", "Browser"),
                ("msedge.exe", "Browser"),
                ("firefox.exe", "Browser"),
                ("brave.exe", "Browser"),
                ("com.google.chrome", "Browser"),
                ("com.apple.safari", "Browser"),
                ("org.mozilla.firefox", "Browser"),
                ("com.microsoft.edgemac", "Browser"),
                ("com.brave.browser", "Browser"),
                ("code.exe", "Code"),
                ("com.microsoft.vscode", "Code"),
                ("spotify.exe", "Media"),
                ("com.spotify.client", "Media"),
                ("com.apple.music", "Media"),
                ("zoom.exe", "Zoom"),
                ("us.zoom.xos", "Zoom"),
                ("ms-teams.exe", "Teams"),
                ("teams.exe", "Teams"),
                ("com.microsoft.teams2", "Teams"),
                ("notepad.exe", "General"),
                ("", "General"),
            ];
            for (id, profile) in expect {
                assert_eq!(profiles[match_profile(&profiles, id)].name, profile, "{id}");
            }
        }
    }

    #[test]
    fn built_in_shortcuts_follow_the_os() {
        let button = |p: &Profile, i: usize| match &p.bindings.buttons[i].press.action {
            Some(Action::Shortcut(s)) => s.to_string(),
            other => panic!("{other:?}"),
        };
        let win = builtins_for(false);
        let mac = builtins_for(true);
        assert_eq!(button(&win[1], 0), "Alt+Left");
        assert_eq!(button(&mac[1], 0), "Meta+[");
        assert_eq!(button(&mac[1], 2), "Meta+T");
        assert_eq!(button(&win[2], 0), "Ctrl+`");
        assert_eq!(
            button(&mac[2], 0),
            "Ctrl+`",
            "VS Code keeps Ctrl+` on macOS"
        );
        assert_eq!(button(&win[4], 2), "Alt+Q");
        assert_eq!(button(&mac[4], 2), "Meta+W");
        assert_eq!(button(&mac[5], 0), "Shift+Meta+M");
        assert_eq!(
            mac[1].rotate,
            RotateBinding::Shortcuts {
                label: "Tabs".into(),
                cw: shortcut("Ctrl+Tab"),
                ccw: shortcut("Ctrl+Shift+Tab"),
            }
        );
        assert_eq!(
            win[3].bindings, win[0].bindings,
            "Media is General's media row"
        );
        assert_eq!(win[4].mood, ContextMood::Meeting);
        assert_eq!(win[5].mood, ContextMood::Meeting);
    }

    #[test]
    fn a_new_foreground_commits_after_400_ms_of_stability() {
        let mut ctx = Context::new(builtins());
        ctx.observe(app("chrome.exe"), ms(0));
        ctx.observe(app("chrome.exe"), ms(300));
        assert_eq!(name(&ctx), "General", "not yet stable");
        // Focus bounces: the clock restarts.
        ctx.observe(app("code.exe"), ms(350));
        ctx.observe(app("chrome.exe"), ms(400));
        ctx.observe(app("chrome.exe"), ms(799));
        assert_eq!(name(&ctx), "General");
        ctx.observe(app("chrome.exe"), ms(800));
        assert_eq!(name(&ctx), "Browser");
        assert_eq!(text(ctx.labels().profile), "Browser");
        // Unknown also needs to be stable, then falls back to a calm General.
        ctx.observe(Foreground::Unknown, ms(900));
        assert_eq!(name(&ctx), "Browser");
        ctx.observe(Foreground::Unknown, ms(1_300));
        assert_eq!(name(&ctx), "General");
        assert_eq!(text(ctx.labels().profile), "", "the fallback is calm");
    }

    #[test]
    fn deliberate_input_commits_a_pending_app_at_once_but_not_unknown() {
        let mut ctx = Context::new(builtins());
        ctx.observe(app("zoom.exe"), ms(0));
        ctx.commit_pending();
        assert_eq!(name(&ctx), "Zoom");
        assert_eq!(ctx.labels().mood, ContextMood::Meeting);
        ctx.observe(Foreground::Unknown, ms(100));
        ctx.commit_pending();
        assert_eq!(name(&ctx), "Zoom", "General is never a stand-in in a race");
    }

    #[test]
    fn protected_commits_at_once_beats_a_pin_and_suspends_shortcuts() {
        let mut ctx = Context::new(builtins());
        ctx.cycle_pin();
        ctx.cycle_pin(); // Browser
        assert_eq!(name(&ctx), "Browser");
        ctx.observe(Foreground::Protected, ms(0));
        assert!(ctx.protected());
        let labels = ctx.labels();
        assert_eq!(text(labels.profile), "Protected");
        assert_eq!(text(labels.rotate), "", "the Tabs knob is suspended");
        assert!(labels.buttons.iter().all(MediaText::is_empty));
        assert_eq!(text(labels.press), "Play/Pause", "system actions still run");
        assert_eq!(text(labels.hold), "Mute");
        assert_eq!(ctx.rotate(), None);
        let button = LogicalInput::ButtonPress {
            button: 1,
            gesture_id: 1,
        };
        assert!(matches!(
            ctx.resolve(&button),
            Resolved::Suspended(Action::Shortcut(_))
        ));
        assert_eq!(
            ctx.resolve(&LogicalInput::Hold { gesture_id: 2 }),
            Resolved::Run(Action::ToggleMute)
        );
        // Leaving Protected follows the normal stability rule, then the pin applies again.
        ctx.observe(app("code.exe"), ms(100));
        assert!(ctx.protected());
        ctx.observe(app("code.exe"), ms(500));
        assert!(!ctx.protected());
        assert_eq!(name(&ctx), "Browser", "still pinned");
        assert_eq!(text(ctx.labels().rotate), "Tabs");
    }

    #[test]
    fn protected_keeps_the_volume_knob() {
        let mut ctx = Context::new(builtins());
        ctx.observe(Foreground::Protected, ms(0));
        assert_eq!(ctx.rotate(), Some(&RotateBinding::Volume));
        assert_eq!(text(ctx.labels().rotate), "Volume");
        assert_eq!(text(ctx.labels().buttons[0]), "Previous");
    }

    #[test]
    fn the_pin_cycles_through_every_profile_and_wraps_to_auto() {
        let mut ctx = Context::new(builtins());
        ctx.observe(app("spotify.exe"), ms(0));
        ctx.commit_pending();
        let mut seen: Vec<(String, String, bool)> = Vec::new();
        for _ in 0..7 {
            ctx.cycle_pin();
            let labels = ctx.labels();
            seen.push((name(&ctx).to_string(), text(labels.profile), labels.pinned));
        }
        let expect = [
            ("General", "General", true),
            ("Browser", "Browser", true),
            ("Code", "Code", true),
            ("Media", "Media", true),
            ("Zoom", "Zoom", true),
            ("Teams", "Teams", true),
            ("Media", "Media", false),
        ];
        let seen: Vec<_> = seen
            .iter()
            .map(|(n, t, p)| (n.as_str(), t.as_str(), *p))
            .collect();
        assert_eq!(seen, expect, "Auto follows the foreground app again");
    }

    #[test]
    fn a_pin_ignores_the_foreground() {
        let mut ctx = Context::new(builtins());
        ctx.cycle_pin(); // General
        ctx.observe(app("zoom.exe"), ms(0));
        ctx.observe(app("zoom.exe"), ms(400));
        assert_eq!(name(&ctx), "General");
        assert_eq!(
            text(ctx.labels().profile),
            "General",
            "a pinned General says so"
        );
        assert_eq!(
            ctx.resolve(&LogicalInput::ButtonHold {
                button: PIN_BUTTON,
                gesture_id: 1
            }),
            Resolved::CyclePin
        );
        assert_eq!(
            ctx.resolve(&LogicalInput::ButtonHold {
                button: 0,
                gesture_id: 2
            }),
            Resolved::Nothing
        );
    }

    #[test]
    fn button_labels_name_what_the_shortcut_does() {
        let mut ctx = Context::new(builtins());
        ctx.observe(app("com.microsoft.vscode"), ms(0));
        ctx.commit_pending();
        let labels = ctx.labels();
        assert_eq!(labels.buttons.map(text), ["Terminal", "Run", "Git"]);
        assert_eq!(text(labels.rotate), "Volume");
    }

    #[test]
    fn set_profiles_keeps_the_pin_and_the_committed_apps_profile() {
        let mut ctx = Context::new(builtins());
        ctx.observe(app("zoom.exe"), ms(0));
        ctx.commit_pending();
        assert_eq!(name(&ctx), "Zoom");
        let mut next = builtins();
        next[4].bindings.press = Slot::bound(Action::NextTrack);
        ctx.set_profiles(next.clone());
        assert_eq!(name(&ctx), "Zoom", "the committed app keeps its profile");
        assert_eq!(ctx.profile().bindings.press.action, Some(Action::NextTrack));

        ctx.cycle_pin();
        ctx.cycle_pin(); // Browser
        ctx.set_profiles(next);
        assert!(ctx.pinned());
        assert_eq!(name(&ctx), "Browser", "the pin survives the swap");

        // Under Protected the last app's profile is kept for when it ends.
        ctx.cycle_pin();
        ctx.cycle_pin();
        ctx.cycle_pin();
        ctx.cycle_pin();
        ctx.cycle_pin(); // back to Auto
        assert!(!ctx.pinned());
        ctx.observe(Foreground::Protected, ms(10));
        ctx.set_profiles(builtins());
        ctx.observe(app("zoom.exe"), ms(20));
        ctx.observe(app("zoom.exe"), ms(500));
        assert_eq!(name(&ctx), "Zoom");
    }

    #[test]
    fn a_button_hold_runs_its_bound_action_and_the_middle_one_still_pins() {
        let mut profiles = builtins();
        profiles[0].bindings.buttons[0].hold = Slot::bound(Action::ToggleMute);
        profiles[0].bindings.buttons[1].hold = Slot::bound(Action::ToggleMute);
        let ctx = Context::new(profiles);
        let hold = |button| LogicalInput::ButtonHold {
            button,
            gesture_id: 1,
        };
        assert_eq!(ctx.resolve(&hold(0)), Resolved::Run(Action::ToggleMute));
        assert_eq!(ctx.resolve(&hold(1)), Resolved::CyclePin);
        assert_eq!(ctx.resolve(&hold(2)), Resolved::Nothing);
    }
}
