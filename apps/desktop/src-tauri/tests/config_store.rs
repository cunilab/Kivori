//! The config file: first run, round trip, recovery of a bad file, downgrade and reset.

use std::fs;
use std::path::Path;

use kivori_desktop::config::{
    ActionSpec, ConfigError, ConfigFile, ConfigNotice, ConfigStore, DisplaySettings, Intensity,
    MacroSpec, ProfileId, ProfileOverride, SecondaryView, SlotSpec, StepSpec, View,
};

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn changed(store: &ConfigStore) -> ConfigFile {
    store.edited(|file| {
        file.display = DisplaySettings {
            default_view: View::Clock,
            secondary_view: SecondaryView::Cycle,
            brightness: 2,
        };
        file.buddy.reactions = false;
        file.buddy.intensity = Intensity::High;
    })
}

#[test]
fn a_missing_file_gives_defaults_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigStore::open(dir.path());
    assert_eq!(store.file(), &ConfigFile::default());
    assert_eq!(store.notice(), None);
    assert_eq!(store.revision(), 0);
    assert!(names(dir.path()).is_empty());
}

#[test]
fn a_missing_directory_is_created_on_the_first_save() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("not").join("yet");
    let mut store = ConfigStore::open(&inner);
    assert!(!inner.exists());
    store.save(changed(&store)).unwrap();
    assert_eq!(names(&inner), ["config.json"]);
}

#[test]
fn save_then_reopen_round_trips_and_leaves_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    let next = changed(&store);
    store.save(next.clone()).unwrap();
    assert_eq!(store.file(), &next);
    assert_eq!(store.revision(), 1);
    assert_eq!(names(dir.path()), ["config.json"]);

    let reopened = ConfigStore::open(dir.path());
    assert_eq!(reopened.file(), &next);
    assert_eq!(reopened.notice(), None);
}

#[test]
fn the_file_uses_the_documented_shape() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    store.save(changed(&store)).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("config.json")).unwrap()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "version": 1,
            "profiles": {},
            "customProfiles": [],
            "macros": [],
            "display": { "defaultView": "clock", "secondaryView": "cycle", "brightness": 2 },
            "buddy": { "reactions": false, "intensity": "high" },
            "onboarding": { "completed": false },
            "updates": { "autoCheck": true, "skippedVersion": null, "snoozed": null }
        })
    );
}

#[test]
fn update_settings_default_to_checking_and_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    // A file from before the field existed: the automatic check is on, nothing skipped or snoozed.
    fs::write(
        dir.path().join("config.json"),
        r#"{ "version": 1, "onboarding": { "completed": true } }"#,
    )
    .unwrap();
    let mut store = ConfigStore::open(dir.path());
    assert_eq!(store.notice(), None);
    assert_eq!(
        store.file().updates,
        kivori_desktop::config::UpdateSettings::default()
    );
    assert!(store.file().updates.auto_check);
    let next = store.edited(|file| {
        file.updates.auto_check = false;
        file.updates.skipped_version = Some("0.5.0".to_string());
        file.updates.snoozed = Some(kivori_desktop::config::UpdateSnooze {
            version: "0.6.0".to_string(),
            at: 1_800_000_000,
        });
    });
    store.save(next.clone()).unwrap();
    let reopened = ConfigStore::open(dir.path());
    assert_eq!(reopened.file().updates, next.updates);
    // A partial section fills in the rest from the defaults.
    fs::write(
        dir.path().join("config.json"),
        r#"{ "version": 1, "updates": { "skippedVersion": "0.5.0" } }"#,
    )
    .unwrap();
    let partial = ConfigStore::open(dir.path());
    assert!(partial.file().updates.auto_check);
    assert_eq!(
        partial.file().updates.skipped_version.as_deref(),
        Some("0.5.0")
    );
}

