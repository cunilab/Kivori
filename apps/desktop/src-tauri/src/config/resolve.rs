//! Layers the user's sparse overrides over the built-in profiles and validates them.
//!
//! An override replaces only the slots it names, so every other control keeps picking up built-in
//! fixes from app updates. "Reset control" removes the key; "reset profile" removes the entry.
//! Rejection reasons are fixed texts that never echo the input.

use std::sync::Arc;
use std::time::Duration;

use kivori_model::desk::MediaText;

use super::schema::{
    ActionSpec, ConfigFile, MacroSpec, ProfileId, ProfileOverride, RotateSpec, SlotSpec, StepSpec,
    DELAY_MS, MAX_MACROS, MAX_STEPS,
};
use super::store::ConfigError;
use super::ResolvedConfig;
use crate::desk::actions::{Macro, Macros, Step};
use crate::desk::profile::{Profile, RotateBinding};
use crate::desk::{Action, Slot};
use crate::platform::launch::validate_target;
use crate::platform::Shortcut;

/// A control `set_binding` can target (the knob has its own command).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Press,
    Hold,
    ButtonPress(u8),
    ButtonHold(u8),
}

/// Every token [`Control::from_token`] accepts, for the Rust/TS vocabulary check.
pub const CONTROL_TOKENS: [&str; 7] = [
    "press",
    "hold",
    "button1Press",
    "button1Hold",
    "button2Press",
    "button3Press",
    "button3Hold",
];

/// Every `kind` an [`ActionSpec`] serializes with, for the Rust/TS vocabulary check.
pub const ACTION_SPEC_KINDS: [&str; 8] = [
    "playPause",
    "previousTrack",
    "nextTrack",
    "systemMute",
    "appMute",
    "shortcut",
    "launch",
    "macro",
];

/// Every `kind` a [`StepSpec`] serializes with.
pub const STEP_SPEC_KINDS: [&str; 2] = ["action", "delay"];

/// Every `kind` a [`RotateSpec`] serializes with.
pub const ROTATE_SPEC_KINDS: [&str; 3] = ["systemVolume", "shortcuts", "appVolume"];

impl Control {
    /// Parses a webview control token.
    ///
    /// # Errors
    /// A fixed reason; `button2Hold` is reserved for the profile pin.
    pub fn from_token(token: &str) -> Result<Self, &'static str> {
        match token {
            "press" => Ok(Self::Press),
            "hold" => Ok(Self::Hold),
            "button1Press" => Ok(Self::ButtonPress(0)),
            "button1Hold" => Ok(Self::ButtonHold(0)),
            "button2Press" => Ok(Self::ButtonPress(1)),
            "button2Hold" => Err("the middle button's Hold is reserved for profile pin"),
            "button3Press" => Ok(Self::ButtonPress(2)),
            "button3Hold" => Ok(Self::ButtonHold(2)),
            _ => Err("unknown control"),
        }
    }

    /// Sets (or, with `None`, resets) this control's override.
    pub fn set(self, profile: &mut ProfileOverride, slot: Option<SlotSpec>) {
        match self {
            Self::Press => profile.press = slot,
            Self::Hold => profile.hold = slot,
            Self::ButtonPress(i) => profile.buttons[usize::from(i)].press = slot,
            Self::ButtonHold(i) => profile.buttons[usize::from(i)].hold = slot,
        }
    }
}

/// A label the device can draw: trimmed, 1 to 32 Latin-1 characters, whitespace collapsed.
fn label(text: &str) -> Result<String, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Err("a label cannot be empty");
    }
    if text.chars().count() > MediaText::CAPACITY {
        return Err("a label is at most 32 characters");
    }
    if text.chars().any(|c| c.is_control() || u32::from(c) > 0xFF) {
        return Err("a label can only use Latin-1 characters");
    }
    Ok(MediaText::from_text(text)
        .as_latin1()
        .iter()
        .map(|&b| char::from(b))
        .collect())
}

