//! One-click, user-local installation of the embedded GNOME bridge. Never installs on startup.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use serde::Serialize;

use super::gnome_focus::{compatible, Peer, VERSION};
use super::session::{is_gnome, session_kind, SessionKind};

/// GNOME shortcuts share the extension setup card, not the ephemeral portal permission card.
pub fn manages_shortcuts() -> bool {
    session_kind() == SessionKind::Wayland && is_gnome()
}

/// Used by the action picker; execution enforces the same policy independently.
pub fn shortcut_policy(keys: &str) -> Option<String> {
    if !manages_shortcuts() {
        return None;
    }
    let result = keys
        .parse::<crate::platform::Shortcut>()
        .map_err(|e| e.to_string())
        .and_then(|shortcut| {
            super::gnome_focus::shortcut_args(&shortcut)
                .map(|_| ())
                .map_err(|e| format!("{e:?}"))
        });
    result.err().map(|_| {
        "This shortcut isn’t allowed. Try a different shortcut, such as Ctrl+Shift+T.".into()
    })
}

pub const UUID: &str = "kivori-focus@cunilab.id";
const METADATA: &str =
    include_str!("../../../linux/gnome-extension/kivori-focus@cunilab.id/metadata.json");
const EXTENSION: &str =
    include_str!("../../../linux/gnome-extension/kivori-focus@cunilab.id/extension.js");
const BRIDGE: &str =
    include_str!("../../../linux/gnome-extension/kivori-focus@cunilab.id/bridge.js");
const SECURITY: &str =
    include_str!("../../../linux/gnome-extension/kivori-focus@cunilab.id/security.js");
const FILES: [(&str, &str); 4] = [
    ("metadata.json", METADATA),
    ("extension.js", EXTENSION),
    ("bridge.js", BRIDGE),
    ("security.js", SECURITY),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FocusStatus {
    NotApplicable,
    NotInstalled,
    /// Written and enabled, but GNOME Shell has not started since (new install or an update).
    NeedsLogout,
    Working,
    Disabled,
    Unsupported,
    Unavailable,
    /// An older copy is installed or running; installing writes the new one.
    UpdateAvailable,
    /// `gnome-extensions` cannot be run, so the extension's state cannot be checked.
    CliMissing,
}

/// What `gnome-extensions info` told us.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cli {
    /// The program is missing or could not be started.
    Missing,
    /// It ran, but the Shell does not know the extension (not discovered since it was written).
    NotLoaded,
    State(String),
}

/// Everything `detect` decides from.
struct Facts<'a> {
    /// The `version` in the installed `metadata.json`; `None` when nothing is installed.
    disk_version: Option<u32>,
    enabled: &'a [String],
    globally_disabled: bool,
    cli: Cli,
    peer: Peer,
    /// GNOME Shell started after the installed files were last written.
    shell_restarted: bool,
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
    let data = super::apps::data_home().ok_or("Your home folder could not be found.")?;
    Ok(data.join("gnome-shell/extensions").join(UUID))
}

/// The unescaped text after a backslash in a GVariant string: `\\ \' \" \n \t \r \a \b \f \v`
/// and `\uXXXX` / `\UXXXXXXXX`.
fn unescape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<char, String> {
    let bad = || "Invalid GNOME extension list.".to_string();
    let hex = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>, digits: usize| {
        let text: String = chars.by_ref().take(digits).collect();
        (text.len() == digits)
            .then(|| u32::from_str_radix(&text, 16).ok().and_then(char::from_u32))
            .flatten()
            .ok_or_else(bad)
    };
    Ok(match chars.next().ok_or_else(bad)? {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        'a' => '\u{7}',
        'b' => '\u{8}',
        'f' => '\u{c}',
        'v' => '\u{b}',
        'u' => hex(chars, 4)?,
        'U' => hex(chars, 8)?,
        other @ ('\\' | '\'' | '"') => other,
        _ => return Err(bad()),
    })
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
                value.push(unescape(&mut chars)?);
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
        .map(|value| {
            let mut out = String::from("'");
            for c in value.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '\'' => out.push_str("\\'"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    c if c.is_control() => out.push_str(&format!("\\u{:04x}", u32::from(c))),
                    c => out.push(c),
                }
            }
            out.push('\'');
            out
        })
        .collect();
    format!("[{}]", quoted.join(", "))
}

