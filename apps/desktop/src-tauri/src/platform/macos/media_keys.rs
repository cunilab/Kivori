//! Media keys on macOS without the Accessibility permission. Posting a media key event needs it,
//! but the bundled MediaRemote adapter (Apple's own `/usr/bin/perl`, entitled for MediaRemote)
//! can send the same play/pause and track commands to the now-playing app directly.
//!
//! So media keys go through the adapter first. Only when it is unavailable or reports failure does
//! the key event (and with it the Accessibility permission) come into play. Shortcuts always use the
//! key event: there is no other way to type into another app.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::media::{adapter_dir, Reaped, PERL, SCRIPT};
use crate::platform::{ActionError, InputGuard, InputSynth, MediaKey, Shortcut};

/// `perl` loading the adapter takes well under a second; this is only a ceiling.
const SEND_TIMEOUT: Duration = Duration::from_secs(2);

/// How a `send` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sent {
    Yes,
    /// Could not run, or ran and reported failure: nothing was sent, so another way may be tried.
    No,
    /// Still running at the deadline: it may yet have sent, so another way must not be tried.
    Unknown,
}

/// Sends one media command to the now-playing app. Injected so tests never run `perl`.
pub(super) trait AdapterSend: Send + Sync {
    fn send(&self, key: MediaKey, timeout: Duration) -> Sent;
}

/// The MRCommand id the adapter's `send` takes (`kMRATogglePlayPause`, `kMRANextTrack`,
/// `kMRAPreviousTrack` in the vendored header).
const fn command_id(key: MediaKey) -> u8 {
    match key {
        MediaKey::PlayPause => 2,
        MediaKey::Next => 4,
        MediaKey::Previous => 5,
    }
}

/// The arguments after `perl`: script, framework, `send`, command id. Plain arguments, never a
/// shell string.
fn send_args(dir: &Path, key: MediaKey) -> [std::ffi::OsString; 4] {
    [
        dir.join(SCRIPT).into_os_string(),
        dir.join("MediaRemoteAdapter.framework").into_os_string(),
        "send".into(),
        command_id(key).to_string().into(),
    ]
}

/// Runs the real adapter script from the app bundle (or this build's output directory in dev).
struct PerlAdapter;

impl AdapterSend for PerlAdapter {
    fn send(&self, key: MediaKey, timeout: Duration) -> Sent {
        let Some(dir) = adapter_dir() else {
            return Sent::No;
        };
        let mut command = Command::new(PERL);
        // A clean environment: no PERL5LIB/PERL5OPT from the user's session reaches the helper.
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(send_args(&dir, key))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let Ok(child) = command.spawn() else {
            return Sent::No;
        };
        let mut child = Reaped(child);
        let deadline = Instant::now() + timeout;
        loop {
            match child.0.try_wait() {
                Ok(Some(status)) => {
                    return if status.success() {
                        Sent::Yes
                    } else {
                        Sent::No
                    }
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => return Sent::Unknown,
            }
        }
    }
}

/// Key synthesis for macOS: media keys through the adapter first, everything else (and the media
/// fallback) through `fallback`.
pub struct MacInputSynth {
    fallback: Arc<dyn InputSynth>,
    adapter: Arc<dyn AdapterSend>,
}

impl MacInputSynth {
    #[must_use]
    pub fn new(fallback: Arc<dyn InputSynth>) -> Self {
        Self {
            fallback,
            adapter: Arc::new(PerlAdapter),
        }
    }
}

impl InputSynth for MacInputSynth {
    fn send_media_key(&self, key: MediaKey, guard: InputGuard) -> Result<(), ActionError> {
        guard.check()?;
        let timeout = guard.deadline.map_or(SEND_TIMEOUT, |deadline| {
            SEND_TIMEOUT.min(deadline.saturating_duration_since(Instant::now()))
        });
        match self.adapter.send(key, timeout) {
            Sent::Yes => Ok(()),
            Sent::No => self.fallback.send_media_key(key, guard),
            Sent::Unknown => Err(ActionError::Uncertain),
        }
    }