/// The longest app id accepted (a Windows executable file name is far shorter).
const APP_ID_MAX: usize = 128;

/// An app id as stored: trimmed and lowercased, 1 to 128 characters, a bare name (no path).
fn app_id(app: &str) -> Result<String, &'static str> {
    let app = app.trim().to_lowercase();
    if app.is_empty() {
        return Err("an app cannot be empty");
    }
    if app.chars().count() > APP_ID_MAX {
        return Err("an app is at most 128 characters");
    }
    if app.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        return Err("an app is a name, not a path");
    }
    Ok(app)
}

/// The device label for an app with no name of its own: `spotify.exe` -> `spotify`, cut to what
/// the device can draw (an app id may be any text).
fn default_app_label(app: &str) -> String {
    let name: String = app
        .strip_suffix(".exe")
        .unwrap_or(app)
        .chars()
        .filter(|c| !c.is_control() && u32::from(*c) <= 0xFF)
        .take(MediaText::CAPACITY)
        .collect();
    if name.trim().is_empty() {
        "App volume".to_string()
    } else {
        name
    }
}

fn shortcut(keys: &str) -> Result<Shortcut, &'static str> {
    keys.parse().map_err(|_| "not a valid shortcut")
}

/// A macro id: 1 to 64 of `a-z 0-9 - _` (it is stored in bindings and never shown).
fn macro_id(id: &str) -> Result<(), &'static str> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    ok.then_some(())
        .ok_or("a macro id is 1 to 64 of a-z, 0-9, - and _")
}

/// The runnable action a spec names, validated (a shortcut parses, a launch target is plain, a
/// macro exists in `macros`).
///
/// # Errors
/// Returns a fixed reason for an invalid shortcut or application, or an unknown macro.
pub fn resolve_action(spec: &ActionSpec, macros: &Macros) -> Result<Action, &'static str> {
    Ok(match spec {
        ActionSpec::Macro { id } => {
            Action::Macro(Arc::clone(macros.get(id.as_str()).ok_or("unknown macro")?))
        }
        ActionSpec::PlayPause => Action::PlayPause,
        ActionSpec::PreviousTrack => Action::PreviousTrack,
        ActionSpec::NextTrack => Action::NextTrack,
        ActionSpec::SystemMute => Action::ToggleMute,
        ActionSpec::AppMute { app } => Action::AppMute { app: app_id(app)? },
        ActionSpec::Shortcut { keys } => Action::Shortcut(shortcut(keys)?),
        ActionSpec::Launch { target } => Action::Launch(
            validate_target(target)
                .map_err(|_| "not a valid application")?
                .to_string(),
        ),
    })
}

fn slot(spec: &SlotSpec, macros: &Macros) -> Result<Slot, &'static str> {
    let Some(bound) = &spec.action else {
        // Unbound: a label would have nothing to name.
        return Ok(Slot::default());
    };
    Ok(Slot {
        action: Some(resolve_action(bound, macros)?),
        label: spec.label.as_deref().map(label).transpose()?,
    })
}

fn rotate(spec: &RotateSpec) -> Result<RotateBinding, &'static str> {
    Ok(match spec {
        RotateSpec::SystemVolume => RotateBinding::Volume,
        RotateSpec::Shortcuts {
            cw,
            ccw,
            label: name,
        } => RotateBinding::Shortcuts {
            label: label(name)?,
            cw: shortcut(cw)?,
            ccw: shortcut(ccw)?,
        },
        RotateSpec::AppVolume { app, label: name } => {
            let app = app_id(app)?;
            let named = match name {
                Some(name) => label(name)?,
                None => default_app_label(&app),
            };
            RotateBinding::AppVolume { app, label: named }
        }
    })
}