/// `enabled` with `UUID` added, or `None` when it is already there (nothing to write).
fn with_extension(enabled: &[String]) -> Option<Vec<String>> {
    if enabled.iter().any(|s| s == UUID) {
        return None;
    }
    let mut merged = enabled.to_vec();
    merged.push(UUID.into());
    Some(merged)
}

fn detect(facts: &Facts<'_>) -> FocusStatus {
    match &facts.peer {
        Peer::Shell { version, .. } if compatible(*version) => {
            return if facts.shell_restarted {
                FocusStatus::Working
            } else {
                FocusStatus::NeedsLogout
            }
        }
        Peer::Foreign => return FocusStatus::Unavailable,
        _ => {}
    }
    let running_stale = matches!(facts.peer, Peer::Shell { .. });
    let Some(disk) = facts.disk_version else {
        // Running from files that are gone: nothing to update, log out to unload it.
        return if running_stale {
            FocusStatus::NeedsLogout
        } else {
            FocusStatus::NotInstalled
        };
    };
    if disk < VERSION {
        return FocusStatus::UpdateAvailable;
    }
    if running_stale {
        // The files are current, the running copy is not: only a new Shell loads them.
        return if facts.shell_restarted {
            FocusStatus::Unavailable
        } else {
            FocusStatus::NeedsLogout
        };
    }
    if facts.globally_disabled || !facts.enabled.iter().any(|s| s == UUID) {
        return FocusStatus::Disabled;
    }
    match &facts.cli {
        Cli::Missing => FocusStatus::CliMissing,
        // Shell never saw the files. Right before the first log-out; after one it is a failure,
        // and asking for another log-out would loop forever.
        Cli::NotLoaded if facts.shell_restarted => FocusStatus::Unavailable,
        Cli::NotLoaded => FocusStatus::NeedsLogout,
        Cli::State(state) => match state.as_str() {
            "OUT OF DATE" => FocusStatus::Unsupported,
            "ERROR" | "ACTIVE" | "ENABLED" => FocusStatus::Unavailable,
            // Loaded but disabled, even if the preference still contains it (session-wide toggles).
            _ => FocusStatus::Disabled,
        },
    }
}

fn cli_info() -> Cli {
    let result = Command::new("gnome-extensions")
        .args(["info", UUID])
        .env("LC_ALL", "C")
        .output();
    let Ok(result) = result else {
        return Cli::Missing;
    };
    let text = String::from_utf8_lossy(&result.stdout);
    text.lines()
        .find_map(|line| line.trim().strip_prefix("State:"))
        .map(str::trim)
        .filter(|_| result.status.success())
        .map_or(Cli::NotLoaded, |state| Cli::State(state.to_string()))
}

/// The `version` recorded in the installed `metadata.json`, and when it was written.
fn installed() -> Option<(u32, SystemTime)> {
    let file = extension_dir().ok()?.join("metadata.json");
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).ok()?).ok()?;
    let version = value["version"]
        .as_u64()
        .map_or(0, |v| u32::try_from(v).unwrap_or(0));
    Some((version, std::fs::metadata(&file).ok()?.modified().ok()?))
}

