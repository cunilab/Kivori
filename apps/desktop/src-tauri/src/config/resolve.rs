//! Layers the user's sparse overrides over the built-in profiles and validates them.
//!
//! An override replaces only the slots it names, so every other control keeps picking up built-in
//! fixes from app updates. "Reset control" removes the key; "reset profile" removes the entry.
//! Rejection reasons are fixed texts that never echo the input.

use kivori_model::desk::MediaText;

use super::schema::{ActionSpec, ConfigFile, ProfileId, ProfileOverride, RotateSpec, SlotSpec};
use super::store::ConfigError;
use super::ResolvedConfig;
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

fn shortcut(keys: &str) -> Result<Shortcut, &'static str> {
    keys.parse().map_err(|_| "not a valid shortcut")
}

/// The runnable action a spec names, validated (a shortcut parses, a launch target is plain).
///
/// # Errors
/// Returns a fixed reason for an invalid shortcut or application.
pub fn resolve_action(spec: &ActionSpec) -> Result<Action, &'static str> {
    Ok(match spec {
        ActionSpec::PlayPause => Action::PlayPause,
        ActionSpec::PreviousTrack => Action::PreviousTrack,
        ActionSpec::NextTrack => Action::NextTrack,
        ActionSpec::SystemMute => Action::ToggleMute,
        ActionSpec::Shortcut { keys } => Action::Shortcut(shortcut(keys)?),
        ActionSpec::Launch { target } => Action::Launch(
            validate_target(target)
                .map_err(|_| "not a valid application")?
                .to_string(),
        ),
    })
}

fn slot(spec: &SlotSpec) -> Result<Slot, &'static str> {
    let Some(bound) = &spec.action else {
        // Unbound: a label would have nothing to name.
        return Ok(Slot::default());
    };
    Ok(Slot {
        action: Some(resolve_action(bound)?),
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
    })
}

/// The spec of a built-in or resolved action (for the UI).
#[must_use]
pub fn action_spec(action: &Action) -> ActionSpec {
    match action {
        Action::PlayPause => ActionSpec::PlayPause,
        Action::PreviousTrack => ActionSpec::PreviousTrack,
        Action::NextTrack => ActionSpec::NextTrack,
        Action::ToggleMute => ActionSpec::SystemMute,
        Action::Shortcut(s) => ActionSpec::Shortcut {
            keys: s.to_string(),
        },
        Action::Launch(target) => ActionSpec::Launch {
            target: target.clone(),
        },
    }
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
    }
}

/// `spec` as it should be stored: validated, shortcuts canonical, text trimmed.
///
/// # Errors
/// A fixed reason naming what is wrong.
pub fn canonical_slot(spec: &SlotSpec) -> Result<SlotSpec, &'static str> {
    let resolved = slot(spec)?;
    Ok(SlotSpec {
        action: resolved.action.as_ref().map(action_spec),
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

fn apply(profile: &mut Profile, over: &ProfileOverride) -> Result<(), &'static str> {
    if let Some(spec) = &over.rotate {
        profile.rotate = rotate(spec)?;
    }
    let bindings = &mut profile.bindings;
    if let Some(spec) = &over.press {
        bindings.press = slot(spec)?;
    }
    if let Some(spec) = &over.hold {
        bindings.hold = slot(spec)?;
    }
    for (target, over) in bindings.buttons.iter_mut().zip(&over.buttons) {
        if let Some(spec) = &over.press {
            target.press = slot(spec)?;
        }
        if let Some(spec) = &over.hold {
            target.hold = slot(spec)?;
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
    for profile in &mut builtins {
        if let Some(over) = file.profiles.get(&profile.id) {
            apply(profile, over).map_err(ConfigError::Invalid)?;
        }
    }
    Ok(ResolvedConfig {
        profiles: builtins,
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
        let canon = canonical_slot(&spec(
            Some(ActionSpec::Shortcut {
                keys: "cmd + shift + m".into(),
            }),
            Some(" Mic "),
        ))
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
}
