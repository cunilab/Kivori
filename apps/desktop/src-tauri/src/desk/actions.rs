//! Discrete desk actions (M1): what Press and Hold do, how each outcome is classified, and the
//! worker thread that runs them off the device thread.
//!
//! Classification follows docs/product.md (actions and confirmation): an observed resulting state
//! is State Confirmed, a known start with nothing lasting to observe is Execution Confirmed, a
//! dispatched input whose effect cannot be seen is Unverified (never success), and a known failure
//! is Error. Nothing falls back to another mechanism when an action cannot run (invariant 19).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use kivori_model::desk::{ActionKind, ControlLabels, FeedbackKind, MediaText};

use crate::platform::{
    ActionError, BackendError, InputSynth, MediaObserver, Shortcut, VolumeBackend,
};

/// One discrete action a control can be bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Media play/pause.
    PlayPause,
    /// Toggle master mute on the default output device.
    ToggleMute,
    /// Send a keyboard shortcut.
    Shortcut(Shortcut),
    /// Launch an application (validated target).
    Launch(String),
}

impl Action {
    /// The wire kind of this action.
    #[must_use]
    pub const fn kind(&self) -> ActionKind {
        match self {
            Action::PlayPause => ActionKind::PlayPause,
            Action::ToggleMute => ActionKind::Mute,
            Action::Shortcut(_) => ActionKind::Shortcut,
            Action::Launch(_) => ActionKind::Launch,
        }
    }
}

impl Action {
    /// What the device legend calls this action: short, in the user's terms.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Action::PlayPause => "Play/Pause".into(),
            Action::ToggleMute => "Mute".into(),
            Action::Shortcut(shortcut) => shortcut.to_string(),
            // An app name, not a path.
            Action::Launch(target) => std::path::Path::new(target)
                .file_stem()
                .map_or_else(|| target.clone(), |s| s.to_string_lossy().into_owned()),
        }
    }
}

/// What the push switch does. The rotary binding is fixed to master volume in M1; configurable
/// bindings arrive with the M2 config UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings {
    pub press: Action,
    pub hold: Action,
}

impl Bindings {
    /// The device legend for these bindings. The knob is master volume in M1.
    #[must_use]
    pub fn labels(&self) -> ControlLabels {
        ControlLabels {
            rotate: MediaText::from_text("Volume"),
            press: MediaText::from_text(&self.press.label()),
            hold: MediaText::from_text(&self.hold.label()),
        }
    }
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            press: Action::PlayPause,
            hold: Action::ToggleMute,
        }
    }
}

/// How one executed action ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub kind: FeedbackKind,
    /// The OS needs a permission before this can work (macOS Accessibility).
    pub permission_required: bool,
}

impl Outcome {
    /// A known failure with no further detail.
    #[must_use]
    pub const fn error() -> Self {
        Self::of(FeedbackKind::Error)
    }

    const fn of(kind: FeedbackKind) -> Self {
        Self {
            kind,
            permission_required: false,
        }
    }

    fn from_error(error: &ActionError) -> Self {
        Self {
            kind: FeedbackKind::Error,
            permission_required: matches!(error, ActionError::PermissionRequired),
        }
    }
}

/// The OS services actions run against.
pub struct Platform {
    pub volume: Arc<dyn VolumeBackend>,
    pub synth: Arc<dyn InputSynth>,
    pub media: Arc<dyn MediaObserver>,
    pub launch: fn(&str) -> Result<(), ActionError>,
}

