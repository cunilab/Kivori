//! Loading, recovering and atomically saving `config.json`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::migrate::{migrate, LoadError};
use super::profiles;
use super::resolve::{canonical_macro, full_override, resolve};
use super::schema::{ConfigFile, MacroSpec, ProfileRef};
use super::ResolvedConfig;
use crate::desk::profile::builtins;

const FILE: &str = "config.json";
const TMP: &str = "config.json.tmp";
const BEFORE_RESET: &str = "config.before-reset.json";
/// App identifiers Kivori used before; each one's data folder sits beside the current one. It was
/// `id.immer.kivori` until 2026-10-30.
const EARLIER_IDENTIFIERS: [&str; 1] = ["id.immer.kivori"];
/// Serialized file limit, including formatting and the final newline, shared by reads and writes.
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
    /// Older settings were converted for use and the original was backed up.
    Migrated,
    /// The original could not be preserved; replacement writes remain blocked.
    BackupFailed,
}

/// Why a change could not be saved. The text is fixed and path-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// The change itself is not acceptable.
    Invalid(&'static str),
    /// There is nowhere to store settings on this computer.
    Unavailable,
    /// The serialized settings exceed the loader's capacity; nothing changed.
    TooLarge,
    /// The original settings must be preserved before they can be replaced.
    BackupFailed,
    /// The file could not be written; nothing changed.
    Io,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => f.write_str(reason),
            Self::Unavailable => f.write_str("Settings cannot be stored on this computer."),
            Self::TooLarge => f.write_str(
                "Settings exceed the 256 KiB limit. Reduce macros or profile customizations and try again.",
            ),
            Self::BackupFailed => f.write_str(
                "The original settings could not be backed up. Nothing was changed. Try saving again.",
            ),
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
    pending_backup: Option<Backup>,
}

#[derive(Debug, Clone, Copy)]
enum Backup {
    Corrupt,
    Newer(u32),
    Migration(u32),
}

impl Backup {
    fn notice(self) -> ConfigNotice {
        match self {
            Self::Corrupt => ConfigNotice::RecoveredCorrupt,
            Self::Newer(_) => ConfigNotice::RecoveredNewerVersion,
            Self::Migration(_) => ConfigNotice::Migrated,
        }
    }

    fn preserve(self, dir: &Path) -> io::Result<()> {
        self.preserve_with(dir, |from, to| fs::rename(from, to), copy_backup)
    }