/// The action a knob's Test button runs: a volume knob nudges one detent and puts it back, a
/// shortcut pair sends its clockwise shortcut once.
///
/// # Errors
/// Returns a fixed reason for an invalid shortcut or application.
pub fn resolve_rotate_test(spec: &RotateSpec) -> Result<Action, &'static str> {
    Ok(match rotate(spec)? {
        RotateBinding::Volume => Action::VolumeNudge { app: None },
        RotateBinding::AppVolume { app, .. } => Action::VolumeNudge { app: Some(app) },
        RotateBinding::Shortcuts { cw, .. } => Action::Shortcut(cw),
    })
}

/// The spec of a built-in or resolved action (for the UI). `None` for a knob step, which no slot
/// binds.
#[must_use]
pub fn action_spec(action: &Action) -> Option<ActionSpec> {
    Some(match action {
        Action::PlayPause => ActionSpec::PlayPause,
        Action::PreviousTrack => ActionSpec::PreviousTrack,
        Action::NextTrack => ActionSpec::NextTrack,
        Action::ToggleMute => ActionSpec::SystemMute,
        Action::AppMute { app } => ActionSpec::AppMute { app: app.clone() },
        Action::Shortcut(s) => ActionSpec::Shortcut {
            keys: s.to_string(),
        },
        Action::Launch(target) => ActionSpec::Launch {
            target: target.clone(),
        },
        Action::Macro(m) => ActionSpec::Macro { id: m.id.clone() },
        Action::AppVolumeStep { .. } | Action::VolumeNudge { .. } => return None,
    })
}

/// Validates one macro: limits, name, id, and steps that are never macros.
///
/// # Errors
/// A fixed reason naming what is wrong.
pub fn resolve_macro(spec: &MacroSpec) -> Result<Macro, &'static str> {
    macro_id(&spec.id)?;
    if spec.steps.len() > MAX_STEPS {
        return Err("a macro has at most 8 steps");
    }
    let no_macros = Macros::new();
    let mut steps = Vec::with_capacity(spec.steps.len());
    for step in &spec.steps {
        steps.push(match step {
            StepSpec::Action {
                action: ActionSpec::Macro { .. },
            } => return Err("a macro cannot contain a macro"),
            StepSpec::Action { action } => Step::Run(resolve_action(action, &no_macros)?),
            StepSpec::Delay { ms } if DELAY_MS.contains(ms) => {
                Step::Delay(Duration::from_millis(u64::from(*ms)))
            }
            StepSpec::Delay { .. } => return Err("a delay is 50 to 2000 ms"),
        });
    }
    if !steps.iter().any(|step| matches!(step, Step::Run(_))) {
        return Err("a macro needs at least one action step");
    }
    Ok(Macro {
        id: spec.id.clone(),
        name: label(&spec.name)?,
        steps,
    })
}

/// The spec of a resolved macro (for the UI and for storing it canonically).
#[must_use]
pub fn macro_spec(m: &Macro) -> MacroSpec {
    MacroSpec {
        id: m.id.clone(),
        name: m.name.clone(),
        steps: m
            .steps
            .iter()
            .filter_map(|step| {
                Some(match step {
                    Step::Run(action) => StepSpec::Action {
                        action: action_spec(action)?,
                    },
                    Step::Delay(wait) => StepSpec::Delay {
                        ms: u16::try_from(wait.as_millis()).unwrap_or(u16::MAX),
                    },
                })
            })
            .collect(),
    }
}

/// [`macro_spec`] of the validated `spec`: shortcuts canonical, text trimmed.
///
/// # Errors
/// A fixed reason naming what is wrong.
pub fn canonical_macro(spec: &MacroSpec) -> Result<MacroSpec, &'static str> {
    resolve_macro(spec).map(|m| macro_spec(&m))
}

/// Every macro of `file`, validated, by id.
fn resolve_macros(file: &ConfigFile) -> Result<Macros, &'static str> {
    if file.macros.len() > MAX_MACROS {
        return Err("at most 32 macros");
    }
    let mut macros = Macros::new();
    for spec in &file.macros {
        let m = resolve_macro(spec)?;
        if macros.insert(m.id.clone(), Arc::new(m)).is_some() {
            return Err("two macros share an id");
        }
    }
    Ok(macros)
}

