//! Which firmware source is this? A content hash of the committed firmware input files, and whether
//! the working tree has uncommitted changes in them (host, std, build scripts only).
//!
//! The firmware and the desktop app both stamp this into themselves at compile time, so a developer
//! can tell at a glance when a unit runs firmware that differs from the repository. The id is a
//! function of the committed content of the input paths only: never the commit, the time or the
//! machine. Identical inputs give an identical id (and so identical firmware bytes) in a shallow CI
//! clone, after a squash merge, or on another branch. Cargo manifests and lockfiles are left out,
//! so a workspace version bump (release bookkeeping) does not change it. Without git (a source
//! tarball, a tree unpacked inside another repository, a repository with no commit) the id is
//! simply unknown.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Hex digits of the source id: the first 12 of a git object id (48 bits, 6 bytes on the wire).
pub const ID_DIGITS: usize = 12;

/// The identity of one firmware source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildId {
    /// Twelve lowercase hex digits, or `None` when git could not say.
    pub source: Option<String>,
    /// Uncommitted changes (tracked edits or new files) under the input paths. Always `false`
    /// when the id is unknown.
    pub dirty: bool,
}

impl BuildId {
    /// The id of a build that cannot name its source.
    pub const UNKNOWN: BuildId = BuildId {
        source: None,
        dirty: false,
    };
}

/// Cargo manifests and lockfiles (`Cargo.*`) carry the workspace version bookkeeping and do not
/// take part in the id. Matched by file name prefix.
fn is_manifest(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with("Cargo."))
}

fn command(repo: &Path, args: &[&str]) -> Command {
    let mut git = Command::new("git");
    git.arg("-C")
        .arg(repo)
        // A build must never take the index lock away from the developer's own git, and a path
        // is a path, never a pattern.
        .args(["--no-optional-locks", "--literal-pathspecs"])
        .args(args)
        // Locale and pager must not change what is parsed; a git hook's repository variables must
        // not point it at another repository.
        .env("LC_ALL", "C")
        .env("GIT_PAGER", "cat")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    git
}

fn git_raw(repo: &Path, args: &[&str], stdin: Option<&[u8]>) -> Option<Vec<u8>> {
    let mut cmd = command(repo, args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = cmd.spawn().ok()?;
    if let Some(bytes) = stdin {
        child.stdin.take()?.write_all(bytes).ok()?;
    }
    let out = child.wait_with_output().ok()?;
    out.status.success().then_some(out.stdout)
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    git_raw(repo, args, None).map(|out| String::from_utf8_lossy(&out).trim().to_string())
}

/// Whether `repo` is itself the top of a git work tree. A source tree unpacked inside some other
/// repository would otherwise report that repository's content.
fn is_toplevel(repo: &Path) -> bool {
    let (Some(top), Ok(here)) = (
        git(repo, &["rev-parse", "--show-toplevel"]),
        repo.canonicalize(),
    ) else {
        return false;
    };
    Path::new(&top).canonicalize().is_ok_and(|top| top == here)
}

/// Reads an input list: one repository-relative path per line, blank lines and `#` comments skipped.
#[must_use]
pub fn read_inputs(list: &str) -> Vec<String> {
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Reads the source id of the repository rooted at `repo` from the committed content of `inputs`
/// (relative to `repo`; they need not all exist), and looks for uncommitted changes under them.
/// Never fails: every problem is `BuildId::UNKNOWN`.
#[must_use]
pub fn detect(repo: &Path, inputs: &[String]) -> BuildId {
    // No inputs would hash the whole tree: that is no firmware source at all.
    if inputs.is_empty() || !is_toplevel(repo) {
        return BuildId::UNKNOWN;
    }
    let paths: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let mut args = vec!["ls-tree", "-r", "-z", "HEAD", "--"];
    args.extend_from_slice(&paths);
    let Some(tree) = git_raw(repo, &args, None) else {
        return BuildId::UNKNOWN;
    };
    // `<mode> <type> <blob id>\t<path>` per entry, already in path order: the blob ids are content
    // hashes, so the listing is a function of the committed content of the input files.
    let mut listing = String::new();
    for entry in String::from_utf8_lossy(&tree).split('\0') {
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        let Some(blob) = meta.split(' ').nth(2) else {
            continue;
        };
        if !is_manifest(path) {
            listing.push_str(&format!("{path}\t{blob}\n"));
        }
    }
    if listing.is_empty() {
        return BuildId::UNKNOWN;
    }
    let Some(hash) = git_raw(repo, &["hash-object", "--stdin"], Some(listing.as_bytes())) else {
        return BuildId::UNKNOWN;
    };
    let hash = String::from_utf8_lossy(&hash).trim().to_string();
    let Some(id) = hash.get(..ID_DIGITS) else {
        return BuildId::UNKNOWN;
    };
    if !id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return BuildId::UNKNOWN;
    }
    let mut args = vec![
        "status",
        "--porcelain",
        "-z",
        "--no-renames",
        "--untracked-files=all",
        "--",
    ];
    args.extend_from_slice(&paths);
    // A status that cannot be read is not a clean tree: say dirty rather than claim a clean build.
    let dirty = git_raw(repo, &args, None).is_none_or(|status| {
        String::from_utf8_lossy(&status)
            .split('\0')
            .filter(|entry| entry.len() > 3)
            .any(|entry| !is_manifest(&entry[3..]))
    });
    BuildId {
        source: Some(id.to_string()),
        dirty,
    }
}

/// The files cargo should watch so the id is refreshed: every tracked or new input file, plus the
/// current commit pointer (`HEAD` and the branch ref it names), because a commit turns "dirty" off
/// without touching any input file.
#[must_use]
pub fn watch_paths(repo: &Path, inputs: &[String]) -> Vec<PathBuf> {
    let mut watch: Vec<PathBuf> = Vec::new();
    let mut args = vec![
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
    ];
    args.extend(inputs.iter().map(String::as_str));
    if let Some(files) = git_raw(repo, &args, None) {
        watch.extend(
            String::from_utf8_lossy(&files)
                .split('\0')
                .filter(|path| !path.is_empty())
                .map(|path| repo.join(path))
                .filter(|path| path.is_file()),
        );
    }
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

/// Prints the cargo directives a build script needs: `<PREFIX>_SOURCE` (12 hex digits or
/// `unknown`), `<PREFIX>_DIRTY` (`1` or `0`) as compile-time environment, and the watch list.
/// `list` is the input list file, relative to `repo`.
pub fn emit(prefix: &str, repo: &Path, list: &str) {
    let list_path = repo.join(list);
    println!("cargo:rerun-if-changed={}", list_path.display());
    let inputs = std::fs::read_to_string(&list_path)
        .map(|text| read_inputs(&text))
        .unwrap_or_default();
    let id = detect(repo, &inputs);
    println!(
        "cargo:rustc-env={prefix}_SOURCE={}",
        id.source.as_deref().unwrap_or("unknown")
    );
    println!("cargo:rustc-env={prefix}_DIRTY={}", u8::from(id.dirty));
    for path in watch_paths(repo, &inputs) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
