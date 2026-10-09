//! Schema migrations. Each step upgrades the raw JSON by one version, so an old file is read by
//! the code of its own era and only then by the current serde types.

use serde_json::Value;

use super::schema::{ConfigFile, CONFIG_VERSION};

/// Why a file could not be turned into a [`ConfigFile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    /// Not JSON, no usable version, a failed migration, or not valid for the current schema.
    Corrupt,
    /// Written by a newer build (a downgrade). Carries that build's version.
    Newer(u32),
}

/// One step: version `i + 1` to version `i + 2`.
type Migration = fn(Value) -> Result<Value, LoadError>;

/// The real chain. Empty while version 1 is the only version.
const MIGRATIONS: &[Migration] = &[];

/// Upgrades `raw` to the current version. Returns the config and, when a migration ran, the version
/// it started from.
///
/// # Errors
/// [`LoadError::Newer`] for a version above [`CONFIG_VERSION`]; [`LoadError::Corrupt`] otherwise.
pub fn migrate(raw: Value) -> Result<(ConfigFile, Option<u32>), LoadError> {
    migrate_with(raw, MIGRATIONS, CONFIG_VERSION)
}

/// [`migrate`] against an explicit chain and target version, so tests can inject steps.
pub(crate) fn migrate_with(
    mut raw: Value,
    migrations: &[Migration],
    current: u32,
) -> Result<(ConfigFile, Option<u32>), LoadError> {
    let version = raw
        .get("version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v >= 1)
        .ok_or(LoadError::Corrupt)?;
    if version > current {
        return Err(LoadError::Newer(version));
    }
    for step in &migrations[(version - 1) as usize..(current - 1) as usize] {
        raw = step(raw)?;
    }
    let file: ConfigFile = serde_json::from_value(raw).map_err(|_| LoadError::Corrupt)?;
    if file.version != current || file.display.validate().is_err() {
        return Err(LoadError::Corrupt);
    }
    Ok((file, (version < current).then_some(version)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_real_chain_is_empty_and_current_files_load_untouched() {
        assert!(MIGRATIONS.is_empty());
        let (file, from) = migrate(json!({ "version": 1 })).unwrap();
        assert_eq!(file, ConfigFile::default());
        assert_eq!(from, None);
    }

    #[test]
    fn a_missing_zero_or_non_numeric_version_is_corrupt() {
        for raw in [
            json!({}),
            json!({ "version": 0 }),
            json!({ "version": "1" }),
            json!({ "version": -1 }),
            json!([]),
        ] {
            assert_eq!(migrate(raw).unwrap_err(), LoadError::Corrupt);
        }
    }

    #[test]
    fn a_newer_version_is_reported_with_its_number() {
        assert_eq!(
            migrate(json!({ "version": 99 })).unwrap_err(),
            LoadError::Newer(99)
        );
    }

    #[test]
    fn an_injected_hook_runs_and_reports_the_source_version() {
        // A pretend version 2 whose predecessor kept the buddy flag at the top level.
        fn v1_to_v2(mut raw: Value) -> Result<Value, LoadError> {
            let reactions = raw.as_object_mut().unwrap().remove("reactions");
            raw["buddy"] = json!({ "reactions": reactions, "intensity": "high" });
            raw["version"] = json!(2);
            Ok(raw)
        }
        let (file, from) =
            migrate_with(json!({ "version": 1, "reactions": false }), &[v1_to_v2], 2).unwrap();
        assert_eq!(from, Some(1));
        assert!(!file.buddy.reactions);
    }

    #[test]
    fn a_failing_hook_is_corrupt() {
        fn broken(_: Value) -> Result<Value, LoadError> {
            Err(LoadError::Corrupt)
        }
        assert_eq!(
            migrate_with(json!({ "version": 1 }), &[broken], 2).unwrap_err(),
            LoadError::Corrupt
        );
    }
}