#[test]
fn an_unknown_update_field_is_set_aside_like_any_other() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("config.json"),
        r#"{ "version": 1, "updates": { "channel": "beta" } }"#,
    )
    .unwrap();
    let store = ConfigStore::open(dir.path());
    assert_eq!(
        store.notice(),
        Some(kivori_desktop::config::ConfigNotice::RecoveredCorrupt)
    );
    assert!(store.file().updates.auto_check);
}

#[test]
fn a_custom_profile_is_written_with_its_apps_and_pin_cycle_flag() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    store.add_custom_profile("Figma.exe", "Figma").unwrap();
    store
        .assign_app(
            &kivori_desktop::config::ProfileRef::Builtin(ProfileId::Code),
            "sublime_text.exe",
        )
        .unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("config.json")).unwrap()).unwrap();
    let custom = &json["customProfiles"][0];
    assert_eq!(custom["id"], "custom-1");
    assert_eq!(custom["name"], "Figma");
    assert_eq!(custom["apps"], serde_json::json!(["figma.exe"]));
    assert_eq!(custom["inPinCycle"], false);
    assert_eq!(custom["bindings"]["buttons"].as_array().unwrap().len(), 3);
    assert_eq!(
        json["profiles"]["code"]["addedApps"],
        serde_json::json!(["sublime_text.exe"])
    );
    assert!(json["profiles"]["code"].get("removedApps").is_none());
}

#[test]
fn an_invalid_save_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    let same_view = store.edited(|file| {
        file.display = DisplaySettings {
            default_view: View::Media,
            secondary_view: SecondaryView::View(View::Media),
            ..DisplaySettings::default()
        };
    });
    assert!(matches!(
        store.save(same_view),
        Err(ConfigError::Invalid(_))
    ));
    assert_eq!(store.file(), &ConfigFile::default());
    assert!(names(dir.path()).is_empty());
}

#[test]
fn a_failed_write_changes_nothing_and_reveals_no_path() {
    let dir = tempfile::tempdir().unwrap();
    // The "directory" is a file, so nothing can be created under it.
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, b"x").unwrap();
    let mut store = ConfigStore::open(&blocker.join("config-dir"));
    let error = store.save(changed(&store)).unwrap_err();
    assert_eq!(error, ConfigError::Io);
    assert!(!error.to_string().contains("blocker"));
    assert_eq!(store.file(), &ConfigFile::default());
    assert_eq!(store.revision(), 0);
}

#[test]
fn a_detached_store_refuses_every_save() {
    let mut store = ConfigStore::detached();
    assert_eq!(store.save(changed(&store)), Err(ConfigError::Unavailable));
    assert_eq!(store.reset(), Err(ConfigError::Unavailable));
}

fn assert_recovered(dir: &Path, original: &[u8], tag: &str) -> ConfigStore {
    let store = ConfigStore::open(dir);
    assert_eq!(store.file(), &ConfigFile::default());
    let backups: Vec<String> = names(dir)
        .into_iter()
        .filter(|n| n.starts_with(&format!("config.{tag}-")))
        .collect();
    assert_eq!(backups.len(), 1, "one backup: {:?}", names(dir));
    assert_eq!(fs::read(dir.join(&backups[0])).unwrap(), original);
    assert!(
        !dir.join("config.json").exists(),
        "nothing is written until the first change"
    );
    store
}

