//! The firmware source id (`kivori_asset_compiler::build_id`): a content hash of the committed
//! input files, so identical inputs give an identical id whatever the commit.

use kivori_asset_compiler::build_id::{detect, read_inputs, watch_paths, BuildId, ID_DIGITS};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("kivori-build-id-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env_remove("GIT_DIR")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

fn commit_all(dir: &Path, message: &str) {
    run(dir, &["add", "-A"]);
    run(dir, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// A repository with one commit holding the firmware input `fw/a.rs`, a manifest `fw/Cargo.toml`
/// with a version, and `other/b.txt`, which is not an input.
fn repo(name: &str) -> Scratch {
    let s = Scratch::new(name);
    fs::create_dir_all(s.0.join("fw")).unwrap();
    fs::create_dir_all(s.0.join("other")).unwrap();
    fs::write(s.0.join("fw/a.rs"), "fn a() {}\n").unwrap();
    fs::write(s.0.join("fw/Cargo.toml"), "version = \"0.1.0\"\n").unwrap();
    fs::write(s.0.join("Cargo.lock"), "version = 3\n").unwrap();
    fs::write(s.0.join("other/b.txt"), "b\n").unwrap();
    run(&s.0, &["init", "-q"]);
    commit_all(&s.0, "one");
    s
}

fn inputs() -> Vec<String> {
    read_inputs("# a comment\nfw\n\nCargo.lock\nmissing-path\n")
}

#[test]
fn a_clean_tree_has_a_twelve_digit_id_and_is_not_dirty() {
    let s = repo("clean");
    let id = detect(&s.0, &inputs());
    let source = id.source.clone().expect("an id");
    assert_eq!(source.len(), ID_DIGITS);
    assert!(source
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert!(!id.dirty);
    assert_eq!(detect(&s.0, &inputs()), id, "stable across calls");
}

#[test]
fn identical_inputs_give_the_same_id_across_different_commits() {
    let s = repo("same");
    let before = detect(&s.0, &inputs());
    // New commits that touch nothing the firmware is made of (an outside file, an empty commit).
    fs::write(s.0.join("other/b.txt"), "edited\n").unwrap();
    commit_all(&s.0, "outside");
    commit_all(&s.0, "empty");
    assert_eq!(detect(&s.0, &inputs()), before);
    // Same content in a fresh repository with an unrelated history: same id.
    let twin = Scratch::new("twin");
    fs::create_dir_all(twin.0.join("fw")).unwrap();
    fs::write(twin.0.join("fw/a.rs"), "fn a() {}\n").unwrap();
    fs::write(twin.0.join("fw/Cargo.toml"), "version = \"9.9.9\"\n").unwrap();
    fs::write(twin.0.join("Cargo.lock"), "version = 4\n").unwrap();
    run(&twin.0, &["init", "-q"]);
    commit_all(&twin.0, "other history");
    assert_eq!(detect(&twin.0, &inputs()), before);
}

#[test]
fn a_committed_change_under_the_inputs_changes_the_id() {
    let s = repo("change");
    let before = detect(&s.0, &inputs()).source;
    fs::write(s.0.join("fw/a.rs"), "fn a() { 1; }\n").unwrap();
    commit_all(&s.0, "edit");
    let after = detect(&s.0, &inputs());
    assert_ne!(after.source, before);
    assert!(!after.dirty);
    // A new file under an input folder counts too.
    fs::write(s.0.join("fw/b.rs"), "").unwrap();
    commit_all(&s.0, "add");
    assert_ne!(detect(&s.0, &inputs()).source, after.source);
}

#[test]
fn a_manifest_or_lockfile_change_does_not_change_the_id() {
    let s = repo("manifest");
    let before = detect(&s.0, &inputs());
    fs::write(s.0.join("fw/Cargo.toml"), "version = \"0.2.0\"\n").unwrap();
    fs::write(s.0.join("Cargo.lock"), "version = 4\n").unwrap();
    // Not dirty while only bookkeeping is uncommitted...
    assert_eq!(detect(&s.0, &inputs()), before);
    // ...and the same id once it is committed.
    commit_all(&s.0, "bump");
    assert_eq!(detect(&s.0, &inputs()), before);
}

#[test]
fn uncommitted_edits_and_new_files_under_the_inputs_are_dirty() {
    let s = repo("dirty");
    let id = detect(&s.0, &inputs()).source;
    fs::write(s.0.join("fw/a.rs"), "fn a() { 1; }\n").unwrap();
    let edited = detect(&s.0, &inputs());
    assert!(edited.dirty);
    assert_eq!(edited.source, id, "the id names the committed content");
    run(&s.0, &["checkout", "--", "fw/a.rs"]);
    assert!(!detect(&s.0, &inputs()).dirty);
    fs::write(s.0.join("fw/new.rs"), "").unwrap();
    assert!(
        detect(&s.0, &inputs()).dirty,
        "an untracked file is a change"
    );
}

#[test]
fn changes_outside_the_inputs_do_not_count() {
    let s = repo("outside");
    fs::write(s.0.join("other/b.txt"), "edited\n").unwrap();
    fs::write(s.0.join("other/untracked"), "").unwrap();
    assert!(!detect(&s.0, &inputs()).dirty);
}

#[test]
fn without_git_or_commits_or_inputs_the_id_is_unknown() {
    let s = Scratch::new("nogit");
    fs::create_dir_all(s.0.join("fw")).unwrap();
    fs::write(s.0.join("fw/a.rs"), "").unwrap();
    assert_eq!(detect(&s.0, &inputs()), BuildId::UNKNOWN);
    assert_eq!(detect(&s.0.join("missing"), &inputs()), BuildId::UNKNOWN);

    // No input list means nothing to hash, never "everything".
    assert_eq!(detect(&repo("noinputlist").0, &[]), BuildId::UNKNOWN);

    let empty = Scratch::new("empty");
    run(&empty.0, &["init", "-q"]);
    assert_eq!(detect(&empty.0, &inputs()), BuildId::UNKNOWN);

    // A repository whose commits hold none of the inputs cannot name a source.
    let other = repo("noinputs");
    assert_eq!(
        detect(&other.0, &read_inputs("nothing-here\n")),
        BuildId::UNKNOWN
    );
}

#[test]
fn a_tree_unpacked_inside_another_repository_is_unknown() {
    let outer = repo("nested");
    let inner = outer.0.join("unpacked");
    fs::create_dir_all(inner.join("fw")).unwrap();
    fs::write(inner.join("fw/a.rs"), "").unwrap();
    assert_eq!(detect(&inner, &inputs()), BuildId::UNKNOWN);
}

#[test]
fn watch_paths_name_existing_input_files_and_the_head_pointer() {
    let s = repo("watch");
    fs::write(s.0.join("fw/untracked.rs"), "").unwrap();
    let watch = watch_paths(&s.0, &inputs());
    assert!(watch.contains(&s.0.join("fw/a.rs")));
    assert!(watch.contains(&s.0.join("fw/untracked.rs")));
    assert!(!watch.contains(&s.0.join("other/b.txt")));
    assert!(watch.iter().any(|p| p.ends_with("HEAD")));
}

#[test]
fn read_inputs_skips_comments_and_blank_lines() {
    assert_eq!(read_inputs("# x\n  a/b  \n\nc\n"), vec!["a/b", "c"]);
}
