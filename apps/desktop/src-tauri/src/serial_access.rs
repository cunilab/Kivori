//! Permission to open the Kivori USB serial port, which only Linux can refuse.
//!
//! A fresh Linux install (or an AppImage, which cannot install files outside itself) has no udev
//! rule granting the logged-in user access to the device, so opening it fails with "permission
//! denied". That is a known, fixable state, not a generic error: the device thread records it here,
//! the webview shows "Kivori needs permission to use the USB device", and one button installs the
//! rule through `pkexec` (polkit's graphical password prompt). Where `pkexec` is missing, the same
//! rule is shown as one command to copy.
//!
//! The rule is the packaging file, embedded at build time, so the `.deb` and this fix install the
//! same content. Nothing here is built from user input: the script `pkexec` runs is a constant and
//! the rule travels on stdin.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the last attempt to open a Kivori port was refused for lack of permission.
static DENIED: AtomicBool = AtomicBool::new(false);

/// Records whether opening the port was refused for permission. Returns whether that changed, so
/// the caller announces only edges.
pub fn set_denied(denied: bool) -> bool {
    DENIED.swap(denied, Ordering::SeqCst) != denied
}

/// Whether the port is currently refused for lack of permission.
#[must_use]
pub fn denied() -> bool {
    DENIED.load(Ordering::SeqCst)
}

/// The udev rule the packaging ships (`apps/desktop/src-tauri/linux/60-kivori.rules`).
pub const UDEV_RULE: &str = include_str!("../linux/60-kivori.rules");

/// Where the rule belongs.
pub const RULE_PATH: &str = "/etc/udev/rules.d/60-kivori.rules";

/// The one script `pkexec` runs as root. It reads the rule from stdin, installs it, then makes udev
/// apply it to the device that is already plugged in. A constant: nothing is interpolated into it.
const INSTALL_SCRIPT: &str = "umask 022 && tee /etc/udev/rules.d/60-kivori.rules >/dev/null \
     && udevadm control --reload-rules && udevadm trigger --subsystem-match=tty";

/// The absolute path `pkexec` is run from (never searched on `PATH`).
const PKEXEC: &str = "/usr/bin/pkexec";

/// Whether the one-click fix can run here: Linux with `pkexec` installed.
#[must_use]
pub fn can_fix() -> bool {
    cfg!(target_os = "linux") && std::path::Path::new(PKEXEC).is_file()
}

/// `text` as one single-quoted shell word, safe to paste into a shell.
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// The manual fix as one command to copy, for machines without `pkexec`: the rule's active lines
/// written with `sudo tee`, then udev reloaded. `None` off Linux.
#[must_use]
pub fn manual_command() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let lines: Vec<String> = UDEV_RULE
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(sh_quote)
        .collect();
    Some(format!(
        "printf '%s\\n' {} | sudo tee {RULE_PATH} >/dev/null \
         && sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=tty",
        lines.join(" ")
    ))
}

/// How the one-click install ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    /// `pkexec` is not installed.
    Unavailable,
    /// The user closed the password prompt, or is not allowed to administer this computer.
    NotAuthorized,
    /// The install ran and failed, or could not be started.
    Failed(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => write!(f, "The password prompt (pkexec) isn't installed"),
            Self::NotAuthorized => write!(f, "Permission wasn't granted"),
            Self::Failed(reason) => write!(f, "Couldn't set up USB access: {reason}"),
        }
    }
}

impl std::error::Error for InstallError {}

/// What `pkexec` exiting with `code` means: 0 is success, 126 (prompt dismissed) and 127 (not
/// authorized) are a refusal, a signal or anything else is a failure.
pub(crate) fn classify_exit(code: Option<i32>) -> Result<(), InstallError> {
    match code {
        Some(0) => Ok(()),
        Some(126 | 127) => Err(InstallError::NotAuthorized),
        Some(code) => Err(InstallError::Failed(format!("exit code {code}"))),
        None => Err(InstallError::Failed("it was stopped".to_string())),
    }
}

/// Installs the rule as root through one `pkexec` call and waits for the answer. Blocks while the
/// user types their password, so call it off the UI thread.
///
/// # Errors
/// See [`InstallError`].
pub fn install_rule() -> Result<(), InstallError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    if !can_fix() {
        return Err(InstallError::Unavailable);
    }
    let mut child = Command::new(PKEXEC)
        .args(["/bin/sh", "-c", INSTALL_SCRIPT])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| InstallError::Failed(e.kind().to_string()))?;
    // The rule goes in on stdin and the pipe is closed, so `tee` sees the end of its input.
    let written = child
        .stdin
        .take()
        .map_or(Ok(()), |mut stdin| stdin.write_all(UDEV_RULE.as_bytes()));
    let status = child
        .wait()
        .map_err(|e| InstallError::Failed(e.kind().to_string()))?;
    classify_exit(status.code())?;
    written.map_err(|e| InstallError::Failed(e.kind().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_rule_grants_the_kivori_port_to_the_logged_in_user() {
        assert!(UDEV_RULE.contains("2e8a") && UDEV_RULE.contains("000a"));
        assert!(UDEV_RULE.contains("uaccess"));
        assert!(!UDEV_RULE.contains("0666"), "never world-writable");
    }

    #[test]
    fn denied_reports_only_edges() {
        // One test owns the global so parallel tests cannot interleave.
        assert!(set_denied(true));
        assert!(!set_denied(true));
        assert!(denied());
        assert!(set_denied(false));
        assert!(!denied());
    }

    #[test]
    fn the_install_script_is_constant_and_carries_no_rule_text() {
        // The rule travels on stdin, so no rule content can be parsed as shell.
        for needle in ["2e8a", "uaccess", "ATTRS", "SUBSYSTEM=="] {
            assert!(!INSTALL_SCRIPT.contains(needle), "{needle}");
        }
        assert!(INSTALL_SCRIPT.contains(RULE_PATH));
        assert!(INSTALL_SCRIPT.contains("udevadm control --reload-rules"));
        assert!(INSTALL_SCRIPT.contains("udevadm trigger --subsystem-match=tty"));
    }

    #[test]
    fn pkexec_exit_codes_mean_refused_or_failed() {
        assert_eq!(classify_exit(Some(0)), Ok(()));
        assert_eq!(classify_exit(Some(126)), Err(InstallError::NotAuthorized));
        assert_eq!(classify_exit(Some(127)), Err(InstallError::NotAuthorized));
        assert!(matches!(
            classify_exit(Some(1)),
            Err(InstallError::Failed(_))
        ));
        assert!(matches!(classify_exit(None), Err(InstallError::Failed(_))));
    }

    #[test]
    fn shell_quoting_survives_a_quote_inside_the_text() {
        assert_eq!(sh_quote("a b"), "'a b'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        assert_eq!(sh_quote(r#"x=="y""#), r#"'x=="y"'"#);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_manual_command_writes_the_active_rule_lines_with_sudo() {
        let command = manual_command().expect("Linux has one");
        assert!(command.starts_with("printf '%s\\n' "));
        assert!(command.contains("sudo tee /etc/udev/rules.d/60-kivori.rules"));
        assert!(command.contains("sudo udevadm control --reload-rules"));
        assert!(command.contains("ATTRS{idVendor}==\"2e8a\""));
        // Comment lines are not part of the pasted rule.
        assert!(!command.contains("plugdev is a fallback"));
    }
}
