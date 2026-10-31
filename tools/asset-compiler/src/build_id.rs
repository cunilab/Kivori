//! Which build is this? The git commit a binary was built from and whether the working tree had
//! uncommitted changes in the files that make it up (host, std, build scripts only).
//!
//! The firmware and the desktop app both stamp this into themselves at compile time, so a developer
//! can tell at a glance when a unit runs a build that differs from the repository. The id depends
//! only on the commit and the dirty flag, never on the time or the machine, so two builds of the
//! same clean commit carry the same id. Without git (a source tarball, a shallow export, a foreign
//! parent repository) the commit is simply unknown.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Hex digits of the commit that are kept: `git rev-parse --short=12`.
pub const COMMIT_DIGITS: usize = 12;

/// The identity of one build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildId {
    /// Twelve lowercase hex digits, or `None` when git could not say.
    pub commit: Option<String>,
    /// Uncommitted changes (tracked edits or new files) under the build's paths. Always `false`
    /// when the commit is unknown.
    pub dirty: bool,
}

impl BuildId {
    /// The id of a build that cannot name its source.
    pub const UNKNOWN: BuildId = BuildId {
        commit: None,
        dirty: false,
    };
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        // A build must never take the index lock away from the developer's own git.
        .arg("--no-optional-locks")
        .args(args)
        // Locale and pager must not change what is parsed; a git hook's repository variables must
        // not point it at another repository.
        .env("LC_ALL", "C")
        .env("GIT_PAGER", "cat")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Whether `repo` is itself the top of a git work tree. A source tree unpacked inside some other
/// repository would otherwise report that repository's commit.
fn is_toplevel(repo: &Path) -> bool {
    let (Some(top), Ok(here)) = (
        git(repo, &["rev-parse", "--show-toplevel"]),
        repo.canonicalize(),
    ) else {
        return false;
    };
    Path::new(&top).canonicalize().is_ok_and(|top| top == here)
}

/// Reads the build id of the repository rooted at `repo`, looking for uncommitted changes only
/// under `paths` (relative to `repo`; they need not all exist). Never fails: every problem is
/// `BuildId::UNKNOWN`.
#[must_use]
pub fn detect(repo: &Path, paths: &[&str]) -> BuildId {
    if !is_toplevel(repo) {
        return BuildId::UNKNOWN;
    }
    let Some(head) = git(
        repo,
        &["rev-parse", &format!("--short={COMMIT_DIGITS}"), "HEAD"],
    ) else {
        return BuildId::UNKNOWN;
    };
    let head = head.get(..COMMIT_DIGITS).unwrap_or_default();
    if head.len() != COMMIT_DIGITS || !head.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return BuildId::UNKNOWN;
    }
    let mut args = vec!["status", "--porcelain", "--untracked-files=all", "--"];
    args.extend_from_slice(paths);
    // A status that cannot be read is not a clean tree: say dirty rather than claim a clean build.
    let dirty = git(repo, &args).is_none_or(|status| !status.is_empty());
    BuildId {
        commit: Some(head.to_string()),
        dirty,
    }
}

/// The files cargo should watch so the id is refreshed: the build's own paths that exist, plus the
/// current commit pointer (`HEAD` and the branch ref it names).
#[must_use]
pub fn watch_paths(repo: &Path, paths: &[&str]) -> Vec<PathBuf> {
    let mut watch: Vec<PathBuf> = paths
        .iter()
        .map(|p| repo.join(p))
        .filter(|p| p.exists())
        .collect();
    let mut pointers = vec!["HEAD".to_string()];
    if let Some(branch) = git(repo, &["symbolic-ref", "-q", "HEAD"]) {
        pointers.push(branch);
    }
    for pointer in pointers {
        if let Some(path) = git(repo, &["rev-parse", "--git-path", &pointer]) {
            let path = PathBuf::from(path);
            watch.push(if path.is_absolute() {
                path
            } else {
                repo.join(path)
            });
        }
    }
    watch
}