/// Runs `action` and classifies what is known about its outcome. Call it on the action worker,
/// never the device thread (a launch can take a moment).
pub fn execute(action: &Action, platform: &Platform) -> Outcome {
    match action {
        // A media key's effect cannot be tied to this press: the observer's state may be stale
        // or changed by something else, so a matching state is never proof. Always Unverified;
        // the media indicator and view show the playback the OS actually reports.
        Action::PlayPause => match platform.synth.send_media_play_pause() {
            Ok(()) => Outcome::of(FeedbackKind::Unverified),
            Err(error) => Outcome::from_error(&error),
        },
        Action::ToggleMute => {
            let Ok(muted) = platform.volume.read_mute() else {
                return Outcome::of(FeedbackKind::Error);
            };
            match platform.volume.set_mute(!muted) {
                Ok(observed) if observed == !muted => Outcome::of(FeedbackKind::StateConfirmed),
                // The OS reports the old state after the write: known not to have happened.
                Ok(_) => Outcome::of(FeedbackKind::Error),
                Err(BackendError::ReadBackUnavailable) => Outcome::of(FeedbackKind::Unverified),
                Err(_) => Outcome::of(FeedbackKind::Error),
            }
        }
        Action::Shortcut(shortcut) => match platform.synth.send_shortcut(shortcut) {
            Ok(()) => Outcome::of(FeedbackKind::Unverified),
            Err(error) => Outcome::from_error(&error),
        },
        Action::Launch(target) => match (platform.launch)(target) {
            Ok(()) => Outcome::of(FeedbackKind::ExecutionConfirmed),
            Err(error) => Outcome::from_error(&error),
        },
    }
}

/// A finished action, as reported by the worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finished {
    pub id: u64,
    pub action: ActionKind,
    pub outcome: Outcome,
}

/// Runs actions one at a time on a `kivori-actions` thread, so a slow action never stalls the
/// device link. Dropping it stops the thread after the action in progress.
pub struct ActionWorker {
    requests: Option<Sender<(u64, u64, Action)>>,
    finished: Receiver<Finished>,
    /// Requests stamped with an older epoch are skipped, not run.
    epoch: Arc<AtomicU64>,
    thread: Option<JoinHandle<()>>,
}

impl ActionWorker {
    #[must_use]
    pub fn spawn(platform: Platform) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<(u64, u64, Action)>();
        let (finished_tx, finished_rx) = mpsc::channel();
        let epoch = Arc::new(AtomicU64::new(0));
        let current = Arc::clone(&epoch);
        let thread = std::thread::Builder::new()
            .name("kivori-actions".to_string())
            .spawn(move || {
                while let Ok((id, stamped, action)) = request_rx.recv() {
                    if stamped != current.load(Ordering::SeqCst) {
                        continue;
                    }
                    let outcome = execute(&action, &platform);
                    let finished = Finished {
                        id,
                        action: action.kind(),
                        outcome,
                    };
                    if finished_tx.send(finished).is_err() {
                        break;
                    }
                }
            })
            .ok();
        Self {
            requests: Some(request_tx),
            finished: finished_rx,
            epoch,
            thread,
        }
    }

    /// Queues `action` under `id`. Returns `false` if the worker is gone.
    pub fn request(&self, id: u64, action: Action) -> bool {
        let epoch = self.epoch.load(Ordering::SeqCst);
        self.requests
            .as_ref()
            .is_some_and(|tx| tx.send((id, epoch, action)).is_ok())
    }

    /// Every request queued so far is skipped instead of run.
    pub fn cancel_pending(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }

    /// One finished action, if any. Never blocks.
    #[must_use]
    pub fn try_finished(&self) -> Option<Finished> {
        self.finished.try_recv().ok()
    }
}

