//! One-click, user-local installation of the embedded GNOME bridge. Never installs on startup.
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use super::session::{session_kind, SessionKind};

pub const UUID: &str = "kivori-focus@cunilab.id";
const METADATA: &str =
    include_str!("../../../linux/gnome-extension/kivori-focus@cunilab.id/metadata.json");
const EXTENSION: &str =
    include_str!("../../../linux/gnome-extension/kivori-focus@cunilab.id/extension.js");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FocusStatus {
    NotApplicable,
    NotInstalled,
    NeedsLogout,
    Working,
    Disabled,
    Unsupported,
    Unavailable,
}

fn is_gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| {
        desktop
            .split(':')
            .any(|part| part.eq_ignore_ascii_case("gnome"))
    })
}

fn output(program: &str, args: &[&str]) -> Result<String, String> {
    let result = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("Could not run {program}: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "{program}: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    String::from_utf8(result.stdout).map_err(|e| e.to_string())
}

fn supported(version: &str) -> bool {
    version
        .split_whitespace()
        .find_map(|word| word.split('.').next()?.parse::<u32>().ok())
        .is_some_and(|major| (46..=50).contains(&major))
}

fn extension_dir() -> Result<PathBuf, String> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or("Your home folder could not be found.")?;
    Ok(data.join("gnome-shell/extensions").join(UUID))
}

/// GSettings prints a GVariant string array, with optional `@as` for an empty array. Parse its
/// strings instead of matching substrings, preserving every unrelated enabled extension.
fn extension_list(text: &str) -> Result<Vec<String>, String> {
    let text = text.trim().strip_prefix("@as ").unwrap_or(text.trim());
    let Some(inner) = text.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        return Err("Could not read GNOME's enabled extensions.".into());
    };
    let mut chars = inner.chars().peekable();
    let mut values = Vec::new();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() || ch == ',' {
            continue;
        }
        if ch != '\'' && ch != '"' {
            return Err("Invalid GNOME extension list.".into());
        }
        let quote = ch;
        let mut value = String::new();
        let mut closed = false;
        while let Some(c) = chars.next() {
            if c == quote {
                closed = true;
                break;
            }
            if c == '\\' {
                value.push(chars.next().ok_or("Invalid GNOME extension list.")?);
            } else {
                value.push(c);
            }
        }
        if !closed {
            return Err("Invalid GNOME extension list.".into());
        }
        values.push(value);
    }
    Ok(values)
}

fn variant_list(values: &[String]) -> String {
    let quoted: Vec<String> = values
        .iter()
        .map(|value| format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'")))
        .collect();
    format!("[{}]", quoted.join(", "))
}

fn detect(
    installed: bool,
    enabled: &[String],
    globally_disabled: bool,
    info: Option<&str>,
    working: bool,
) -> FocusStatus {
    if working {
        return FocusStatus::Working;
    }
    if !installed {
        return FocusStatus::NotInstalled;
    }
    if globally_disabled || !enabled.iter().any(|s| s == UUID) {
        return FocusStatus::Disabled;
    }
    match info
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.trim().strip_prefix("State:"))
        })
        .map(str::trim)
    {
        None => FocusStatus::NeedsLogout,
        Some("OUT OF DATE") => FocusStatus::Unsupported,
        Some("ERROR" | "ACTIVE" | "ENABLED") => FocusStatus::Unavailable,
        // Loaded but disabled, even if the preference still contains it (session-wide toggles).
        Some(_) => FocusStatus::Disabled,
    }
}

pub fn status() -> FocusStatus {
    if session_kind() != SessionKind::Wayland || !is_gnome() {
        return FocusStatus::NotApplicable;
    }
    match output("gnome-shell", &["--version"]) {
        Ok(version) if !supported(&version) => return FocusStatus::Unsupported,
        Err(_) => return FocusStatus::Unavailable,
        _ => {}
    }
    let installed = extension_dir().is_ok_and(|dir| dir.join("metadata.json").is_file());
    let working = super::gnome_focus::connection()
        .and_then(|conn| super::gnome_focus::read(&conn))
        .is_ok();
    let Ok(enabled) = output(
        "gsettings",
        &["get", "org.gnome.shell", "enabled-extensions"],
    )
    .and_then(|text| extension_list(&text)) else {
        return if working {
            FocusStatus::Working
        } else {
            FocusStatus::Unavailable
        };
    };
    let globally_disabled = output(
        "gsettings",
        &["get", "org.gnome.shell", "disable-user-extensions"],
    )
    .is_ok_and(|text| text.trim() == "true");
    let info = output("gnome-extensions", &["info", UUID]).ok();
    detect(
        installed,
        &enabled,
        globally_disabled,
        info.as_deref(),
        working,
    )
}