/// Prints the cargo directives a build script needs: `<PREFIX>_COMMIT` (12 hex digits or
/// `unknown`), `<PREFIX>_DIRTY` (`1` or `0`) as compile-time environment, and the watch list.
pub fn emit(prefix: &str, repo: &Path, paths: &[&str]) {
    let id = detect(repo, paths);
    println!(
        "cargo:rustc-env={prefix}_COMMIT={}",
        id.commit.as_deref().unwrap_or("unknown")
    );
    println!("cargo:rustc-env={prefix}_DIRTY={}", u8::from(id.dirty));
    for path in watch_paths(repo, paths) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    /// A repository with one commit holding `fw/a.rs` and `other/b.txt`.
    fn repo(name: &str) -> Scratch {
        let s = Scratch::new(name);
        fs::create_dir_all(s.0.join("fw")).unwrap();
        fs::create_dir_all(s.0.join("other")).unwrap();
        fs::write(s.0.join("fw/a.rs"), "fn a() {}\n").unwrap();
        fs::write(s.0.join("other/b.txt"), "b\n").unwrap();
        run(&s.0, &["init", "-q"]);
        run(&s.0, &["add", "."]);
        run(&s.0, &["commit", "-q", "-m", "one"]);
        s
    }

    fn head(dir: &Path) -> String {
        git(dir, &["rev-parse", "HEAD"]).unwrap()[..COMMIT_DIGITS].to_string()
    }

    #[test]
    fn a_clean_tree_reports_its_commit_and_is_not_dirty() {
        let s = repo("clean");
        let id = detect(&s.0, &["fw"]);
        assert_eq!(id.commit.as_deref(), Some(head(&s.0).as_str()));
        assert!(!id.dirty);
        // The same clean commit gives the same id every time.
        assert_eq!(detect(&s.0, &["fw"]), id);
    }

    #[test]
    fn an_edit_or_a_new_file_under_the_build_paths_is_dirty() {
        let s = repo("dirty");
        fs::write(s.0.join("fw/a.rs"), "fn a() { 1; }\n").unwrap();
        assert!(detect(&s.0, &["fw"]).dirty);
        run(&s.0, &["checkout", "--", "fw/a.rs"]);
        assert!(!detect(&s.0, &["fw"]).dirty);
        fs::write(s.0.join("fw/new.rs"), "").unwrap();
        assert!(detect(&s.0, &["fw"]).dirty, "an untracked file is a change");
    }

    #[test]
    fn changes_outside_the_build_paths_do_not_count() {
        let s = repo("outside");
        fs::write(s.0.join("other/b.txt"), "edited\n").unwrap();
        fs::write(s.0.join("other/untracked"), "").unwrap();
        assert!(!detect(&s.0, &["fw", "missing-path"]).dirty);
        assert!(detect(&s.0, &["other"]).dirty);
    }

    #[test]
    fn without_git_the_commit_is_unknown() {
        let s = Scratch::new("nogit");
        fs::write(s.0.join("a.rs"), "").unwrap();
        assert_eq!(detect(&s.0, &["."]), BuildId::UNKNOWN);
        assert_eq!(detect(&s.0.join("missing"), &["."]), BuildId::UNKNOWN);
    }

    #[test]
    fn a_tree_unpacked_inside_another_repository_is_unknown() {
        let outer = repo("nested");
        let inner = outer.0.join("unpacked");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join("a.rs"), "").unwrap();
        assert_eq!(detect(&inner, &["."]), BuildId::UNKNOWN);
    }

    #[test]
    fn a_repository_without_commits_is_unknown() {
        let s = Scratch::new("empty");
        run(&s.0, &["init", "-q"]);
        assert_eq!(detect(&s.0, &["."]), BuildId::UNKNOWN);
    }

    #[test]
    fn watch_paths_name_existing_paths_and_the_head_pointer() {
        let s = repo("watch");
        let watch = watch_paths(&s.0, &["fw", "nope"]);
        assert!(watch.contains(&s.0.join("fw")));
        assert!(!watch.contains(&s.0.join("nope")));
        assert!(watch.iter().any(|p| p.ends_with("HEAD")));
    }
}