#[test]
fn garbage_oversize_and_unknown_fields_are_backed_up_and_reported() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("not json", b"\x00\xffnot json".to_vec()),
        ("no version", br#"{"display":{}}"#.to_vec()),
        ("string version", br#"{"version":"1"}"#.to_vec()),
        ("unknown field", br#"{"version":1,"theme":"dark"}"#.to_vec()),
        (
            "unknown nested field",
            br#"{"version":1,"buddy":{"reactions":true,"intensity":"low","mood":1}}"#.to_vec(),
        ),
        (
            "bad view",
            br#"{"version":1,"display":{"defaultView":"nope","secondaryView":"cycle"}}"#.to_vec(),
        ),
        (
            "same views",
            br#"{"version":1,"display":{"defaultView":"clock","secondaryView":"clock"}}"#.to_vec(),
        ),
        ("oversize", vec![b' '; 256 * 1024 + 1]),
    ];
    for (name, bytes) in cases {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.json"), &bytes).unwrap();
        let store = assert_recovered(dir.path(), &bytes, "corrupt");
        assert_eq!(
            store.notice(),
            Some(ConfigNotice::RecoveredCorrupt),
            "{name}"
        );
    }
}

#[test]
fn a_newer_version_is_backed_up_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let original = br#"{"version":99,"futureThing":{"a":1}}"#;
    fs::write(dir.path().join("config.json"), original).unwrap();
    let store = assert_recovered(dir.path(), original, "v99");
    assert_eq!(store.notice(), Some(ConfigNotice::RecoveredNewerVersion));
}

#[test]
fn a_stale_temp_file_is_ignored_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    store.save(changed(&store)).unwrap();
    fs::write(dir.path().join("config.json.tmp"), b"half-writ").unwrap();

    let reopened = ConfigStore::open(dir.path());
    assert_eq!(reopened.file(), store.file(), "the real file wins");
    assert_eq!(reopened.notice(), None);
    assert_eq!(names(dir.path()), ["config.json"]);
}

#[test]
fn backups_are_pruned_to_the_newest_five() {
    let dir = tempfile::tempdir().unwrap();
    for secs in 1..=8u32 {
        fs::write(
            dir.path().join(format!("config.corrupt-{secs}.json")),
            b"old",
        )
        .unwrap();
    }
    // Not part of the pruned set.
    fs::write(dir.path().join("config.before-reset.json"), b"keep").unwrap();
    fs::write(dir.path().join("config.v0.bak"), b"keep").unwrap();
    fs::write(dir.path().join("config.json"), b"garbage").unwrap();

    let _ = ConfigStore::open(dir.path());
    let backups: Vec<String> = names(dir.path())
        .into_iter()
        .filter(|n| n.starts_with("config.corrupt-"))
        .collect();
    assert_eq!(backups.len(), 5, "{backups:?}");
    for gone in 1..=4 {
        assert!(
            !backups.contains(&format!("config.corrupt-{gone}.json")),
            "{gone}"
        );
    }
    // The newest four survive, and so does the file just set aside (the newest of all).
    for kept in 5..=8 {
        assert!(
            backups.contains(&format!("config.corrupt-{kept}.json")),
            "{kept}"
        );
    }
    assert!(names(dir.path()).contains(&"config.before-reset.json".to_string()));
    assert!(names(dir.path()).contains(&"config.v0.bak".to_string()));
}

#[test]
fn reset_keeps_the_previous_file_and_writes_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    store.save(changed(&store)).unwrap();
    let before = fs::read(dir.path().join("config.json")).unwrap();

    store.reset().unwrap();
    assert_eq!(store.file(), &ConfigFile::default());
    assert_eq!(store.revision(), 2);
    assert_eq!(
        fs::read(dir.path().join("config.before-reset.json")).unwrap(),
        before
    );
    assert_eq!(ConfigStore::open(dir.path()).file(), &ConfigFile::default());

    // The single slot holds the most recent previous file.
    store
        .save(store.edited(|file| file.buddy.intensity = Intensity::Low))
        .unwrap();
    store.reset().unwrap();
    assert_ne!(
        fs::read(dir.path().join("config.before-reset.json")).unwrap(),
        before
    );
}

#[test]
fn reset_with_no_file_just_writes_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    store.reset().unwrap();
    assert_eq!(names(dir.path()), ["config.json"]);
}

#[test]
fn reset_clears_a_recovery_notice() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config.json"), b"garbage").unwrap();
    let mut store = ConfigStore::open(dir.path());
    assert_eq!(store.notice(), Some(ConfigNotice::RecoveredCorrupt));
    store.reset().unwrap();
    assert_eq!(store.notice(), None);
}

