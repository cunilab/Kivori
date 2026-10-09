//! Loading, recovering and atomically saving `config.json`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::migrate::{migrate, LoadError};
use super::resolve::resolve;
use super::schema::ConfigFile;
use super::ResolvedConfig;
use crate::desk::profile::builtins;

const FILE: &str = "config.json";
const TMP: &str = "config.json.tmp";
const BEFORE_RESET: &str = "config.before-reset.json";
/// A larger file is never a config Kivori wrote.
const MAX_BYTES: u64 = 256 * 1024;
/// How many quarantined `config.*-*.json` files are kept.
const KEEP_BACKUPS: usize = 5;
const RENAME_ATTEMPTS: u32 = 3;
const RENAME_RETRY: Duration = Duration::from_millis(50);

/// Something the user should be told about the file that was loaded. Never carries a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigNotice {
    /// The file was unreadable or invalid; it was set aside and defaults are in use.
    RecoveredCorrupt,
    /// The file came from a newer Kivori; it was set aside and defaults are in use.
    RecoveredNewerVersion,
    /// An older file was upgraded in place (information only).
    Migrated,
}

/// Why a change could not be saved. The text is fixed and path-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// The change itself is not acceptable.
    Invalid(&'static str),
    /// There is nowhere to store settings on this computer.
    Unavailable,
    /// The file could not be written; nothing changed.
    Io,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => f.write_str(reason),
            Self::Unavailable => f.write_str("Settings cannot be stored on this computer."),
            Self::Io => f.write_str("Settings could not be saved. Nothing was changed."),
        }
    }
}

/// The current config and the file behind it.
#[derive(Debug)]
pub struct ConfigStore {
    /// `None` = nowhere to persist (defaults only; every save fails).
    dir: Option<PathBuf>,
    file: ConfigFile,
    /// `file` layered over the built-ins; always in step with it.
    resolved: Arc<ResolvedConfig>,
    /// Counts successful saves and resets in this process.
    revision: u64,
    notice: Option<ConfigNotice>,
}

enum Read {
    Missing,
    Bytes(Vec<u8>),
    Unreadable,
}

impl ConfigStore {
    /// Opens the store in `dir`. Never fails: whatever is wrong with the file, the app gets a
    /// usable config and, if something was set aside or upgraded, a [`ConfigNotice`].
    #[must_use]
    pub fn open(dir: &Path) -> Self {
        let _ = fs::remove_file(dir.join(TMP));
        let mut store = Self {
            dir: Some(dir.to_path_buf()),
            file: ConfigFile::default(),
            resolved: Arc::default(),
            revision: 0,
            notice: None,
        };
        let path = dir.join(FILE);
        let loaded = match read(&path) {
            Read::Missing => return store,
            Read::Unreadable => Err(LoadError::Corrupt),
            Read::Bytes(bytes) => serde_json::from_slice::<Value>(&bytes)
                .map_err(|_| LoadError::Corrupt)
                .and_then(migrate),
        }
        // A file whose overrides do not resolve is as unusable as one that does not parse.
        .and_then(|(file, from)| {
            resolve(builtins(), &file)
                .map(|resolved| (file, resolved, from))
                .map_err(|_| LoadError::Corrupt)
        });
        match loaded {
            Ok((file, resolved, None)) => {
                store.file = file;
                store.resolved = Arc::new(resolved);
            }
            Ok((file, resolved, Some(from))) => {
                let _ = fs::copy(&path, dir.join(format!("config.v{from}.bak")));
                store.file = file;
                store.resolved = Arc::new(resolved);
                // A failed write-back is harmless: the next open migrates again.
                let _ = write_atomic(dir, &store.file);
                store.notice = Some(ConfigNotice::Migrated);
            }
            Err(LoadError::Newer(version)) => {
                quarantine(dir, &path, &format!("v{version}"));
                store.notice = Some(ConfigNotice::RecoveredNewerVersion);
            }
            Err(LoadError::Corrupt) => {
                quarantine(dir, &path, "corrupt");
                store.notice = Some(ConfigNotice::RecoveredCorrupt);
            }
        }
        store
    }

    /// A store with no backing file: defaults, and every save is refused.
    #[must_use]
    pub fn detached() -> Self {
        Self {
            dir: None,
            file: ConfigFile::default(),
            resolved: Arc::default(),
            revision: 0,
            notice: None,
        }
    }