    fn preserve_with(
        self,
        dir: &Path,
        rename: impl FnOnce(&Path, &Path) -> io::Result<()>,
        copy: impl FnOnce(&Path, &Path) -> io::Result<()>,
    ) -> io::Result<()> {
        let source = dir.join(FILE);
        if let Self::Migration(from) = self {
            return copy(
                &source,
                &backup_path(dir, &format!("config.v{from}"), "bak"),
            );
        }
        let tag = match self {
            Self::Corrupt => "corrupt".to_string(),
            Self::Newer(version) => format!("v{version}"),
            Self::Migration(_) => unreachable!(),
        };
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let target = backup_path(dir, &format!("config.{tag}-{secs}"), "json");
        if rename(&source, &target).is_err() {
            copy(&source, &target)?;
            let _ = fs::remove_file(&source);
        }
        prune(dir, &target);
        Ok(())
    }
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
        Self::open_with(dir, migrate, Backup::preserve)
    }

    fn open_with(
        dir: &Path,
        migrate: impl FnOnce(Value) -> Result<(ConfigFile, Option<u32>), LoadError>,
        preserve: impl FnOnce(Backup, &Path) -> io::Result<()>,
    ) -> Self {
        let _ = fs::remove_file(dir.join(TMP));
        let mut store = Self {
            dir: Some(dir.to_path_buf()),
            file: ConfigFile::default(),
            resolved: Arc::default(),
            revision: 0,
            notice: None,
            pending_backup: None,
        };
        let path = dir.join(FILE);
        // A valid file written before the `onboarding` field existed belongs to someone who has
        // already been using Kivori: they are not sent through first-run setup. (A file that is
        // recovered as corrupt, or from a newer build, falls back to defaults and shows it once.)
        let mut predates_onboarding = false;
        let loaded = match read(&path) {
            Read::Missing => return store,
            Read::Unreadable => Err(LoadError::Corrupt),
            Read::Bytes(bytes) => serde_json::from_slice::<Value>(&bytes)
                .map_err(|_| LoadError::Corrupt)
                .and_then(|raw| {
                    predates_onboarding = raw.get("onboarding").is_none();
                    migrate(raw)
                }),
        }
        // A file whose overrides do not resolve is as unusable as one that does not parse.
        .and_then(|(mut file, from)| {
            if predates_onboarding {
                file.onboarding.completed = true;
            }
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
                store.pending_backup = Some(Backup::Migration(from));
                if store.preserve_pending(preserve).is_ok() {
                    // A failed write-back leaves the original and its backup for the next open.
                    let _ = serialize(&file).and_then(|bytes| write_atomic(dir, &bytes));
                }
                store.file = file;
                store.resolved = Arc::new(resolved);
            }
            Err(LoadError::Newer(version)) => {
                store.pending_backup = Some(Backup::Newer(version));
                let _ = store.preserve_pending(preserve);
            }
            Err(LoadError::Corrupt) => {
                store.pending_backup = Some(Backup::Corrupt);
                let _ = store.preserve_pending(preserve);
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
            pending_backup: None,
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
        let bytes = serialize(&next)?;
        self.preserve_pending(Backup::preserve)?;
        let dir = self.dir.as_deref().ok_or(ConfigError::Unavailable)?;
        write_atomic(dir, &bytes)?;
        self.file = next;
        self.resolved = Arc::new(resolved);
        self.revision += 1;
        Ok(())
    }

    /// Creates `spec`, or replaces the macro with its id. Bindings keep pointing at the id.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn save_macro(&mut self, spec: MacroSpec) -> Result<(), ConfigError> {
        let spec = canonical_macro(&spec).map_err(ConfigError::Invalid)?;
        let next = self.edited(
            |file| match file.macros.iter_mut().find(|m| m.id == spec.id) {
                Some(existing) => *existing = spec,
                None => file.macros.push(spec),
            },
        );
        self.save(next)
    }

    /// Deletes macro `id`. Refused while a control is bound to it, so nothing becomes unbound
    /// silently.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn delete_macro(&mut self, id: &str) -> Result<(), ConfigError> {
        if self.file.macro_bound(id) {
            return Err(ConfigError::Invalid(
                "This macro is still bound to a control. Unbind it first.",
            ));
        }
        if !self.file.macros.iter().any(|m| m.id == id) {
            return Err(ConfigError::Invalid("unknown macro"));
        }
        let next = self.edited(|file| file.macros.retain(|m| m.id != id));
        self.save(next)
    }

    /// Creates a custom profile for `app` (`name` is its display name, may be empty) as a copy of
    /// General's bindings as they are now, moving `app` out of any profile that held it.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn add_custom_profile(&mut self, app: &str, name: &str) -> Result<(), ConfigError> {
        let general = full_override(&self.resolved.profiles[0]);
        let mut next = self.file.clone();
        profiles::add_custom_profile(&mut next, app, name, general)
            .map_err(ConfigError::Invalid)?;
        self.save(next)
    }

    /// Moves `app` into `profile` (General = out of every other profile).
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn assign_app(&mut self, profile: &ProfileRef, app: &str) -> Result<(), ConfigError> {
        let mut next = self.file.clone();
        profiles::assign_app(&mut next, profile, app).map_err(ConfigError::Invalid)?;
        self.save(next)
    }

    /// Renames a custom profile.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn rename_profile(&mut self, profile: &ProfileRef, name: &str) -> Result<(), ConfigError> {
        let mut next = self.file.clone();
        profiles::rename_profile(&mut next, profile, name).map_err(ConfigError::Invalid)?;
        self.save(next)
    }

    /// Deletes a custom profile.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn delete_profile(&mut self, profile: &ProfileRef) -> Result<(), ConfigError> {
        let mut next = self.file.clone();
        profiles::delete_profile(&mut next, profile).map_err(ConfigError::Invalid)?;
        self.save(next)
    }

    /// Puts a custom profile in or out of the pin cycle.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn set_pin_cycle(&mut self, profile: &ProfileRef, on: bool) -> Result<(), ConfigError> {
        let mut next = self.file.clone();
        profiles::set_pin_cycle(&mut next, profile, on).map_err(ConfigError::Invalid)?;
        self.save(next)
    }

    /// A built-in profile goes back to its built-in bindings and app list; a custom one goes back
    /// to a fresh copy of General's bindings (its name and apps stay).
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn reset_profile(&mut self, profile: &ProfileRef) -> Result<(), ConfigError> {
        let general = full_override(&self.resolved.profiles[0]);
        let mut next = self.file.clone();
        match profile {
            ProfileRef::Builtin(id) => {
                next.profiles.remove(id);
            }
            ProfileRef::Custom(id) => {
                let custom = next
                    .custom_profiles
                    .iter_mut()
                    .find(|p| p.id == *id)
                    .ok_or(ConfigError::Invalid("unknown profile"))?;
                custom.bindings = general;
            }
        }
        self.save(next)
    }

    /// Restores every setting to its default, keeping the previous file as `config.before-reset.json`.
    ///
    /// # Errors
    /// [`ConfigError`]; on any error the stored config is unchanged.
    pub fn reset(&mut self) -> Result<(), ConfigError> {
        // Resetting settings does not send a set-up user back through first-run setup.
        let defaults = ConfigFile {
            onboarding: self.file.onboarding,
            ..ConfigFile::default()
        };
        let bytes = serialize(&defaults)?;
        self.preserve_pending(Backup::preserve)?;
        let dir = self.dir.as_deref().ok_or(ConfigError::Unavailable)?;
        let path = dir.join(FILE);
        if path.exists() {
            fs::copy(&path, dir.join(BEFORE_RESET)).map_err(|_| ConfigError::Io)?;
        }
        write_atomic(dir, &bytes)?;
        self.file = defaults;
        self.resolved = Arc::default();
        self.revision += 1;
        self.notice = None;
        Ok(())
    }

    fn preserve_pending(
        &mut self,
        preserve: impl FnOnce(Backup, &Path) -> io::Result<()>,
    ) -> Result<(), ConfigError> {
        let Some(backup) = self.pending_backup else {
            return Ok(());
        };
        let dir = self.dir.as_deref().ok_or(ConfigError::Unavailable)?;
        if preserve(backup, dir).is_err() {
            self.notice = Some(ConfigNotice::BackupFailed);
            return Err(ConfigError::BackupFailed);
        }
        self.pending_backup = None;
        self.notice = Some(backup.notice());
        Ok(())
    }
}