/// The spec of a resolved knob binding (for the UI).
#[must_use]
pub fn rotate_spec(binding: &RotateBinding) -> RotateSpec {
    match binding {
        RotateBinding::Volume => RotateSpec::SystemVolume,
        RotateBinding::Shortcuts { label, cw, ccw } => RotateSpec::Shortcuts {
            cw: cw.to_string(),
            ccw: ccw.to_string(),
            label: label.clone(),
        },
        RotateBinding::AppVolume { app, label } => RotateSpec::AppVolume {
            app: app.clone(),
            label: Some(label.clone()),
        },
    }
}

/// `spec` as it should be stored: validated, shortcuts canonical, text trimmed.
///
/// # Errors
/// A fixed reason naming what is wrong.
pub fn canonical_slot(spec: &SlotSpec, macros: &Macros) -> Result<SlotSpec, &'static str> {
    let resolved = slot(spec, macros)?;
    Ok(SlotSpec {
        action: resolved.action.as_ref().and_then(action_spec),
        label: resolved.label,
    })
}

/// [`canonical_slot`] for the knob.
///
/// # Errors
/// A fixed reason naming what is wrong.
pub fn canonical_rotate(spec: &RotateSpec) -> Result<RotateSpec, &'static str> {
    rotate(spec).map(|binding| rotate_spec(&binding))
}

fn apply(
    profile: &mut Profile,
    over: &ProfileOverride,
    macros: &Macros,
) -> Result<(), &'static str> {
    if let Some(spec) = &over.rotate {
        profile.rotate = rotate(spec)?;
    }
    let bindings = &mut profile.bindings;
    if let Some(spec) = &over.press {
        bindings.press = slot(spec, macros)?;
    }
    if let Some(spec) = &over.hold {
        bindings.hold = slot(spec, macros)?;
    }
    for (target, over) in bindings.buttons.iter_mut().zip(&over.buttons) {
        if let Some(spec) = &over.press {
            target.press = slot(spec, macros)?;
        }
        if let Some(spec) = &over.hold {
            target.hold = slot(spec, macros)?;
        }
    }
    if over.buttons[1].hold.is_some() {
        return Err("the middle button's Hold is reserved for profile pin");
    }
    Ok(())
}

/// Layers `file`'s overrides over `builtins`.
///
/// # Errors
/// [`ConfigError::Invalid`] for the first override that is not acceptable.
pub fn resolve(
    mut builtins: Vec<Profile>,
    file: &ConfigFile,
) -> Result<ResolvedConfig, ConfigError> {
    let macros = resolve_macros(file).map_err(ConfigError::Invalid)?;
    for profile in &mut builtins {
        if let Some(over) = file.profiles.get(&profile.id) {
            apply(profile, over, &macros).map_err(ConfigError::Invalid)?;
        }
    }
    Ok(ResolvedConfig {
        profiles: builtins,
        macros,
        display: file.display,
        buddy: file.buddy,
    })
}