impl Drop for ActionWorker {
    fn drop(&mut self) {
        self.requests = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{
        ActionAvailability, FakeInputSynth, FakeMediaObserver, FakeVolumeBackend,
    };
    use kivori_model::desk::MediaStatus;

    fn platform(
        synth: Result<(), ActionError>,
        media: Option<MediaStatus>,
        volume: FakeVolumeBackend,
    ) -> (Platform, Arc<FakeMediaObserver>) {
        let observer = Arc::new(FakeMediaObserver::default());
        *observer.status.lock().unwrap() = media;
        (
            Platform {
                volume: Arc::new(volume),
                synth: Arc::new(FakeInputSynth::new(synth)),
                media: observer.clone(),
                launch: |target| {
                    if target == "Missing" {
                        Err(ActionError::Failed("application not found".into()))
                    } else {
                        Ok(())
                    }
                },
            },
            observer,
        )
    }

    fn kind(action: &Action, p: &Platform) -> FeedbackKind {
        execute(action, p).kind
    }

    #[test]
    fn mute_is_state_confirmed_only_when_the_os_reports_the_new_state() {
        let (p, _) = platform(Ok(()), None, FakeVolumeBackend::new(50));
        assert_eq!(kind(&Action::ToggleMute, &p), FeedbackKind::StateConfirmed);
        assert_eq!(p.volume.read_mute(), Ok(true));
        assert_eq!(kind(&Action::ToggleMute, &p), FeedbackKind::StateConfirmed);
        assert_eq!(p.volume.read_mute(), Ok(false));

        let (p, _) = platform(Ok(()), None, FakeVolumeBackend::unreadable_after_write(50));
        assert_eq!(kind(&Action::ToggleMute, &p), FeedbackKind::Unverified);

        let unavailable = FakeVolumeBackend::with_availability(ActionAvailability::Unknown);
        let (p, _) = platform(Ok(()), None, unavailable);
        assert_eq!(kind(&Action::ToggleMute, &p), FeedbackKind::Error);
    }

    #[test]
    fn play_pause_is_never_more_than_unverified() {
        for media in [None, Some(MediaStatus::Paused), Some(MediaStatus::Playing)] {
            let (p, _) = platform(Ok(()), media, FakeVolumeBackend::new(0));
            assert_eq!(kind(&Action::PlayPause, &p), FeedbackKind::Unverified);
        }
    }

    #[test]
    fn requests_queued_before_a_cancel_are_skipped_not_run() {
        let (mut p, _) = platform(Ok(()), None, FakeVolumeBackend::new(0));
        p.launch = |target| {
            if target == "Slow" {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Ok(())
        };
        let volume = Arc::clone(&p.volume);
        let worker = ActionWorker::spawn(p);
        assert!(worker.request(1, Action::Launch("Slow".into())));
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            worker.request(2, Action::ToggleMute),
            "queued behind the slow launch"
        );
        worker.cancel_pending();
        assert!(worker.request(3, Action::Launch("Calculator".into())));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut ids = Vec::new();
        while ids.len() < 2 {
            if let Some(f) = worker.try_finished() {
                ids.push(f.id);
            }
            assert!(std::time::Instant::now() < deadline, "got only {ids:?}");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            ids,
            [1, 3],
            "the in-flight launch finishes; the stale mute never runs"
        );
        assert_eq!(volume.read_mute(), Ok(false));
    }

    #[test]
    fn a_shortcut_is_never_more_than_unverified() {
        let shortcut: Shortcut = "Ctrl+Shift+M".parse().unwrap();
        let (p, _) = platform(Ok(()), None, FakeVolumeBackend::new(0));
        assert_eq!(
            kind(&Action::Shortcut(shortcut), &p),
            FeedbackKind::Unverified
        );
    }

    #[test]
    fn a_missing_permission_is_an_error_that_says_so() {
        let shortcut: Shortcut = "Meta+Space".parse().unwrap();
        let (p, _) = platform(
            Err(ActionError::PermissionRequired),
            None,
            FakeVolumeBackend::new(0),
        );
        assert_eq!(
            execute(&Action::Shortcut(shortcut), &p),
            Outcome {
                kind: FeedbackKind::Error,
                permission_required: true
            }
        );
        assert!(execute(&Action::PlayPause, &p).permission_required);
    }

    #[test]
    fn launch_is_execution_confirmed_or_a_known_failure() {
        let (p, _) = platform(Ok(()), None, FakeVolumeBackend::new(0));
        assert_eq!(
            kind(&Action::Launch("Calculator".into()), &p),
            FeedbackKind::ExecutionConfirmed
        );
        assert_eq!(
            kind(&Action::Launch("Missing".into()), &p),
            FeedbackKind::Error
        );
    }

    #[test]
    fn the_worker_runs_requests_and_reports_them_by_id() {
        let (p, _) = platform(Ok(()), None, FakeVolumeBackend::new(0));
        let worker = ActionWorker::spawn(p);
        assert!(worker.request(7, Action::ToggleMute));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let finished = loop {
            if let Some(f) = worker.try_finished() {
                break f;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "worker never answered"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(finished.id, 7);
        assert_eq!(finished.action, ActionKind::Mute);
        assert_eq!(finished.outcome.kind, FeedbackKind::StateConfirmed);
    }
}