/// Copies `config.json` from an earlier identifier's data folder into `dir` once, when `dir` has
/// none, so renaming the app identifier keeps people's settings; [`ConfigStore::open`] then loads
/// (and if needed migrates) it as usual. The earlier folder is left as it was, so an older build
/// still finds its own settings. Returns whether settings were adopted.
pub fn adopt_earlier_config(dir: &Path) -> bool {
    if !matches!(read(&dir.join(FILE)), Read::Missing) {
        return false;
    }
    let Some(parent) = dir.parent() else {
        return false;
    };
    EARLIER_IDENTIFIERS
        .iter()
        .any(|id| match read(&parent.join(id).join(FILE)) {
            Read::Bytes(bytes) => write_atomic(dir, &bytes).is_ok(),
            Read::Missing | Read::Unreadable => false,
        })
}

fn read(path: &Path) -> Read {
    match fs::metadata(path) {
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Read::Missing
        }
        Ok(meta) if meta.is_file() && meta.len() <= MAX_BYTES => match fs::read(path) {
            Ok(bytes) => Read::Bytes(bytes),
            Err(_) => Read::Unreadable,
        },
        _ => Read::Unreadable,
    }
}

fn serialize(file: &ConfigFile) -> Result<Vec<u8>, ConfigError> {
    let mut bytes = serde_json::to_vec_pretty(file).map_err(|_| ConfigError::Io)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_BYTES {
        return Err(ConfigError::TooLarge);
    }
    Ok(bytes)
}