/// Whether `profile` has any override in `file` (the UI's "Custom" badge input lives per slot).
#[must_use]
pub fn overrides(file: &ConfigFile, profile: ProfileId) -> Option<&ProfileOverride> {
    file.profiles.get(&profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk::profile::builtins_for;

    fn file_with(id: ProfileId, over: ProfileOverride) -> ConfigFile {
        let mut file = ConfigFile::default();
        file.profiles.insert(id, over);
        file
    }

    fn spec(action: Option<ActionSpec>, label: Option<&str>) -> SlotSpec {
        SlotSpec {
            action,
            label: label.map(str::to_string),
        }
    }

    fn run(mac: bool, file: &ConfigFile) -> Result<ResolvedConfig, ConfigError> {
        resolve(builtins_for(mac), file)
    }

    fn invalid(file: &ConfigFile) -> &'static str {
        match run(false, file) {
            Err(ConfigError::Invalid(reason)) => reason,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_rotate_test_nudges_a_volume_and_sends_a_shortcut_pairs_clockwise_key_once() {
        assert_eq!(
            resolve_rotate_test(&RotateSpec::SystemVolume),
            Ok(Action::VolumeNudge { app: None })
        );
        assert_eq!(
            resolve_rotate_test(&RotateSpec::AppVolume {
                app: "Spotify.exe".into(),
                label: None
            }),
            Ok(Action::VolumeNudge {
                app: Some("spotify.exe".into())
            })
        );
        let pair = RotateSpec::Shortcuts {
            cw: "Ctrl+Tab".into(),
            ccw: "Ctrl+Shift+Tab".into(),
            label: "Tabs".into(),
        };
        assert_eq!(
            resolve_rotate_test(&pair),
            Ok(Action::Shortcut("Ctrl+Tab".parse().unwrap()))
        );
        let bad = RotateSpec::Shortcuts {
            cw: "nonsense".into(),
            ccw: "Ctrl+Tab".into(),
            label: "Tabs".into(),
        };
        assert!(resolve_rotate_test(&bad).is_err());
    }

    #[test]
    fn no_overrides_resolve_to_the_built_ins_on_both_oses() {
        for mac in [false, true] {
            let resolved = run(mac, &ConfigFile::default()).unwrap();
            assert_eq!(resolved.profiles, builtins_for(mac));
        }
    }

    #[test]
    fn overriding_press_leaves_every_other_slot_built_in() {
        let over = ProfileOverride {
            press: Some(spec(Some(ActionSpec::NextTrack), None)),
            ..ProfileOverride::default()
        };
        let file = file_with(ProfileId::Zoom, over);
        for mac in [false, true] {
            let resolved = run(mac, &file).unwrap();
            let built = builtins_for(mac);
            assert_eq!(
                resolved.profiles[4].bindings.press.action,
                Some(Action::NextTrack)
            );
            assert_eq!(resolved.profiles[4].bindings.hold, built[4].bindings.hold);
            assert_eq!(
                resolved.profiles[4].bindings.buttons,
                built[4].bindings.buttons
            );
            assert_eq!(resolved.profiles[4].rotate, built[4].rotate);
            assert_eq!(
                resolved.profiles[1], built[1],
                "other profiles are untouched"
            );
        }
    }

    #[test]
    fn a_null_action_unbinds_and_drops_the_label() {
        let over = ProfileOverride {
            hold: Some(spec(None, Some("ignored"))),
            ..ProfileOverride::default()
        };
        let resolved = run(false, &file_with(ProfileId::General, over)).unwrap();
        assert_eq!(resolved.profiles[0].bindings.hold, Slot::default());
    }

    #[test]
    fn a_button_hold_and_a_label_resolve_and_a_rotate_override_replaces_the_knob() {
        let mut over = ProfileOverride::default();
        over.buttons[0].hold = Some(spec(
            Some(ActionSpec::Launch {
                target: " Spotify ".into(),
            }),
            Some("  My   music "),
        ));
        over.rotate = Some(RotateSpec::Shortcuts {
            cw: "ctrl+right".into(),
            ccw: "Ctrl+Left".into(),
            label: "Seek".into(),
        });
        let resolved = run(false, &file_with(ProfileId::Media, over)).unwrap();
        let media = &resolved.profiles[3];
        assert_eq!(
            media.bindings.buttons[0].hold,
            Slot::named(Action::Launch("Spotify".into()), "My music")
        );
        assert_eq!(media.rotate.label(), "Seek");
    }

    #[test]
    fn app_volume_and_app_mute_resolve_with_a_lowercase_app_id_and_a_default_label() {
        let over = ProfileOverride {
            rotate: Some(RotateSpec::AppVolume {
                app: "  Spotify.EXE ".into(),
                label: None,
            }),
            press: Some(spec(
                Some(ActionSpec::AppMute {
                    app: "Spotify.exe".into(),
                }),
                None,
            )),
            ..ProfileOverride::default()
        };
        let resolved = run(false, &file_with(ProfileId::Media, over)).unwrap();
        let media = &resolved.profiles[3];
        assert_eq!(
            media.rotate,
            RotateBinding::AppVolume {
                app: "spotify.exe".into(),
                label: "spotify".into()
            }
        );
        assert_eq!(
            media.bindings.press,
            Slot::bound(Action::AppMute {
                app: "spotify.exe".into()
            })
        );
        // The UI gets the canonical spec back, and it resolves to the same binding.
        let spec = rotate_spec(&media.rotate);
        assert_eq!(canonical_rotate(&spec).unwrap(), spec);
    }

    #[test]
    fn the_spec_kind_vocabulary_lists_every_variant() {
        let kind_of = |value: serde_json::Value| value["kind"].as_str().unwrap().to_string();
        let actions = [
            ActionSpec::PlayPause,
            ActionSpec::PreviousTrack,
            ActionSpec::NextTrack,
            ActionSpec::SystemMute,
            ActionSpec::AppMute { app: "a".into() },
            ActionSpec::Shortcut { keys: "A".into() },
            ActionSpec::Launch { target: "a".into() },
            ActionSpec::Macro { id: "a".into() },
        ];
        let kinds: Vec<_> = actions
            .iter()
            .map(|a| kind_of(serde_json::to_value(a).unwrap()))
            .collect();
        assert_eq!(kinds, ACTION_SPEC_KINDS);
        let steps = [
            StepSpec::Action {
                action: ActionSpec::PlayPause,
            },
            StepSpec::Delay { ms: 50 },
        ];
        let kinds: Vec<_> = steps
            .iter()
            .map(|s| kind_of(serde_json::to_value(s).unwrap()))
            .collect();
        assert_eq!(kinds, STEP_SPEC_KINDS);
        let rotates = [
            RotateSpec::SystemVolume,
            RotateSpec::Shortcuts {
                cw: "A".into(),
                ccw: "B".into(),
                label: "L".into(),
            },
            RotateSpec::AppVolume {
                app: "a".into(),
                label: None,
            },
        ];
        let kinds: Vec<_> = rotates
            .iter()
            .map(|r| kind_of(serde_json::to_value(r).unwrap()))
            .collect();
        assert_eq!(kinds, ROTATE_SPEC_KINDS);
    }

    #[test]
    fn a_bad_app_id_is_rejected() {
        let rotate = |app: &str| {
            file_with(
                ProfileId::Media,
                ProfileOverride {
                    rotate: Some(RotateSpec::AppVolume {
                        app: app.into(),
                        label: None,
                    }),
                    ..ProfileOverride::default()
                },
            )
        };
        assert_eq!(invalid(&rotate("  ")), "an app cannot be empty");
        assert_eq!(
            invalid(&rotate(&"a".repeat(129))),
            "an app is at most 128 characters"
        );
        assert_eq!(
            invalid(&rotate(r"C:\Apps\spotify.exe")),
            "an app is a name, not a path"
        );
        assert!(run(false, &rotate(&"a".repeat(128))).is_ok());
    }

    #[test]
    fn reset_removes_the_override_and_the_built_in_returns() {
        let mut file = file_with(
            ProfileId::General,
            ProfileOverride {
                press: Some(spec(Some(ActionSpec::NextTrack), None)),
                ..ProfileOverride::default()
            },
        );
        Control::Press.set(file.profiles.get_mut(&ProfileId::General).unwrap(), None);
        assert!(file.profiles[&ProfileId::General].is_empty());
        file.profiles.remove(&ProfileId::General);
        assert_eq!(run(false, &file).unwrap().profiles, builtins_for(false));
    }

    #[test]
    fn bad_shortcuts_labels_targets_and_the_pin_button_are_rejected() {
        let one = |press: SlotSpec| {
            file_with(
                ProfileId::Code,
                ProfileOverride {
                    press: Some(press),
                    ..ProfileOverride::default()
                },
            )
        };
        let shortcut_of =
            |keys: &str| one(spec(Some(ActionSpec::Shortcut { keys: keys.into() }), None));
        assert_eq!(invalid(&shortcut_of("Ctrl+")), "not a valid shortcut");
        assert_eq!(invalid(&shortcut_of("Ctrl+A+B")), "not a valid shortcut");
        let labelled = |text: &str| one(spec(Some(ActionSpec::PlayPause), Some(text)));
        assert_eq!(
            invalid(&labelled(&"x".repeat(33))),
            "a label is at most 32 characters"
        );
        assert_eq!(
            invalid(&labelled("play \u{25B6}")),
            "a label can only use Latin-1 characters"
        );
        assert_eq!(invalid(&labelled("  ")), "a label cannot be empty");
        assert!(run(false, &labelled(&"x".repeat(32))).is_ok());
        assert!(
            run(false, &labelled("caf\u{e9}")).is_ok(),
            "Latin-1 is fine"
        );
        let launch = |target: &str| {
            one(spec(
                Some(ActionSpec::Launch {
                    target: target.into(),
                }),
                None,
            ))
        };
        assert_eq!(invalid(&launch("  ")), "not a valid application");
        assert_eq!(invalid(&launch("a\0b")), "not a valid application");
        let mut pin = ProfileOverride::default();
        pin.buttons[1].hold = Some(spec(Some(ActionSpec::PlayPause), None));
        assert_eq!(
            invalid(&file_with(ProfileId::Code, pin)),
            "the middle button's Hold is reserved for profile pin"
        );
        let bad_knob = ProfileOverride {
            rotate: Some(RotateSpec::Shortcuts {
                cw: "nope".into(),
                ccw: "Ctrl+Left".into(),
                label: "Seek".into(),
            }),
            ..ProfileOverride::default()
        };
        assert_eq!(
            invalid(&file_with(ProfileId::Code, bad_knob)),
            "not a valid shortcut"
        );
    }

    #[test]
    fn control_tokens_parse_and_the_pin_hold_is_refused() {
        for token in CONTROL_TOKENS {
            assert!(Control::from_token(token).is_ok(), "{token}");
        }
        assert!(Control::from_token("button2Hold").is_err());
        assert!(Control::from_token("rotate").is_err());
    }

    #[test]
    fn stored_specs_are_canonical() {
        let canon = canonical_slot(
            &spec(
                Some(ActionSpec::Shortcut {
                    keys: "cmd + shift + m".into(),
                }),
                Some(" Mic "),
            ),
            &Macros::new(),
        )
        .unwrap();
        assert_eq!(
            canon,
            spec(
                Some(ActionSpec::Shortcut {
                    keys: "Shift+Meta+M".into()
                }),
                Some("Mic")
            )
        );
    }

    #[test]
    fn the_file_round_trips_through_json_sparsely() {
        let mut over = ProfileOverride::default();
        over.buttons[2].hold = Some(spec(None, None));
        let file = file_with(ProfileId::Zoom, over);
        let json = serde_json::to_string(&file).unwrap();
        assert!(
            json.contains(r#""buttons":[{},{},{"hold":{"action":null}}]"#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<ConfigFile>(&json).unwrap(), file);
    }

    fn step(action: ActionSpec) -> StepSpec {
        StepSpec::Action { action }
    }

    fn macro_of(id: &str, steps: Vec<StepSpec>) -> MacroSpec {
        MacroSpec {
            id: id.into(),
            name: "Standup".into(),
            steps,
        }
    }

    fn with_macro(m: MacroSpec) -> ConfigFile {
        ConfigFile {
            macros: vec![m],
            ..ConfigFile::default()
        }
    }

    fn bound_to(file: &mut ConfigFile, id: &str) {
        file.profiles.insert(
            ProfileId::Zoom,
            ProfileOverride {
                press: Some(spec(Some(ActionSpec::Macro { id: id.into() }), None)),
                ..ProfileOverride::default()
            },
        );
    }

    #[test]
    fn a_macro_resolves_by_id_into_a_shared_arc_that_a_binding_runs() {
        let mut file = with_macro(macro_of(
            "standup",
            vec![
                step(ActionSpec::SystemMute),
                StepSpec::Delay { ms: 500 },
                step(ActionSpec::Shortcut {
                    keys: "ctrl+m".into(),
                }),
            ],
        ));
        bound_to(&mut file, "standup");
        let resolved = run(false, &file).unwrap();
        let m = &resolved.macros["standup"];
        assert_eq!(m.steps.len(), 3);
        assert_eq!(m.steps[1], Step::Delay(Duration::from_millis(500)));
        let Some(Action::Macro(bound)) = &resolved.profiles[4].bindings.press.action else {
            panic!("press is not a macro");
        };
        assert!(Arc::ptr_eq(bound, m), "the binding shares the table's Arc");
        // The device names it after the macro; the UI gets the canonical spec back.
        assert_eq!(
            resolved.profiles[4].bindings.press.device_label(),
            "Standup"
        );
        assert_eq!(
            macro_spec(m).steps[2],
            step(ActionSpec::Shortcut {
                keys: "Ctrl+M".into()
            })
        );
    }

    #[test]
    fn a_binding_to_an_unknown_macro_is_rejected() {
        let mut file = ConfigFile::default();
        bound_to(&mut file, "ghost");
        assert_eq!(invalid(&file), "unknown macro");
        let mut file = with_macro(macro_of("a", vec![step(ActionSpec::PlayPause)]));
        bound_to(&mut file, "b");
        assert_eq!(invalid(&file), "unknown macro");
    }

    #[test]
    fn macro_limits_steps_delays_nesting_and_ids_are_enforced() {
        let play = || step(ActionSpec::PlayPause);
        let reason = |m: MacroSpec| invalid(&with_macro(m));
        assert!(run(false, &with_macro(macro_of("m", vec![play(); 8]))).is_ok());
        assert_eq!(
            reason(macro_of("m", vec![play(); 9])),
            "a macro has at most 8 steps"
        );
        let delayed = |ms| macro_of("m", vec![play(), StepSpec::Delay { ms }]);
        assert!(run(false, &with_macro(delayed(50))).is_ok());
        assert!(run(false, &with_macro(delayed(2000))).is_ok());
        assert_eq!(reason(delayed(49)), "a delay is 50 to 2000 ms");
        assert_eq!(reason(delayed(2001)), "a delay is 50 to 2000 ms");
        assert_eq!(
            reason(macro_of(
                "m",
                vec![step(ActionSpec::Macro { id: "m".into() })]
            )),
            "a macro cannot contain a macro"
        );
        assert_eq!(
            reason(macro_of("m", vec![])),
            "a macro needs at least one action step"
        );
        assert_eq!(
            reason(macro_of("m", vec![StepSpec::Delay { ms: 100 }])),
            "a macro needs at least one action step"
        );
        for bad in ["", "Has Space", "UPPER", &"a".repeat(65)] {
            assert_eq!(
                reason(macro_of(bad, vec![play()])),
                "a macro id is 1 to 64 of a-z, 0-9, - and _"
            );
        }
        let mut named = macro_of("m", vec![play()]);
        named.name = "x".repeat(33);
        assert_eq!(reason(named), "a label is at most 32 characters");
    }

    #[test]
    fn at_most_32_macros_with_unique_ids() {
        let many = |n: usize| ConfigFile {
            macros: (0..n)
                .map(|i| macro_of(&format!("m{i}"), vec![step(ActionSpec::PlayPause)]))
                .collect(),
            ..ConfigFile::default()
        };
        assert!(run(false, &many(32)).is_ok());
        assert_eq!(invalid(&many(33)), "at most 32 macros");
        let mut dup = many(2);
        dup.macros[1].id = "m0".into();
        assert_eq!(invalid(&dup), "two macros share an id");
    }
}