fn trusted_executable_matches(dir: &Path) -> bool {
    let expected = std::env::current_exe()
        .ok()
        .and_then(|path| path.canonicalize().ok());
    let trusted = std::fs::read(dir.join("trusted.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    match (expected, trusted) {
        (Some(expected), Some(trusted)) => trusted["executable"].as_str() == expected.to_str(),
        _ => false,
    }
}

pub fn status() -> FocusStatus {
    let state = read_status();
    tracing::info!(?state, "GNOME app profiles status");
    state
}

fn read_status() -> FocusStatus {
    if session_kind() != SessionKind::Wayland || !is_gnome() {
        tracing::info!(session = ?session_kind(), desktop = ?std::env::var("XDG_CURRENT_DESKTOP").ok(), "GNOME app profiles not applicable");
        return FocusStatus::NotApplicable;
    }
    match output("gnome-shell", &["--version"]) {
        Ok(version) => {
            tracing::info!(version = version.trim(), "GNOME Shell version detected");
            if !supported(&version) {
                return FocusStatus::Unsupported;
            }
        }
        Err(error) => {
            tracing::info!(%error, "GNOME Shell version check failed");
            return FocusStatus::Unavailable;
        }
    }
    if installed().is_some() && !extension_dir().is_ok_and(|dir| trusted_executable_matches(&dir)) {
        return FocusStatus::UpdateAvailable;
    }
    let conn = super::gnome_focus::connection().ok();
    let peer = conn
        .as_ref()
        .and_then(|conn| super::gnome_focus::peer(conn).ok())
        .unwrap_or(Peer::Absent);
    let working = matches!(&peer, Peer::Shell { version, .. } if compatible(*version));
    let enabled = match output(
        "gsettings",
        &["get", "org.gnome.shell", "enabled-extensions"],
    )
    .and_then(|text| extension_list(&text))
    {
        Ok(enabled) => enabled,
        Err(error) => {
            tracing::info!(%error, "GNOME enabled extensions check failed");
            return if working {
                FocusStatus::Working
            } else {
                FocusStatus::Unavailable
            };
        }
    };
    let globally_disabled = output(
        "gsettings",
        &["get", "org.gnome.shell", "disable-user-extensions"],
    )
    .is_ok_and(|text| text.trim() == "true");
    let disk = installed();
    let shell_restarted = disk.is_some_and(|(_, written)| {
        conn.as_ref()
            .and_then(super::gnome_focus::shell_pid)
            .and_then(super::proc::start_time)
            .is_some_and(|started| started > written)
    });
    let cli = if working { Cli::NotLoaded } else { cli_info() };
    tracing::info!(directory = ?extension_dir(), disk_version = ?disk.map(|(version, _)| version), enabled = enabled.iter().any(|uuid| uuid == UUID), globally_disabled, ?cli, ?peer, shell_restarted, "GNOME app profiles detection");
    detect(&Facts {
        disk_version: disk.map(|(version, _)| version),
        enabled: &enabled,
        globally_disabled,
        cli,
        peer,
        shell_restarted,
    })
}

fn write_extension(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not create extension folder: {e}"))?;
    for (name, text) in FILES {
        std::fs::write(dir.join(name), text).map_err(|e| format!("Could not write {name}: {e}"))?;
    }
    // Record a canonical executable, never a caller-provided path or a sandbox proxy identity.
    let executable = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|e| format!("Could not identify Kivori executable: {e}"))?;
    let trusted = serde_json::to_vec(&serde_json::json!({ "executable": executable }))
        .map_err(|e| e.to_string())?;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let temporary = dir.join("trusted.json.tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(&trusted).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, dir.join("trusted.json")).map_err(|e| e.to_string())?;
    Ok(())
}

/// Add the extension to `enabled-extensions` by reading the list again right before writing it,
/// so a change made a moment ago (the Extensions app) is merged rather than overwritten.
fn persist_enabled() -> Result<(), String> {
    let enabled = extension_list(&output(
        "gsettings",
        &["get", "org.gnome.shell", "enabled-extensions"],
    )?)?;
    let Some(merged) = with_extension(&enabled) else {
        return Ok(());
    };
    output(
        "gsettings",
        &[
            "set",
            "org.gnome.shell",
            "enabled-extensions",
            &variant_list(&merged),
        ],
    )
    .map(|_| ())
}

/// Installs, or updates an older copy. Either way GNOME Shell loads JavaScript only at login on
/// Wayland, so a log-out finishes it.
pub fn install() -> Result<FocusStatus, String> {
    tracing::info!("GNOME app profiles install requested");
    let result = install_extension();
    match &result {
        Ok(state) => tracing::info!(?state, "GNOME app profiles install completed"),
        Err(error) => tracing::info!(%error, "GNOME app profiles install failed"),
    }
    result
}

fn install_extension() -> Result<FocusStatus, String> {
    match status() {
        FocusStatus::NotApplicable => {
            return Err("App profiles require a GNOME Wayland session.".into())
        }
        FocusStatus::Unsupported => {
            return Err(
                "This extension supports GNOME 46–50. Update Kivori for your GNOME version.".into(),
            )
        }

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
    let directory = extension_dir()?;
    write_extension(&directory)?;
    tracing::info!(?directory, "GNOME app profiles extension files installed");
    // The Shell's own enable is atomic. A copy it has not discovered yet cannot be enabled that
    // way ("does not exist" until the next login), so that one is persisted through the setting.
    let enable_result = output("gnome-extensions", &["enable", UUID]);
    if let Err(error) = &enable_result {
        tracing::info!(%error, "GNOME Shell has not enabled the extension; persisting enable for next login");
        persist_enabled()?;
    }
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
    let mut child = Command::new("gnome-session-quit")
        .arg("--logout")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    // Reap it when it exits (the dialog may stay open a while) so it never lingers as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(version: Option<u32>) -> Peer {
        Peer::Shell {
            owner: ":1.5".into(),
            version,
        }
    }

    fn facts<'a>(enabled: &'a [String]) -> Facts<'a> {
        Facts {
            disk_version: Some(VERSION),
            enabled,
            globally_disabled: false,
            cli: Cli::NotLoaded,
            peer: Peer::Absent,
            shell_restarted: false,
        }
    }

    #[test]
    fn even_a_compatible_running_extension_needs_logout_after_reinstall() {
        let enabled = vec![UUID.to_string()];
        let mut f = facts(&enabled);
        f.peer = shell(Some(VERSION));
        assert_eq!(detect(&f), FocusStatus::NeedsLogout);
        f.shell_restarted = true;
        assert_eq!(detect(&f), FocusStatus::Working);
    }

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
            "line\nbreak\ttab".into(),
            "bell\u{7}é".into(),
        ];
        assert_eq!(extension_list(&variant_list(&values)).unwrap(), values);
        assert!(extension_list("garbage").is_err());
        assert!(extension_list("['unfinished]").is_err());
        assert!(
            extension_list("['bad\\q']").is_err(),
            "an unknown escape is refused"
        );
    }

    #[test]
    fn gvariant_escapes_decode_instead_of_becoming_literal_characters() {
        // `\n` is a newline and `\u00e9` is é: the old parser kept the letters n and u00e9.
        assert_eq!(
            extension_list(r"['a\nb', 'caf\u00e9', 'x\U0001f600', 'q\\r']").unwrap(),
            ["a\nb", "café", "x\u{1f600}", "q\\r"]
        );
        assert!(extension_list(r"['\u12']").is_err());
        assert!(extension_list(r"['\ud800zz']").is_err());
    }

    #[test]
    fn enabling_merges_into_the_current_list_and_skips_a_no_op() {
        let current = vec!["a@x".to_string(), "b@y".to_string()];
        assert_eq!(
            with_extension(&current).unwrap(),
            ["a@x", "b@y", UUID],
            "every other extension survives, in order"
        );
        assert_eq!(with_extension(&["a@x".into(), UUID.into()]), None);
        assert_eq!(with_extension(&[]).unwrap(), [UUID]);
        assert_eq!(
            variant_list(&with_extension(&[]).unwrap()),
            format!("['{UUID}']")
        );
        assert_ne!(with_extension(&[format!("other-{UUID}")]), None);
    }

    #[test]
    fn states_use_exact_preferences_and_cli_output() {
        let enabled = vec![UUID.to_string()];
        let none: Vec<String> = Vec::new();
        let status = |edit: &dyn Fn(&mut Facts<'_>), list: &[String]| {
            let mut f = facts(list);
            edit(&mut f);
            detect(&f)
        };
        assert_eq!(
            status(&|f| f.disk_version = None, &none),
            FocusStatus::NotInstalled
        );
        assert_eq!(status(&|_| {}, &enabled), FocusStatus::NeedsLogout);
        assert_eq!(
            status(&|f| f.cli = Cli::State("DISABLED".into()), &none),
            FocusStatus::Disabled
        );
        assert_eq!(
            status(
                &|f| {
                    f.globally_disabled = true;
                    f.cli = Cli::State("INITIALIZED".into());
                },
                &enabled
            ),
            FocusStatus::Disabled
        );
        assert_eq!(
            status(&|f| f.cli = Cli::State("ERROR".into()), &enabled),
            FocusStatus::Unavailable
        );
        assert_eq!(
            status(&|f| f.cli = Cli::State("OUT OF DATE".into()), &enabled),
            FocusStatus::Unsupported
        );
        assert_eq!(
            status(&|f| f.peer = shell(Some(VERSION)), &enabled),
            FocusStatus::NeedsLogout
        );
        assert_eq!(
            status(&|_| {}, &[format!("other-{UUID}")]),
            FocusStatus::Disabled
        );
    }

    #[test]
    fn a_missing_gnome_extensions_tool_is_its_own_state() {
        let enabled = vec![UUID.to_string()];
        let mut f = facts(&enabled);
        f.cli = Cli::Missing;
        assert_eq!(detect(&f), FocusStatus::CliMissing);
        // Not a log-out request, so it cannot loop: an unreadable tool never claims "log out".
        assert_ne!(detect(&f), FocusStatus::NeedsLogout);
    }

    #[test]
    fn a_log_out_that_already_happened_is_not_asked_for_again() {
        let enabled = vec![UUID.to_string()];
        let mut f = facts(&enabled);
        assert_eq!(
            detect(&f),
            FocusStatus::NeedsLogout,
            "Shell never saw the files"
        );
        f.shell_restarted = true;
        assert_eq!(
            detect(&f),
            FocusStatus::Unavailable,
            "Shell restarted and still has none"
        );
    }

    #[test]
    fn an_outdated_extension_is_updated_then_needs_a_log_out() {
        let enabled = vec![UUID.to_string()];
        // Old files on disk (v1), an old copy running: update.
        let mut f = facts(&enabled);
        f.disk_version = Some(VERSION - 1);
        f.peer = shell(None);
        assert_eq!(detect(&f), FocusStatus::UpdateAvailable);
        // Old files, nothing running: still an update, never the stale code.
        f.peer = Peer::Absent;
        assert_eq!(detect(&f), FocusStatus::UpdateAvailable);
        // New files written, the old copy still running in this Shell: log out to load them.
        f.disk_version = Some(VERSION);
        f.peer = shell(Some(VERSION - 1));
        assert_eq!(detect(&f), FocusStatus::NeedsLogout);
        f.peer = shell(None);
        assert_eq!(detect(&f), FocusStatus::NeedsLogout);
        // A Shell that restarted after the write and still runs old code is a failure.
        f.shell_restarted = true;
        assert_eq!(detect(&f), FocusStatus::Unavailable);
        // Current copy running: working. A newer one is trusted too.
        f.peer = shell(Some(VERSION + 1));
        assert_eq!(detect(&f), FocusStatus::Working);
    }

    #[test]
    fn a_stranger_owning_the_name_is_never_working() {
        let enabled = vec![UUID.to_string()];
        let mut f = facts(&enabled);
        f.peer = Peer::Foreign;
        assert_eq!(detect(&f), FocusStatus::Unavailable);
    }

    #[test]
    fn install_writes_embedded_files_under_a_temporary_home() {
        let home = tempfile::tempdir().unwrap();
        let dir = home
            .path()
            .join(".local/share/gnome-shell/extensions")
            .join(UUID);
        assert!(!trusted_executable_matches(&dir));
        write_extension(&dir).unwrap();
        assert!(trusted_executable_matches(&dir));
        let metadata: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("metadata.json")).unwrap())
                .unwrap();
        assert_eq!(metadata["uuid"], UUID);
        assert_eq!(metadata["version"], VERSION);
        assert_eq!(
            metadata["shell-version"],
            serde_json::json!(["46", "47", "48", "49", "50"])
        );
        for (name, text) in FILES {
            assert_eq!(
                std::fs::read_to_string(dir.join(name)).unwrap(),
                text,
                "{name}"
            );
        }
        assert!(
            EXTENSION.contains("./bridge.js"),
            "extension.js imports the bridge it ships with"
        );
        let trusted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("trusted.json")).unwrap()).unwrap();
        assert_eq!(
            trusted["executable"],
            std::env::current_exe()
                .unwrap()
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
        );
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(dir.join("trusted.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        std::fs::write(dir.join("trusted.json"), "old executable").unwrap();
        assert!(!trusted_executable_matches(&dir));
        write_extension(&dir).unwrap(); // Repeat install refreshes the trusted executable.
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &std::fs::read(dir.join("trusted.json")).unwrap()
            )
            .unwrap(),
            trusted
        );
    }

    #[test]
    fn an_update_overwrites_an_older_installed_copy() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("ext");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("metadata.json"), r#"{"uuid":"x","version":1}"#).unwrap();
        std::fs::write(dir.join("extension.js"), "old").unwrap();
        write_extension(&dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("extension.js")).unwrap(),
            EXTENSION
        );
        assert!(dir.join("bridge.js").is_file());
    }
}