    #[must_use]
    pub const fn file(&self) -> &ConfigFile {
        &self.file
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn notice(&self) -> Option<ConfigNotice> {
        self.notice
    }

    /// A copy of the current file with `edit` applied, ready to [`Self::save`].
    #[must_use]
    pub fn edited(&self, edit: impl FnOnce(&mut ConfigFile)) -> ConfigFile {
        let mut next = self.file.clone();
        edit(&mut next);
        next
    }

    #[must_use]
    pub fn resolved(&self) -> Arc<ResolvedConfig> {
        Arc::clone(&self.resolved)
    }

    /// Validates `next`, writes it atomically, and only then makes it current.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn save(&mut self, next: ConfigFile) -> Result<(), ConfigError> {
        next.validate().map_err(ConfigError::Invalid)?;
        let resolved = resolve(builtins(), &next)?;
        let dir = self.dir.as_deref().ok_or(ConfigError::Unavailable)?;
        write_atomic(dir, &next).map_err(|()| ConfigError::Io)?;
        self.file = next;
        self.resolved = Arc::new(resolved);
        self.revision += 1;
        Ok(())
    }

    /// Restores every setting to its default, keeping the previous file as `config.before-reset.json`.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn reset(&mut self) -> Result<(), ConfigError> {
        let dir = self.dir.as_deref().ok_or(ConfigError::Unavailable)?;
        let path = dir.join(FILE);
        if path.exists() {
            fs::copy(&path, dir.join(BEFORE_RESET)).map_err(|_| ConfigError::Io)?;
        }
        let defaults = ConfigFile::default();
        write_atomic(dir, &defaults).map_err(|()| ConfigError::Io)?;
        self.file = defaults;
        self.resolved = Arc::default();
        self.revision += 1;
        self.notice = None;
        Ok(())
    }
}

fn read(path: &Path) -> Read {
    match fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Read::Missing,
        Ok(meta) if meta.is_file() && meta.len() <= MAX_BYTES => match fs::read(path) {
            Ok(bytes) => Read::Bytes(bytes),
            Err(_) => Read::Unreadable,
        },
        _ => Read::Unreadable,
    }
}

/// Writes `config.json.tmp`, flushes it to disk, then renames it over `config.json`. The rename is
/// retried briefly because antivirus or an indexer can hold the target for a moment on Windows.
/// There is deliberately no non-atomic fallback.
fn write_atomic(dir: &Path, file: &ConfigFile) -> Result<(), ()> {
    fs::create_dir_all(dir).map_err(|_| ())?;
    let mut bytes = serde_json::to_vec_pretty(file).map_err(|_| ())?;
    bytes.push(b'\n');
    let tmp = dir.join(TMP);
    let written = fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(&bytes)?;
        f.sync_all()
    });
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(());
    }
    let target = dir.join(FILE);
    for attempt in 1..=RENAME_ATTEMPTS {
        if fs::rename(&tmp, &target).is_ok() {
            return Ok(());
        }
        if attempt < RENAME_ATTEMPTS {
            std::thread::sleep(RENAME_RETRY);
        }
    }
    let _ = fs::remove_file(&tmp);
    Err(())
}

/// Moves a file Kivori cannot use to `config.<tag>-<unix_secs>.json` (copying if the move fails,
/// so the original is never left to be overwritten), then prunes old backups.
fn quarantine(dir: &Path, path: &Path, tag: &str) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut target = dir.join(format!("config.{tag}-{secs}.json"));
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("config.{tag}-{secs}-{n}.json"));
        n += 1;
    }
    if fs::rename(path, &target).is_err() && fs::copy(path, &target).is_ok() {
        let _ = fs::remove_file(path);
    }
    prune(dir);
}

/// `(unix_secs, collision counter)` of a `config.<tag>-<secs>[-<n>].json` backup name.
fn backup_key(name: &str) -> Option<(u64, u32)> {
    let stem = name.strip_prefix("config.")?.strip_suffix(".json")?;
    let mut parts = stem.split('-');
    parts.next()?;
    let secs = parts.next()?.parse().ok()?;
    let n = parts.next().map_or(Some(0), |n| n.parse().ok())?;
    parts.next().is_none().then_some((secs, n))
}

/// Keeps the newest [`KEEP_BACKUPS`] quarantined files. `config.before-reset.json` and the
/// `.bak` migration copies are not in this set.
fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<((u64, u32), PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let key = backup_key(entry.file_name().to_str()?)?;
            Some((key, entry.path()))
        })
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(KEEP_BACKUPS);
    for (_, path) in backups.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::backup_key;

    #[test]
    fn only_quarantine_names_are_backups() {
        assert_eq!(
            backup_key("config.corrupt-1700000000.json"),
            Some((1_700_000_000, 0))
        );
        assert_eq!(
            backup_key("config.v99-1700000000-2.json"),
            Some((1_700_000_000, 2))
        );
        for other in [
            "config.json",
            "config.before-reset.json",
            "config.v1.bak",
            "config.json.tmp",
            "config.corrupt-x.json",
        ] {
            assert_eq!(backup_key(other), None, "{other}");
        }
    }
}