    fn send_shortcut(&self, shortcut: &Shortcut, guard: InputGuard) -> Result<(), ActionError> {
        self.fallback.send_shortcut(shortcut, guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct FakeAdapter {
        outcome: Sent,
        seen: Mutex<Vec<(MediaKey, Duration)>>,
    }

    impl AdapterSend for FakeAdapter {
        fn send(&self, key: MediaKey, timeout: Duration) -> Sent {
            self.seen.lock().unwrap().push((key, timeout));
            self.outcome
        }
    }

    #[derive(Default)]
    struct FakeKeys {
        media: Mutex<Vec<MediaKey>>,
        shortcuts: Mutex<usize>,
        result: Mutex<Option<ActionError>>,
    }

    impl InputSynth for FakeKeys {
        fn send_media_key(&self, key: MediaKey, _: InputGuard) -> Result<(), ActionError> {
            self.media.lock().unwrap().push(key);
            self.result.lock().unwrap().clone().map_or(Ok(()), Err)
        }
        fn send_shortcut(&self, _: &Shortcut, _: InputGuard) -> Result<(), ActionError> {
            *self.shortcuts.lock().unwrap() += 1;
            Ok(())
        }
    }

    fn guard() -> InputGuard {
        InputGuard {
            deadline: None,
            current: Arc::new(|| true),
        }
    }

    fn synth(outcome: Sent) -> (MacInputSynth, Arc<FakeAdapter>, Arc<FakeKeys>) {
        let adapter = Arc::new(FakeAdapter {
            outcome,
            seen: Mutex::default(),
        });
        let keys = Arc::new(FakeKeys::default());
        let synth = MacInputSynth {
            fallback: keys.clone(),
            adapter: adapter.clone(),
        };
        (synth, adapter, keys)
    }

    #[test]
    fn each_key_maps_to_its_adapter_command() {
        let dir = PathBuf::from("/app/Contents/Resources/mediaremote-adapter");
        for (key, id) in [
            (MediaKey::PlayPause, "2"),
            (MediaKey::Next, "4"),
            (MediaKey::Previous, "5"),
        ] {
            let args = send_args(&dir, key);
            assert_eq!(
                args.map(|a| a.to_string_lossy().into_owned()),
                [
                    "/app/Contents/Resources/mediaremote-adapter/mediaremote-adapter.pl",
                    "/app/Contents/Resources/mediaremote-adapter/MediaRemoteAdapter.framework",
                    "send",
                    id
                ]
            );
        }
    }

    #[test]
    fn media_keys_use_the_adapter_and_never_the_key_event() {
        let (synth, adapter, keys) = synth(Sent::Yes);
        for key in [MediaKey::PlayPause, MediaKey::Previous, MediaKey::Next] {
            assert_eq!(synth.send_media_key(key, guard()), Ok(()));
        }
        let sent: Vec<_> = adapter.seen.lock().unwrap().iter().map(|s| s.0).collect();
        assert_eq!(
            sent,
            [MediaKey::PlayPause, MediaKey::Previous, MediaKey::Next]
        );
        assert!(keys.media.lock().unwrap().is_empty());
    }

    #[test]
    fn a_failed_adapter_falls_back_to_the_key_event_and_its_permission() {
        let (synth, _, keys) = synth(Sent::No);
        assert_eq!(synth.send_media_key(MediaKey::Next, guard()), Ok(()));
        assert_eq!(*keys.media.lock().unwrap(), [MediaKey::Next]);
        *keys.result.lock().unwrap() = Some(ActionError::PermissionRequired);
        assert_eq!(
            synth.send_media_key(MediaKey::Next, guard()),
            Err(ActionError::PermissionRequired)
        );
    }

    #[test]
    fn a_slow_adapter_is_uncertain_and_never_doubled_by_the_key_event() {
        let (synth, _, keys) = synth(Sent::Unknown);
        assert_eq!(
            synth.send_media_key(MediaKey::PlayPause, guard()),
            Err(ActionError::Uncertain)
        );
        assert!(keys.media.lock().unwrap().is_empty());
    }

    #[test]
    fn the_wait_is_bounded_by_the_ceiling_and_the_guard_deadline() {
        let (synth, adapter, _) = synth(Sent::Yes);
        synth.send_media_key(MediaKey::Next, guard()).unwrap();
        let soon = InputGuard {
            deadline: Some(Instant::now() + Duration::from_millis(300)),
            current: Arc::new(|| true),
        };
        synth.send_media_key(MediaKey::Next, soon).unwrap();
        let seen = adapter.seen.lock().unwrap();
        assert_eq!(seen[0].1, SEND_TIMEOUT);
        assert!(seen[1].1 <= Duration::from_millis(300));
    }

    #[test]
    fn a_stale_request_sends_nothing() {
        let (synth, adapter, keys) = synth(Sent::Yes);
        let stale = InputGuard {
            deadline: None,
            current: Arc::new(|| false),
        };
        assert!(synth.send_media_key(MediaKey::Next, stale).is_err());
        assert!(adapter.seen.lock().unwrap().is_empty());
        assert!(keys.media.lock().unwrap().is_empty());
    }

    #[test]
    fn shortcuts_always_use_the_key_event() {
        let (synth, adapter, keys) = synth(Sent::Yes);
        let shortcut: Shortcut = "Ctrl+M".parse().unwrap();
        assert_eq!(synth.send_shortcut(&shortcut, guard()), Ok(()));
        assert_eq!(*keys.shortcuts.lock().unwrap(), 1);
        assert!(adapter.seen.lock().unwrap().is_empty());
    }
}