/// Writes the already-validated bytes, flushes to disk, then atomically replaces `config.json`.
/// Rename retries accommodate short antivirus/indexer locks on Windows; there is no fallback.
fn write_atomic(dir: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    fs::create_dir_all(dir).map_err(|_| ConfigError::Io)?;
    let tmp = dir.join(TMP);
    let written = fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()
    });
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(ConfigError::Io);
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
    Err(ConfigError::Io)
}

/// Selects a fresh name so a retry never overwrites an earlier original or partial copy.
fn backup_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let mut target = dir.join(format!("{stem}.{extension}"));
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("{stem}-{n}.{extension}"));
        n += 1;
    }
    target
}

fn copy_backup(source: &Path, target: &Path) -> io::Result<()> {
    copy_backup_with(source, target, |source, backup| {
        io::copy(source, backup).map(|_| ())
    })
}

fn copy_backup_with(
    source: &Path,
    target: &Path,
    copy: impl FnOnce(&mut fs::File, &mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let mut source = fs::File::open(source)?;
    let mut backup = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    let copied = (|| {
        copy(&mut source, &mut backup)?;
        backup.set_permissions(source.metadata()?.permissions())?;
        backup.sync_all()
    })();
    if copied.is_err() {
        drop(backup);
        let _ = fs::remove_file(target);
    }
    copied
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

/// Keeps the newly preserved original plus the newest remaining quarantine backups, at most
/// [`KEEP_BACKUPS`] total. Reset and migration `.bak` copies are outside this set.
fn prune(dir: &Path, preserved: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<((u64, u32), PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path() != preserved)
        .filter_map(|entry| {
            let key = backup_key(entry.file_name().to_str()?)?;
            Some((key, entry.path()))
        })
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(KEEP_BACKUPS - 1);
    for (_, path) in backups.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `<root>/<current>` beside `<root>/id.immer.kivori` holding `bytes` (if any).
    fn identifier_dirs(earlier: Option<&[u8]>) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join(EARLIER_IDENTIFIERS[0]);
        if let Some(bytes) = earlier {
            fs::create_dir_all(&old).unwrap();
            fs::write(old.join(FILE), bytes).unwrap();
        }
        let current = root.path().join("id.cunilab.kivori");
        (root, current, old)
    }

    #[test]
    fn settings_from_the_earlier_identifier_are_adopted_once_and_left_in_place() {
        let saved = br#"{"version":1,"onboarding":{"completed":true}}"#;
        let (_root, current, old) = identifier_dirs(Some(saved));
        assert!(adopt_earlier_config(&current));
        assert_eq!(fs::read(current.join(FILE)).unwrap(), saved);
        assert_eq!(
            fs::read(old.join(FILE)).unwrap(),
            saved,
            "earlier folder untouched"
        );
        let store = ConfigStore::open(&current);
        assert!(
            store.file().onboarding.completed,
            "adopted settings load normally"
        );
        // Once the current folder has settings, the earlier ones never overwrite them again.
        fs::write(old.join(FILE), b"{}").unwrap();
        assert!(!adopt_earlier_config(&current));
        assert_eq!(fs::read(current.join(FILE)).unwrap(), saved);
    }

    #[test]
    fn nothing_is_adopted_without_usable_earlier_settings() {
        let (_root, current, _old) = identifier_dirs(None);
        assert!(!adopt_earlier_config(&current));
        assert!(!current.join(FILE).exists());
        let too_big = vec![b' '; usize::try_from(MAX_BYTES).unwrap() + 1];
        let (_root, current, _old) = identifier_dirs(Some(&too_big));
        assert!(!adopt_earlier_config(&current));
        assert!(!current.join(FILE).exists());
    }

    fn fail_backup(backup: Backup, dir: &Path) -> io::Result<()> {
        backup.preserve_with(
            dir,
            |_, _| Err(io::ErrorKind::PermissionDenied.into()),
            |_, _| Err(io::ErrorKind::PermissionDenied.into()),
        )
    }

    // The released schema is still v1; inject migration output to exercise its real store branch.
    fn migrated(raw: Value) -> Result<(ConfigFile, Option<u32>), LoadError> {
        let mut file: ConfigFile = serde_json::from_value(raw).map_err(|_| LoadError::Corrupt)?;
        file.buddy.intensity = super::super::Intensity::High;
        Ok((file, Some(1)))
    }

    fn assert_copy_exists(dir: &Path, bytes: &[u8]) {
        assert!(
            fs::read_dir(dir)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| {
                    entry.file_name() != FILE
                        && fs::read(entry.path()).ok().as_deref() == Some(bytes)
                }),
            "original bytes must survive in a separate backup"
        );
    }

    #[test]
    fn quarantine_failure_blocks_save_and_reset_until_the_original_is_preserved() {
        for original in [
            b"damaged settings".as_slice(),
            br#"{"version":99}"#.as_slice(),
        ] {
            for reset in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join(FILE);
                fs::write(&path, original).unwrap();
                let mut store = ConfigStore::open_with(dir.path(), migrate, fail_backup);
                assert_eq!(fs::read(&path).unwrap(), original);
                assert_eq!(store.file(), &ConfigFile::default());
                assert_eq!(store.notice(), Some(ConfigNotice::BackupFailed));
                assert_eq!(
                    crate::ipc::dto::config_dto(&store).notice,
                    Some("backupFailed")
                );
                let cache = store.resolved();
                let next = store.edited(|f| f.buddy.reactions = false);
                // The source remains unavailable on retry, while its bytes stay safely in the rig.
                let parked = dir.path().join("parked-original");
                fs::rename(&path, &parked).unwrap();
                let error = if reset {
                    store.reset()
                } else {
                    store.save(next.clone())
                }
                .unwrap_err();
                assert_eq!(error, ConfigError::BackupFailed);
                assert!(!error.to_string().contains(dir.path().to_str().unwrap()));
                assert!(!path.exists());
                assert!(!dir.path().join(TMP).exists());
                assert!(Arc::ptr_eq(&cache, &store.resolved()));
                assert_eq!(store.revision(), 0);
                assert_eq!(fs::read(&parked).unwrap(), original);
                fs::rename(&parked, &path).unwrap();
                if reset {
                    store.reset().unwrap();
                } else {
                    store.save(next).unwrap();
                }
                assert_copy_exists(dir.path(), original);
                assert_eq!(store.revision(), 1);
                assert_ne!(store.notice(), Some(ConfigNotice::BackupFailed));
                assert_eq!(ConfigStore::open(dir.path()).file(), store.file());
            }
        }
    }

    #[test]
    fn migration_backup_failure_blocks_replacement_and_retains_usable_settings() {
        for reset in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(FILE);
            let original = br#"{"version":1,"onboarding":{"completed":true}}"#;
            fs::write(&path, original).unwrap();
            fs::write(dir.path().join("config.v1.bak"), b"earlier original").unwrap();
            let mut store = ConfigStore::open_with(dir.path(), migrated, fail_backup);
            assert_eq!(fs::read(&path).unwrap(), original);
            assert_eq!(store.file().buddy.intensity, super::super::Intensity::High);
            assert_eq!(store.notice(), Some(ConfigNotice::BackupFailed));
            let next = store.file().clone();
            let parked = dir.path().join("parked-original");
            fs::rename(&path, &parked).unwrap();
            assert_eq!(
                if reset {
                    store.reset()
                } else {
                    store.save(next.clone())
                },
                Err(ConfigError::BackupFailed)
            );
            assert!(!path.exists());
            assert_eq!(store.revision(), 0);
            fs::rename(&parked, &path).unwrap();
            if reset {
                store.reset().unwrap();
            } else {
                store.save(next).unwrap();
            }
            assert_eq!(
                fs::read(dir.path().join("config.v1.bak")).unwrap(),
                b"earlier original"
            );
            assert_eq!(
                fs::read(dir.path().join("config.v1-1.bak")).unwrap(),
                original
            );
            assert_eq!(ConfigStore::open(dir.path()).file(), store.file());
        }
    }

    #[test]
    fn failed_migration_writeback_keeps_its_backup_and_can_retry_without_replacing_it() {
        let dir = tempfile::tempdir().unwrap();
        let original = br#"{"version":1,"onboarding":{"completed":true}}"#;
        fs::write(dir.path().join(FILE), original).unwrap();
        // Opening removes stale temp files, but this directory keeps the atomic write from starting.
        fs::create_dir(dir.path().join(TMP)).unwrap();
        let mut store = ConfigStore::open_with(dir.path(), migrated, Backup::preserve);
        assert_eq!(store.notice(), Some(ConfigNotice::Migrated));
        assert_eq!(fs::read(dir.path().join(FILE)).unwrap(), original);
        assert_eq!(
            fs::read(dir.path().join("config.v1.bak")).unwrap(),
            original
        );
        assert_eq!(store.revision(), 0);
        fs::remove_dir(dir.path().join(TMP)).unwrap();
        store.save(store.file().clone()).unwrap();
        assert_eq!(
            fs::read(dir.path().join("config.v1.bak")).unwrap(),
            original
        );
        assert!(!dir.path().join("config.v1-1.bak").exists());
    }

    #[test]
    fn quarantine_copy_fallback_preserves_bytes_when_rename_fails() {
        let dir = tempfile::tempdir().unwrap();
        let original = b"keep every original byte";
        fs::write(dir.path().join(FILE), original).unwrap();
        Backup::Corrupt
            .preserve_with(
                dir.path(),
                |_, _| Err(io::ErrorKind::PermissionDenied.into()),
                copy_backup,
            )
            .unwrap();
        assert_copy_exists(dir.path(), original);
        assert!(!dir.path().join(FILE).exists());
    }

    #[test]
    fn a_failed_copy_removes_its_partial_output_and_never_clobbers_an_existing_backup() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join(FILE);
        let target = dir.path().join("original.bak");
        fs::write(&source, b"original").unwrap();
        assert!(copy_backup_with(&source, &target, |_, output| {
            output.write_all(b"partial")?;
            Err(io::ErrorKind::WriteZero.into())
        })
        .is_err());
        assert!(!target.exists());
        assert_eq!(fs::read(&source).unwrap(), b"original");
        fs::write(&target, b"earlier original").unwrap();
        assert!(copy_backup(&source, &target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"earlier original");
    }

    #[test]
    fn pruning_never_deletes_the_new_original_even_if_older_backups_have_future_times() {
        let dir = tempfile::tempdir().unwrap();
        for n in 1..=8 {
            fs::write(
                dir.path()
                    .join(format!("config.corrupt-{}.json", u64::MAX - n)),
                b"old",
            )
            .unwrap();
        }
        let original = b"current original";
        fs::write(dir.path().join(FILE), original).unwrap();
        Backup::Corrupt.preserve(dir.path()).unwrap();
        assert_copy_exists(dir.path(), original);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), KEEP_BACKUPS);
    }

    #[test]
    fn an_oversized_candidate_does_not_attempt_pending_preservation() {
        let dir = tempfile::tempdir().unwrap();
        let original = b"original";
        fs::write(dir.path().join(FILE), original).unwrap();
        let mut store = ConfigStore::open_with(dir.path(), migrate, fail_backup);
        let next =
            store.edited(|f| f.updates.skipped_version = Some("x".repeat(MAX_BYTES as usize)));
        assert_eq!(store.save(next), Err(ConfigError::TooLarge));
        assert_eq!(store.notice(), Some(ConfigNotice::BackupFailed));
        assert_eq!(fs::read(dir.path().join(FILE)).unwrap(), original);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

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
