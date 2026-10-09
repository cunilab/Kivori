//! The config file: first run, round trip, recovery of a bad file, downgrade and reset.

use std::fs;
use std::path::Path;

use kivori_desktop::config::{
    ConfigError, ConfigFile, ConfigNotice, ConfigStore, DisplaySettings, Intensity, SecondaryView,
    View,
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
            "macros": [],
            "display": { "defaultView": "clock", "secondaryView": "cycle" },
            "buddy": { "reactions": false, "intensity": "high" }
        })
    );
}

#[test]
fn an_invalid_save_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(dir.path());
    let same_view = store.edited(|file| {
        file.display = DisplaySettings {
            default_view: View::Media,
            secondary_view: SecondaryView::View(View::Media),
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
