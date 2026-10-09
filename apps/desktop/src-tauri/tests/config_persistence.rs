//! Bindings survive a restart: rebind, drop the store, reopen, resolve, and the binding is there.

use kivori_desktop::config::resolve::{resolve, Control};
use kivori_desktop::config::{
    ActionSpec, ConfigNotice, ConfigStore, ProfileId, RotateSpec, SlotSpec,
};
use kivori_desktop::desk::profile::builtins_for;
use kivori_desktop::desk::{Action, Slot};

#[test]
fn a_rebind_survives_dropping_the_store_and_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    let next = store.edited(|file| {
        let over = file.profiles.entry(ProfileId::Zoom).or_default();
        Control::ButtonHold(0).set(
            over,
            Some(SlotSpec {
                action: Some(ActionSpec::Launch {
                    target: "Spotify".into(),
                }),
                label: Some("Music".into()),
            }),
        );
        Control::Press.set(
            over,
            Some(SlotSpec {
                action: None,
                label: None,
            }),
        );
        over.rotate = Some(RotateSpec::Shortcuts {
            cw: "Ctrl+Right".into(),
            ccw: "Ctrl+Left".into(),
            label: "Seek".into(),
        });
    });
    store.save(next).unwrap();
    drop(store);

    let reopened = ConfigStore::open(dir.path());
    assert_eq!(reopened.notice(), None);
    for mac in [false, true] {
        let resolved = resolve(builtins_for(mac), reopened.file()).unwrap();
        let zoom = &resolved.profiles[4];
        assert_eq!(
            zoom.bindings.buttons[0].hold,
            Slot::named(Action::Launch("Spotify".into()), "Music")
        );
        assert_eq!(
            zoom.bindings.press,
            Slot::default(),
            "an explicit unbind persists"
        );
        assert_eq!(zoom.rotate.label(), "Seek");
        assert_eq!(
            zoom.bindings.buttons[0].press,
            builtins_for(mac)[4].bindings.buttons[0].press,
            "untouched controls stay built-in"
        );
    }
    assert_eq!(reopened.resolved().profiles[4].rotate.label(), "Seek");
}

#[test]
fn a_file_whose_override_does_not_resolve_is_set_aside_as_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.json"),
        r#"{"version":1,"profiles":{"code":{"press":{"action":{"kind":"shortcut","keys":"Ctrl+"}}}}}"#,
    )
    .unwrap();
    let store = ConfigStore::open(dir.path());
    assert_eq!(store.notice(), Some(ConfigNotice::RecoveredCorrupt));
    assert!(store.file().profiles.is_empty());
}

#[test]
fn an_invalid_rebind_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    let bad = store.edited(|file| {
        file.profiles.entry(ProfileId::Code).or_default().press = Some(SlotSpec {
            action: Some(ActionSpec::PlayPause),
            label: Some("x".repeat(33)),
        });
    });
    assert!(store.save(bad).is_err());
    assert!(store.file().profiles.is_empty());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