fn standup() -> MacroSpec {
    MacroSpec {
        id: "standup".into(),
        name: "Standup".into(),
        steps: vec![
            StepSpec::Action {
                action: ActionSpec::SystemMute,
            },
            StepSpec::Delay { ms: 500 },
            StepSpec::Action {
                action: ActionSpec::Shortcut {
                    keys: "ctrl+m".into(),
                },
            },
        ],
    }
}

fn bind_to_standup(store: &mut ConfigStore) {
    let next = store.edited(|file| {
        file.profiles.insert(
            ProfileId::Zoom,
            ProfileOverride {
                press: Some(SlotSpec {
                    action: Some(ActionSpec::Macro {
                        id: "standup".into(),
                    }),
                    label: None,
                }),
                ..ProfileOverride::default()
            },
        );
    });
    store.save(next).unwrap();
}

#[test]
fn a_macro_is_saved_canonical_replaced_by_id_and_survives_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    store.save_macro(standup()).unwrap();
    assert_eq!(store.file().macros.len(), 1);
    let mut renamed = standup();
    renamed.name = "Daily".into();
    store.save_macro(renamed).unwrap();
    assert_eq!(store.file().macros.len(), 1, "same id replaces");
    let reopened = ConfigStore::open(dir.path());
    assert_eq!(reopened.file().macros[0].name, "Daily");
    assert_eq!(
        reopened.file().macros[0].steps[2],
        StepSpec::Action {
            action: ActionSpec::Shortcut {
                keys: "Ctrl+M".into()
            }
        },
        "shortcuts are stored canonical"
    );
    assert!(reopened.resolved().macros.contains_key("standup"));
}

#[test]
fn delete_macro_is_refused_while_bound_and_allowed_once_unbound() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    store.save_macro(standup()).unwrap();
    bind_to_standup(&mut store);
    let before = store.file().clone();
    assert!(matches!(
        store.delete_macro("standup"),
        Err(ConfigError::Invalid(_))
    ));
    assert_eq!(store.file(), &before, "nothing changed");
    assert!(ConfigStore::open(dir.path())
        .resolved()
        .macros
        .contains_key("standup"));

    let unbound = store.edited(|file| file.profiles.clear());
    store.save(unbound).unwrap();
    store.delete_macro("standup").unwrap();
    assert!(store.file().macros.is_empty());
    assert!(matches!(
        store.delete_macro("standup"),
        Err(ConfigError::Invalid(_))
    ));
}

#[test]
fn an_invalid_macro_is_rejected_on_save_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    let play = || StepSpec::Action {
        action: ActionSpec::PlayPause,
    };
    let with = |steps: Vec<StepSpec>| MacroSpec { steps, ..standup() };
    for bad in [
        with(vec![play(); 9]),
        with(vec![play(), StepSpec::Delay { ms: 49 }]),
        with(vec![
            play(),
            StepSpec::Action {
                action: ActionSpec::Macro {
                    id: "standup".into(),
                },
            },
        ]),
    ] {
        assert!(matches!(
            store.save_macro(bad),
            Err(ConfigError::Invalid(_))
        ));
    }
    assert_eq!(store.file(), &ConfigFile::default());
    assert!(names(dir.path()).is_empty());
}

#[test]
fn a_binding_to_an_unknown_macro_is_refused_on_save_and_corrupt_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    let next = store.edited(|file| {
        file.profiles.insert(
            ProfileId::Zoom,
            ProfileOverride {
                press: Some(SlotSpec {
                    action: Some(ActionSpec::Macro { id: "ghost".into() }),
                    label: None,
                }),
                ..ProfileOverride::default()
            },
        );
    });
    assert!(matches!(store.save(next), Err(ConfigError::Invalid(_))));

    let bytes =
        br#"{"version":1,"profiles":{"zoom":{"press":{"action":{"kind":"macro","id":"ghost"}}}}}"#;
    fs::write(dir.path().join("config.json"), bytes).unwrap();
    let store = assert_recovered(dir.path(), bytes, "corrupt");
    assert_eq!(store.notice(), Some(ConfigNotice::RecoveredCorrupt));
}

