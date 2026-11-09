//! Process identity from `/proc`, shared by the focus observer and the running-apps list.

use std::path::Path;

/// The file name of an executable path, without the ` (deleted)` suffix the kernel appends when the
/// binary was replaced on disk while running (an upgrade in progress). `None` for an empty name.
pub(super) fn exe_basename(path: &str) -> Option<String> {
    let path = path.strip_suffix(" (deleted)").unwrap_or(path);
    let file = path.rsplit('/').next()?.trim();
    (!file.is_empty()).then(|| file.to_string())
}

/// The executable file name behind `pid`: `/proc/<pid>/exe`, else `/proc/<pid>/comm` (which the
/// kernel truncates to 15 bytes) when the link is unreadable (another user's process, or a
/// kernel thread). `None` when neither can be read.
pub(super) fn exe_name(pid: u32) -> Option<String> {
    exe_name_in(Path::new("/proc"), pid)
}

fn exe_name_in(proc: &Path, pid: u32) -> Option<String> {
    let dir = proc.join(pid.to_string());
    if let Some(name) = std::fs::read_link(dir.join("exe"))
        .ok()
        .and_then(|target| exe_basename(&target.to_string_lossy()))
    {
        return Some(name);
    }
    let comm = std::fs::read_to_string(dir.join("comm")).ok()?;
    let comm = comm.trim();
    (!comm.is_empty()).then(|| comm.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_basename_drops_the_directory_and_the_deleted_marker() {
        assert_eq!(exe_basename("/usr/bin/firefox"), Some("firefox".into()));
        assert_eq!(
            exe_basename("/opt/app/bin/App (deleted)"),
            Some("App".into())
        );
        assert_eq!(exe_basename("spotify"), Some("spotify".into()));
        assert_eq!(exe_basename("/usr/bin/"), None);
        assert_eq!(exe_basename(""), None);
    }

    #[test]
    fn comm_is_the_fallback_when_exe_cannot_be_read() {
        let proc = tempfile::tempdir().unwrap();
        let dir = proc.path().join("42");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("comm"), "kworker/0:1\n").unwrap();
        assert_eq!(exe_name_in(proc.path(), 42), Some("kworker/0:1".into()));
        std::os::unix::fs::symlink("/usr/bin/tool (deleted)", dir.join("exe")).unwrap();
        assert_eq!(exe_name_in(proc.path(), 42), Some("tool".into()));
        assert_eq!(exe_name_in(proc.path(), 43), None);
    }

    #[test]
    fn this_process_resolves_to_its_own_name() {
        let me = exe_name(std::process::id()).expect("own exe");
        assert!(!me.is_empty());
    }
}
