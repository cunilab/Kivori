//! Discrete desk actions (M1): what Press and Hold do, how each outcome is classified, and the
//! worker thread that runs them off the device thread.
//!
//! Classification follows docs/product.md (actions and confirmation): an observed resulting state
//! is State Confirmed, a known start with nothing lasting to observe is Execution Confirmed, a
//! dispatched input whose effect cannot be seen is Unverified (never success), and a known failure
//! is Error. Nothing falls back to another mechanism when an action cannot run (invariant 19).

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kivori_model::desk::{ActionKind, FeedbackKind, MediaStatus};

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

/// What the push switch does. The rotary binding is fixed to master volume in M1; configurable
/// bindings arrive with the M2 config UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings {
    pub press: Action,
    pub hold: Action,
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
    /// How long play/pause watches the media observer for the resulting state.
    pub media_confirm_window: Duration,
}

/// Runs `action` and classifies what is known about its outcome. May block (play/pause watches
/// for up to `media_confirm_window`); call it on the action worker, never the device thread.
pub fn execute(action: &Action, platform: &Platform) -> Outcome {
    match action {
        Action::PlayPause => {
            let before = platform.media.status();
            if let Err(error) = platform.synth.send_media_play_pause() {
                return Outcome::from_error(&error);
            }
            // A media key's effect is only knowable where playback is observable. The key
            // toggles, so a known state flipping to its opposite is the observed result.
            let expected = match before {
                Some(MediaStatus::Playing) => Some(MediaStatus::Paused),
                Some(MediaStatus::Paused) => Some(MediaStatus::Playing),
                Some(MediaStatus::Stopped) | None => None,
            };
            let Some(expected) = expected else {
                return Outcome::of(FeedbackKind::Unverified);
            };
            let deadline = Instant::now() + platform.media_confirm_window;
            loop {
                if platform.media.status() == Some(expected) {
                    return Outcome::of(FeedbackKind::StateConfirmed);
                }
                if Instant::now() >= deadline {
                    return Outcome::of(FeedbackKind::Unverified);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
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
    requests: Option<Sender<(u64, Action)>>,
    finished: Receiver<Finished>,
    thread: Option<JoinHandle<()>>,
}

impl ActionWorker {
    #[must_use]
    pub fn spawn(platform: Platform) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<(u64, Action)>();
        let (finished_tx, finished_rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("kivori-actions".to_string())
            .spawn(move || {
                while let Ok((id, action)) = request_rx.recv() {
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
            thread,
        }
    }

    /// Queues `action` under `id`. Returns `false` if the worker is gone.
    pub fn request(&self, id: u64, action: Action) -> bool {
        self.requests
            .as_ref()
            .is_some_and(|tx| tx.send((id, action)).is_ok())
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
                media_confirm_window: Duration::ZERO,
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
    fn play_pause_is_confirmed_only_when_playback_is_seen_to_flip() {
        let (p, observer) = platform(Ok(()), Some(MediaStatus::Paused), FakeVolumeBackend::new(0));
        // The observer still reports Paused: the key went out, the effect is unknown.
        assert_eq!(kind(&Action::PlayPause, &p), FeedbackKind::Unverified);
        *observer.status.lock().unwrap() = Some(MediaStatus::Playing);
        // Reported Playing before the key; the fake keeps Playing, so no flip is seen.
        assert_eq!(kind(&Action::PlayPause, &p), FeedbackKind::Unverified);

        let (p, _) = platform(Ok(()), None, FakeVolumeBackend::new(0));
        assert_eq!(
            kind(&Action::PlayPause, &p),
            FeedbackKind::Unverified,
            "unobservable playback is never confirmed"
        );
    }

    #[test]
    fn play_pause_confirms_when_the_observer_sees_the_opposite_state() {
        struct Flips(std::sync::Mutex<Vec<MediaStatus>>);
        impl MediaObserver for Flips {
            fn status(&self) -> Option<MediaStatus> {
                let mut s = self.0.lock().unwrap();
                Some(if s.len() > 1 { s.remove(0) } else { s[0] })
            }
        }
        let (mut p, _) = platform(Ok(()), None, FakeVolumeBackend::new(0));
        p.media = Arc::new(Flips(std::sync::Mutex::new(vec![
            MediaStatus::Playing,
            MediaStatus::Paused,
        ])));
        p.media_confirm_window = Duration::from_millis(200);
        assert_eq!(kind(&Action::PlayPause, &p), FeedbackKind::StateConfirmed);
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
        let deadline = Instant::now() + Duration::from_secs(2);
        let finished = loop {
            if let Some(f) = worker.try_finished() {
                break f;
            }
            assert!(Instant::now() < deadline, "worker never answered");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(finished.id, 7);
        assert_eq!(finished.action, ActionKind::Mute);
        assert_eq!(finished.outcome.kind, FeedbackKind::StateConfirmed);
    }
}