#[test]
fn onboarding_round_trips_and_a_fresh_install_has_not_completed_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    assert!(!store.file().onboarding.completed);
    store
        .save(store.edited(|file| file.onboarding.completed = true))
        .unwrap();
    assert!(ConfigStore::open(dir.path()).file().onboarding.completed);
    store
        .save(store.edited(|file| file.onboarding.completed = false))
        .unwrap();
    let reopened = ConfigStore::open(dir.path());
    assert!(!reopened.file().onboarding.completed);
    assert_eq!(reopened.notice(), None);
}

#[test]
fn an_existing_file_without_the_field_counts_as_already_set_up() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("config.json"),
        br#"{"version":1,"buddy":{"reactions":false,"intensity":"low"}}"#,
    )
    .unwrap();
    let store = ConfigStore::open(dir.path());
    assert_eq!(store.notice(), None);
    assert!(store.file().onboarding.completed);
    assert!(!store.file().buddy.reactions);
}

#[test]
fn a_file_that_names_the_field_is_taken_at_its_word() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("config.json"),
        br#"{"version":1,"onboarding":{}}"#,
    )
    .unwrap();
    assert!(!ConfigStore::open(dir.path()).file().onboarding.completed);
}

#[test]
fn reset_keeps_onboarding_completed() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    store
        .save(store.edited(|file| {
            file.onboarding.completed = true;
            file.buddy.reactions = false;
        }))
        .unwrap();
    store.reset().unwrap();
    assert!(store.file().onboarding.completed);
    assert!(store.file().buddy.reactions);
    assert!(ConfigStore::open(dir.path()).file().onboarding.completed);
}

/// A file written before the brightness setting existed stays version 1 and loads at the top step
/// (the always-on backlight it had), so no migration and no backup is needed.
#[test]
fn a_file_without_brightness_loads_at_full_and_the_setting_survives_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("config.json"),
        br#"{"version":1,"display":{"defaultView":"clock","secondaryView":"cycle"}}"#,
    )
    .unwrap();
    let mut store = ConfigStore::open(dir.path());
    assert_eq!(store.notice(), None);
    assert_eq!(store.file().version, 1);
    assert_eq!(store.file().display.brightness, 5);
    assert_eq!(names(dir.path()), ["config.json"]);

    let next = store.edited(|file| file.display.brightness = 1);
    store.save(next).unwrap();
    assert_eq!(ConfigStore::open(dir.path()).file().display.brightness, 1);
}

#[test]
fn a_brightness_step_the_device_does_not_have_is_refused_and_corrupt_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    for step in [0, 6] {
        let bad = store.edited(|file| file.display.brightness = step);
        assert!(matches!(store.save(bad), Err(ConfigError::Invalid(_))));
    }
    assert_eq!(store.file(), &ConfigFile::default());

    let original =
        br#"{"version":1,"display":{"defaultView":"buddy","secondaryView":"system","brightness":9}}"#;
    fs::write(dir.path().join("config.json"), original).unwrap();
    let reopened = ConfigStore::open(dir.path());
    assert_eq!(reopened.notice(), Some(ConfigNotice::RecoveredCorrupt));
    assert_eq!(reopened.file(), &ConfigFile::default());
}

mod profiles {
    use kivori_desktop::config::{ProfileRef, MAX_CUSTOM_PROFILES};

    use super::*;

    fn custom(id: &str) -> ProfileRef {
        ProfileRef::Custom(id.into())
    }

    fn names_of(store: &ConfigStore) -> Vec<String> {
        store
            .resolved()
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect()
    }

