//! Launching an application: Execution Confirmed once the OS accepted the launch.
//!
//! No shell is ever involved: the target is passed as one argument, never interpolated into a
//! command line.

use super::ActionError;

/// Longest accepted target, in bytes.
const MAX_TARGET: usize = 1024;

/// Checks a launch target at the trust boundary: non-empty, bounded, no NUL.
///
/// # Errors
/// [`ActionError::Failed`] naming what is wrong.
pub fn validate_target(target: &str) -> Result<&str, ActionError> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err(ActionError::Failed("no application given".into()));
    }
    if trimmed.len() > MAX_TARGET || trimmed.contains('\0') {
        return Err(ActionError::Failed("not a valid application".into()));
    }
    Ok(trimmed)
}

/// Starts `target`.
///
/// - macOS: an application name or `.app` path, opened with `/usr/bin/open -a`. `open` exits
///   non-zero when no such application exists, so success means Launch Services accepted it.
/// - Windows: an executable name or path, spawned directly (searched on `PATH` like a Run box
///   would). Success means the process was created.
///
/// # Errors
/// [`ActionError::Failed`] if the target is invalid or the OS refused it;
/// [`ActionError::NotImplemented`] on other targets.
pub fn launch(target: &str) -> Result<(), ActionError> {
    let target = validate_target(target)?;
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("/usr/bin/open")
            .arg("-a")
            .arg(target)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| ActionError::Failed(e.kind().to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(ActionError::Failed("application not found".into()))
        }
    }
    #[cfg(windows)]
    {
        std::process::Command::new(target)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(drop)
            .map_err(|e| ActionError::Failed(e.kind().to_string()))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = target;
        Err(ActionError::NotImplemented {
            target: std::env::consts::OS,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_validated_before_any_process_starts() {
        assert!(validate_target("   ").is_err());
        assert!(validate_target("a\0b").is_err());
        assert!(validate_target(&"x".repeat(MAX_TARGET + 1)).is_err());
        assert_eq!(validate_target("  Calculator "), Ok("Calculator"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_missing_application_is_a_known_failure() {
        assert_eq!(
            launch("Kivori Definitely Not An App 7f3a"),
            Err(ActionError::Failed("application not found".into()))
        );
    }
}