fn write_extension(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not create extension folder: {e}"))?;
    for (name, text) in [("metadata.json", METADATA), ("extension.js", EXTENSION)] {
        std::fs::write(dir.join(name), text).map_err(|e| format!("Could not write {name}: {e}"))?;
    }
    Ok(())
}

pub fn install() -> Result<FocusStatus, String> {
    match status() {
        FocusStatus::NotApplicable => {
            return Err("App profiles require a GNOME Wayland session.".into())
        }
        FocusStatus::Unsupported => {
            return Err(
                "This extension supports GNOME 46–50. Update Kivori for your GNOME version.".into(),
            )
        }
        FocusStatus::Working => return Ok(FocusStatus::Working),
        _ => {}
    }
    if output(
        "gsettings",
        &["get", "org.gnome.shell", "disable-user-extensions"],
    )?
    .trim()
        == "true"
    {
        return Err(
            "GNOME extensions are turned off. Turn them on in the Extensions app, then try again."
                .into(),
        );
    }
    // Read before writing; never replace other extension preferences with an empty fallback.
    let mut enabled = extension_list(&output(
        "gsettings",
        &["get", "org.gnome.shell", "enabled-extensions"],
    )?)?;
    write_extension(&extension_dir()?)?;
    // New extensions cannot be hot-loaded on Wayland. Persist the opt-in even if enable reports
    // "does not exist" until Shell discovers the files at the next login.
    let enable_result = output("gnome-extensions", &["enable", UUID]);
    if !enabled.iter().any(|s| s == UUID) {
        enabled.push(UUID.into());
    }
    output(
        "gsettings",
        &[
            "set",
            "org.gnome.shell",
            "enabled-extensions",
            &variant_list(&enabled),
        ],
    )?;
    let state = status();
    if state == FocusStatus::Unavailable {
        return Err(enable_result.err().unwrap_or_else(|| {
            "GNOME could not start the extension. Open Extensions to check Kivori App Profiles."
                .into()
        }));
    }
    Ok(state)
}

pub fn logout() -> Result<(), String> {
    if session_kind() != SessionKind::Wayland || !is_gnome() {
        return Err("This action requires GNOME Wayland.".into());
    }
    // GNOME shows its normal confirmation dialog; never force or bypass unsaved work handling.
    Command::new("gnome-session-quit")
        .arg("--logout")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_versions_and_gsettings_round_trip() {
        for version in ["GNOME Shell 46.0", "GNOME Shell 50.1"] {
            assert!(supported(version));
        }
        for version in ["GNOME Shell 45.9", "GNOME Shell 51.alpha", "unknown"] {
            assert!(!supported(version));
        }
        assert!(extension_list("@as []").unwrap().is_empty());
        let values = vec![
            "ubuntu-appindicators@ubuntu.com".into(),
            UUID.into(),
            "a'b\\c".into(),
        ];
        assert_eq!(extension_list(&variant_list(&values)).unwrap(), values);
        assert!(extension_list("garbage").is_err());
        assert!(extension_list("['unfinished]").is_err());
    }

    #[test]
    fn states_use_exact_preferences_and_cli_output() {
        let enabled = vec![UUID.into()];
        assert_eq!(
            detect(false, &[], false, None, false),
            FocusStatus::NotInstalled
        );
        assert_eq!(
            detect(true, &enabled, false, None, false),
            FocusStatus::NeedsLogout
        );
        assert_eq!(
            detect(true, &[], false, Some("State: DISABLED"), false),
            FocusStatus::Disabled
        );
        assert_eq!(
            detect(true, &enabled, true, Some("State: INITIALIZED"), false),
            FocusStatus::Disabled
        );
        assert_eq!(
            detect(true, &enabled, false, Some("  State: ERROR"), false),
            FocusStatus::Unavailable
        );
        assert_eq!(
            detect(true, &enabled, false, Some("  State: OUT OF DATE"), false),
            FocusStatus::Unsupported
        );
        assert_eq!(
            detect(true, &enabled, false, Some("State: ACTIVE"), true),
            FocusStatus::Working
        );
        assert_eq!(
            detect(true, &[format!("other-{UUID}")], false, None, false),
            FocusStatus::Disabled
        );
    }

    #[test]
    fn install_writes_embedded_files_under_a_temporary_home() {
        let home = tempfile::tempdir().unwrap();
        let dir = home
            .path()
            .join(".local/share/gnome-shell/extensions")
            .join(UUID);
        write_extension(&dir).unwrap();
        let metadata: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("metadata.json")).unwrap())
                .unwrap();
        assert_eq!(metadata["uuid"], UUID);
        assert_eq!(
            metadata["shell-version"],
            serde_json::json!(["46", "47", "48", "49", "50"])
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("extension.js")).unwrap(),
            EXTENSION
        );
        write_extension(&dir).unwrap(); // Repeat install is safe.
    }
}