    #[test]
    fn a_file_without_custom_profiles_loads_with_the_default_and_stays_version_1() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.json"),
            br#"{ "version": 1, "profiles": {} }"#,
        )
        .unwrap();
        let store = ConfigStore::open(dir.path());
        assert_eq!(store.notice(), None);
        assert!(store.file().custom_profiles.is_empty());
        assert_eq!(store.file().version, 1);
    }

    #[test]
    fn a_custom_profile_copies_general_once_and_survives_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::open(dir.path());
        // General's Press is changed before the copy, then again after it.
        let press = |keys: &str| SlotSpec {
            action: Some(ActionSpec::Shortcut { keys: keys.into() }),
            label: Some("Mine".into()),
        };
        store
            .save(store.edited(|f| {
                f.profiles.entry(ProfileId::General).or_default().press = Some(press("Ctrl+K"));
            }))
            .unwrap();
        store.add_custom_profile("Figma.exe", "Figma").unwrap();
        store
            .save(store.edited(|f| {
                f.profiles.entry(ProfileId::General).or_default().press = Some(press("Ctrl+J"));
            }))
            .unwrap();
        let reopened = ConfigStore::open(dir.path());
        assert_eq!(reopened.file(), store.file());
        let resolved = reopened.resolved();
        let figma = resolved.profiles.last().unwrap();
        assert_eq!(figma.name, "Figma");
        assert_eq!(figma.ids, ["figma.exe"]);
        assert!(!figma.in_pin_cycle);
        let bound = |p: &kivori_desktop::desk::profile::Profile| p.bindings.press.action.clone();
        assert_ne!(
            bound(figma),
            bound(&resolved.profiles[0]),
            "later edits do not flow in"
        );
        assert_eq!(figma.bindings.press.label.as_deref(), Some("Mine"));
    }

    #[test]
    fn reset_to_general_recopies_a_custom_profile_and_a_built_in_gets_its_apps_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::open(dir.path());
        store.add_custom_profile("figma.exe", "").unwrap();
        let id = custom("custom-1");
        store
            .save(store.edited(|f| {
                f.custom_profiles[0].bindings.press = Some(SlotSpec {
                    action: Some(ActionSpec::NextTrack),
                    label: None,
                });
            }))
            .unwrap();
        store.reset_profile(&id).unwrap();
        assert_eq!(
            store.resolved().profiles.last().unwrap().bindings,
            store.resolved().profiles[0].bindings
        );
        assert_eq!(
            store.file().custom_profiles[0].apps,
            ["figma.exe"],
            "apps stay"
        );

        store
            .assign_app(&ProfileRef::Builtin(ProfileId::Code), "sublime_text.exe")
            .unwrap();
        store
            .assign_app(&ProfileRef::Builtin(ProfileId::General), "code.exe")
            .unwrap();
        let code = |s: &ConfigStore| s.resolved().profiles[2].ids.clone();
        assert!(code(&store).contains(&"sublime_text.exe".to_string()));
        assert!(!code(&store).contains(&"code.exe".to_string()));
        store
            .reset_profile(&ProfileRef::Builtin(ProfileId::Code))
            .unwrap();
        assert!(code(&store).contains(&"code.exe".to_string()));
        assert!(store.file().profiles.is_empty());
    }

    #[test]
    fn rename_delete_and_pin_cycle_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::open(dir.path());
        store.add_custom_profile("figma.exe", "Figma").unwrap();
        store
            .rename_profile(&custom("custom-1"), "  Design  ")
            .unwrap();
        store.set_pin_cycle(&custom("custom-1"), true).unwrap();
        let reopened = ConfigStore::open(dir.path());
        assert_eq!(
            names_of(&reopened).last().map(String::as_str),
            Some("Design")
        );
        assert!(reopened.resolved().profiles.last().unwrap().in_pin_cycle);
        let revision = store.revision();
        assert!(store.rename_profile(&custom("custom-1"), " ").is_err());
        assert!(store
            .rename_profile(&custom("custom-1"), &"x".repeat(33))
            .is_err());
        assert_eq!(store.revision(), revision, "a refused change saves nothing");
        store.delete_profile(&custom("custom-1")).unwrap();
        assert!(store.file().custom_profiles.is_empty());
        assert!(store.delete_profile(&custom("custom-1")).is_err());
    }

    #[test]
    fn at_most_sixteen_custom_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::open(dir.path());
        for n in 0..MAX_CUSTOM_PROFILES {
            store
                .add_custom_profile(&format!("app{n}.exe"), "")
                .unwrap();
        }
        assert!(matches!(
            store.add_custom_profile("one-more.exe", ""),
            Err(ConfigError::Invalid(_))
        ));
        assert_eq!(store.file().custom_profiles.len(), MAX_CUSTOM_PROFILES);
    }

    #[test]
    fn a_file_with_an_app_in_two_profiles_is_set_aside_like_any_damaged_file() {
        let dir = tempfile::tempdir().unwrap();
        let both = r#"{ "version": 1, "customProfiles": [
            { "id": "custom-1", "name": "A", "apps": ["figma.exe"] },
            { "id": "custom-2", "name": "B", "apps": ["FIGMA.exe"] } ] }"#;
        fs::write(dir.path().join("config.json"), both).unwrap();
        let store = ConfigStore::open(dir.path());
        assert_eq!(store.notice(), Some(ConfigNotice::RecoveredCorrupt));
        assert!(store.file().custom_profiles.is_empty());
        assert!(names(dir.path())
            .iter()
            .any(|n| n.starts_with("config.corrupt-")));
    }

    #[test]
    fn a_newer_file_with_custom_profiles_is_still_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let newer = r#"{ "version": 2, "customProfiles": [{ "id": "custom-1", "name": "A" }] }"#;
        fs::write(dir.path().join("config.json"), newer).unwrap();
        let store = ConfigStore::open(dir.path());
        assert_eq!(store.notice(), Some(ConfigNotice::RecoveredNewerVersion));
        assert!(store.file().custom_profiles.is_empty());
    }

    #[test]
    fn reset_everything_removes_custom_profiles_too() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::open(dir.path());
        store.add_custom_profile("figma.exe", "").unwrap();
        store.reset().unwrap();
        assert!(store.file().custom_profiles.is_empty());
        assert_eq!(store.resolved().profiles.len(), ProfileId::ALL.len());
    }

    #[test]
    fn a_macro_bound_in_a_custom_profile_cannot_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::open(dir.path());
        store
            .save_macro(MacroSpec {
                id: "hello".into(),
                name: "Hello".into(),
                steps: vec![StepSpec::Action {
                    action: ActionSpec::PlayPause,
                }],
            })
            .unwrap();
        store.add_custom_profile("figma.exe", "").unwrap();
        store
            .save(store.edited(|f| {
                f.custom_profiles[0].bindings.hold = Some(SlotSpec {
                    action: Some(ActionSpec::Macro { id: "hello".into() }),
                    label: None,
                });
            }))
            .unwrap();
        assert!(store.delete_macro("hello").is_err());
        let _ = ProfileOverride::default();
    }

    #[test]
    fn the_config_dto_marks_custom_profiles_and_their_pin_cycle_flag() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::open(dir.path());
        store.add_custom_profile("figma.exe", "Figma").unwrap();
        store.set_pin_cycle(&custom("custom-1"), true).unwrap();
        let dto = kivori_desktop::ipc::dto::config_dto(&store);
        assert_eq!(dto.profiles.len(), 7);
        assert!(dto.profiles[..6]
            .iter()
            .all(|p| !p.custom && p.in_pin_cycle));
        let mine = &dto.profiles[6];
        assert_eq!(
            (mine.id.as_str(), mine.custom, mine.in_pin_cycle),
            ("custom-1", true, true)
        );
        assert_eq!(mine.apps, ["figma.exe"]);
        assert!(!mine.press.overridden);
    }
}
