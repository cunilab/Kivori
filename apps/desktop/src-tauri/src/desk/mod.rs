//! The M1 desk pipeline owned by the device task: push-switch bindings, the action worker, the
//! feedback ladder, and the desk status the device displays.
//!
//! Everything here is driven by the device thread with an injected elapsed time, so the timing
//! rules are host-testable: Processing at ~500 ms, Unverified at ~1,500 ms (timeout alone is never
//! Error), newest deliberate input replaces older feedback, and a status is re-sent whenever what
//! it shows changes (docs/product.md, actions and confirmation).

pub mod actions;

use std::sync::Arc;
use std::time::Duration;

use kivori_model::desk::{
    ActionFeedback, ActionKind, ClockTime, DeskStatus, DisplayMode, FeedbackKind, MediaStatus,
};

use crate::activity::{ActivityEventKind, ActivityMetadata, SessionActivity};
use crate::input::LogicalInput;
use crate::platform::system::{SystemMonitor, SystemProbe, SystemSample};
use crate::platform::{LocalClock, MediaObserver, OsServices, VolumeBackend, VolumeChange};
pub use actions::{Action, ActionWorker, Bindings, Finished, Outcome, Platform};

/// Show Processing when an action has not finished after this long.
pub const PROCESSING_AFTER: Duration = Duration::from_millis(500);
/// Give up waiting and show Unverified after this long.
pub const ACTION_TIMEOUT: Duration = Duration::from_millis(1_500);
/// Re-send an unchanged status this often, so the device clock never drifts far.
pub const STATUS_REFRESH: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
struct Pending {
    id: u64,
    action: ActionKind,
    started: Duration,
    processing_sent: bool,
}

/// Turns action progress into at most one visible feedback at a time.
#[derive(Debug, Default)]
pub struct FeedbackLadder {
    pending: Option<Pending>,
    next_id: u64,
}

impl FeedbackLadder {
    /// Starts tracking a new deliberate action, replacing any older one (no queue). Returns its id.
    pub fn start(&mut self, action: ActionKind, now: Duration) -> u64 {
        self.next_id += 1;
        self.pending = Some(Pending {
            id: self.next_id,
            action,
            started: now,
            processing_sent: false,
        });
        self.next_id
    }

    /// The outcome of action `id`. Shown only if it is still the newest and has not timed out.
    pub fn on_outcome(&mut self, id: u64, kind: FeedbackKind) -> Option<ActionFeedback> {
        let pending = self.pending.filter(|p| p.id == id)?;
        self.pending = None;
        Some(ActionFeedback {
            action: pending.action,
            kind,
        })
    }

    /// Time-driven steps: Processing once at [`PROCESSING_AFTER`], Unverified at
    /// [`ACTION_TIMEOUT`] (after which a late outcome is no longer shown).
    pub fn poll(&mut self, now: Duration) -> Option<ActionFeedback> {
        let pending = self.pending.as_mut()?;
        let age = now.saturating_sub(pending.started);
        if age >= ACTION_TIMEOUT {
            let action = pending.action;
            self.pending = None;
            return Some(ActionFeedback {
                action,
                kind: FeedbackKind::Unverified,
            });
        }
        if age >= PROCESSING_AFTER && !pending.processing_sent {
            pending.processing_sent = true;
            return Some(ActionFeedback {
                action: pending.action,
                kind: FeedbackKind::Processing,
            });
        }
        None
    }

    /// The session ended: nothing in flight will be shown (no stale replay).
    pub fn clear(&mut self) {
        self.pending = None;
    }
}

/// Everything the desk status is built from, as last observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Observed {
    pub volume_percent: Option<u8>,
    pub muted: Option<bool>,
    pub media: Option<MediaStatus>,
    pub system: SystemSample,
}

/// Decides when the device needs a fresh [`DeskStatus`].
#[derive(Debug, Default)]
pub struct StatusPublisher {
    mode: DisplayMode,
    observed: Observed,
    /// What was last sent, without its clock, and when.
    last_sent: Option<(DeskStatus, Duration)>,
}

impl StatusPublisher {
    #[must_use]
    pub const fn mode(&self) -> DisplayMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: DisplayMode) {
        self.mode = mode;
    }

    #[must_use]
    pub const fn observed(&self) -> Observed {
        self.observed
    }

    pub fn observe(&mut self, observed: Observed) {
        self.observed = observed;
    }

    /// The status as it stands, stamped with `clock`.
    #[must_use]
    pub fn status(&self, clock: Option<ClockTime>) -> DeskStatus {
        DeskStatus {
            mode: self.mode,
            clock,
            volume_percent: self.observed.volume_percent,
            muted: self.observed.muted,
            media: self.observed.media,
            cpu_percent: self.observed.system.cpu_percent,
            ram_percent: self.observed.system.ram_percent,
            high_load: self.observed.system.high_load,
        }
    }

    /// The status to send now, if any: when what it shows changed (clock aside — the device runs
    /// its own), when the minute rolled over, or every [`STATUS_REFRESH`].
    pub fn due(&mut self, clock: Option<ClockTime>, now: Duration) -> Option<DeskStatus> {
        let status = self.status(clock);
        let unclocked = DeskStatus {
            clock: clock.map(|c| ClockTime { second: 0, ..c }),
            ..status
        };
        let fresh = match self.last_sent {
            None => true,
            Some((sent, at)) => sent != unclocked || now.saturating_sub(at) >= STATUS_REFRESH,
        };
        if fresh {
            self.last_sent = Some((unclocked, now));
            return Some(status);
        }
        None
    }

    /// Forget what the device has: a new session starts with nothing (send on next `due`).
    pub fn invalidate(&mut self) {
        self.last_sent = None;
    }
}

/// How often CPU/RAM, volume/mute and media are sampled.
pub const SAMPLE_EVERY: Duration = Duration::from_secs(1);

/// What one [`DeskRuntime::tick`] wants sent to the device (if the session negotiated it).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DeskOutput {
    pub status: Option<DeskStatus>,
    pub feedback: Vec<ActionFeedback>,
}

/// The last action outcome, for the desktop UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastAction {
    pub action: ActionKind,
    pub kind: FeedbackKind,
    pub permission_required: bool,
}

/// The desk pipeline the device task drives once per loop iteration.
pub struct DeskRuntime {
    bindings: Bindings,
    worker: ActionWorker,
    ladder: FeedbackLadder,
    publisher: StatusPublisher,
    monitor: SystemMonitor<Box<dyn SystemProbe>>,
    volume: Arc<dyn VolumeBackend>,
    media: Arc<dyn MediaObserver>,
    clock: LocalClock,
    next_sample: Duration,
    outbox: Vec<ActionFeedback>,
    last_action: Option<LastAction>,
}

impl DeskRuntime {
    /// Takes the OS services and starts the action worker.
    #[must_use]
    pub fn new(services: OsServices) -> Self {
        let OsServices {
            volume,
            synth,
            media,
            system,
            clock,
        } = services;
        let worker = ActionWorker::spawn(Platform {
            volume: Arc::clone(&volume),
            synth,
            media: Arc::clone(&media),
            launch: crate::platform::launch::launch,
            media_confirm_window: Duration::from_millis(1_000),
        });
        Self {
            bindings: Bindings::default(),
            worker,
            ladder: FeedbackLadder::default(),
            publisher: StatusPublisher::default(),
            monitor: SystemMonitor::new(system),
            volume,
            media,
            clock,
            next_sample: Duration::ZERO,
            outbox: Vec::new(),
            last_action: None,
        }
    }

    #[must_use]
    pub const fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    #[must_use]
    pub const fn mode(&self) -> DisplayMode {
        self.publisher.mode()
    }

    #[must_use]
    pub const fn observed(&self) -> Observed {
        self.publisher.observed()
    }

    #[must_use]
    pub const fn last_action(&self) -> Option<LastAction> {
        self.last_action
    }

    /// Playback can be observed on this OS right now.
    #[must_use]
    pub fn media_observable(&self) -> bool {
        self.media.status().is_some()
    }

    /// The user picked a display mode.
    pub fn set_mode(&mut self, mode: DisplayMode, observe: &mut impl FnMut(SessionActivity)) {
        if mode != self.publisher.mode() {
            self.publisher.set_mode(mode);
            observe(SessionActivity::new(
                ActivityEventKind::DisplayModeChanged,
                None,
            ));
        }
    }

    /// Runs one deliberate action (the switch or a desktop test action).
    pub fn run(
        &mut self,
        action: Action,
        now: Duration,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        let kind = action.kind();
        let id = self.ladder.start(kind, now);
        observe(desk_activity(ActivityEventKind::DeskActionRequested, kind));
        if !self.worker.request(id, action) {
            self.finish(kind, Outcome::error(), observe);
            self.ladder.clear();
        }
    }

    /// A validated input from the device; only push-switch inputs act here.
    pub fn on_input(
        &mut self,
        input: &LogicalInput,
        now: Duration,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        if let Some(action) = bound_action(&self.bindings, input).cloned() {
            self.run(action, now, observe);
        }
    }

    /// An outcome known elsewhere (a failed volume write) shown through the same channel.
    pub fn report(&mut self, feedback: ActionFeedback) {
        self.outbox.push(feedback);
    }

    /// An OS-observed audio change: show it without waiting for the next sample.
    pub fn on_audio_change(&mut self, change: &VolumeChange) {
        self.publisher.observe(Observed {
            volume_percent: Some(change.percent),
            muted: Some(change.muted),
            ..self.publisher.observed()
        });
    }

    /// A new session: the device knows nothing yet.
    pub fn on_session_begin(&mut self) {
        self.publisher.invalidate();
    }

    /// The session ended: nothing in flight reaches a later session.
    pub fn on_session_end(&mut self) {
        self.ladder.clear();
        self.outbox.clear();
    }

    /// Samples on schedule, collects finished actions and ladder steps, and returns what the
    /// device should receive now.
    pub fn tick(&mut self, now: Duration, observe: &mut impl FnMut(SessionActivity)) -> DeskOutput {
        if now >= self.next_sample {
            self.next_sample = now + SAMPLE_EVERY;
            self.publisher.observe(Observed {
                volume_percent: self.volume.read().ok(),
                muted: self.volume.read_mute().ok(),
                media: self.media.status(),
                system: self.monitor.sample(),
            });
        }
        while let Some(finished) = self.worker.try_finished() {
            self.finish(finished.action, finished.outcome, observe);
            if let Some(feedback) = self.ladder.on_outcome(finished.id, finished.outcome.kind) {
                self.outbox.push(feedback);
            }
        }
        if let Some(feedback) = self.ladder.poll(now) {
            if feedback.kind == FeedbackKind::Unverified {
                observe(desk_activity(
                    ActivityEventKind::DeskActionUnverified,
                    feedback.action,
                ));
            }
            self.outbox.push(feedback);
        }
        DeskOutput {
            status: self.publisher.due((self.clock)(), now),
            feedback: std::mem::take(&mut self.outbox),
        }
    }

    fn finish(
        &mut self,
        action: ActionKind,
        outcome: Outcome,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        self.last_action = Some(LastAction {
            action,
            kind: outcome.kind,
            permission_required: outcome.permission_required,
        });
        let kind = match outcome.kind {
            _ if outcome.permission_required => ActivityEventKind::DeskActionPermissionRequired,
            FeedbackKind::StateConfirmed | FeedbackKind::ExecutionConfirmed => {
                ActivityEventKind::DeskActionConfirmed
            }
            FeedbackKind::Unverified | FeedbackKind::Processing => {
                ActivityEventKind::DeskActionUnverified
            }
            FeedbackKind::Error => ActivityEventKind::DeskActionFailed,
        };
        observe(desk_activity(kind, action));
    }
}

fn desk_activity(kind: ActivityEventKind, action: ActionKind) -> SessionActivity {
    SessionActivity::new(kind, Some(ActivityMetadata::DeskAction { action }))
}

/// The action a push-switch input is bound to, if it is one.
#[must_use]
pub fn bound_action<'b>(bindings: &'b Bindings, input: &LogicalInput) -> Option<&'b Action> {
    match input {
        LogicalInput::Press { .. } => Some(&bindings.press),
        LogicalInput::Hold { .. } => Some(&bindings.hold),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_fast_outcome_is_shown_directly() {
        let mut ladder = FeedbackLadder::default();
        let id = ladder.start(ActionKind::Mute, ms(0));
        assert_eq!(ladder.poll(ms(100)), None);
        assert_eq!(
            ladder.on_outcome(id, FeedbackKind::StateConfirmed),
            Some(ActionFeedback {
                action: ActionKind::Mute,
                kind: FeedbackKind::StateConfirmed
            })
        );
        assert_eq!(ladder.poll(ms(2_000)), None, "nothing left pending");
    }

    #[test]
    fn a_slow_action_shows_processing_once_then_its_outcome() {
        let mut ladder = FeedbackLadder::default();
        let id = ladder.start(ActionKind::Launch, ms(1_000));
        assert_eq!(ladder.poll(ms(1_499)), None);
        assert_eq!(
            ladder.poll(ms(1_500)).map(|f| f.kind),
            Some(FeedbackKind::Processing)
        );
        assert_eq!(ladder.poll(ms(1_600)), None, "Processing is sent once");
        assert_eq!(
            ladder
                .on_outcome(id, FeedbackKind::ExecutionConfirmed)
                .map(|f| f.kind),
            Some(FeedbackKind::ExecutionConfirmed)
        );
    }

    #[test]
    fn a_timeout_is_unverified_never_error_and_a_late_outcome_is_not_shown() {
        let mut ladder = FeedbackLadder::default();
        let id = ladder.start(ActionKind::Launch, ms(0));
        let _ = ladder.poll(ms(600));
        assert_eq!(
            ladder.poll(ms(1_500)).map(|f| f.kind),
            Some(FeedbackKind::Unverified)
        );
        assert_eq!(ladder.on_outcome(id, FeedbackKind::Error), None);
    }

    #[test]
    fn newer_input_replaces_older_feedback() {
        let mut ladder = FeedbackLadder::default();
        let old = ladder.start(ActionKind::Launch, ms(0));
        let new = ladder.start(ActionKind::PlayPause, ms(100));
        assert_eq!(
            ladder.on_outcome(old, FeedbackKind::ExecutionConfirmed),
            None
        );
        assert_eq!(
            ladder
                .on_outcome(new, FeedbackKind::Unverified)
                .map(|f| f.action),
            Some(ActionKind::PlayPause)
        );
    }

    #[test]
    fn a_session_end_drops_the_pending_action() {
        let mut ladder = FeedbackLadder::default();
        let id = ladder.start(ActionKind::Mute, ms(0));
        ladder.clear();
        assert_eq!(ladder.poll(ms(5_000)), None);
        assert_eq!(ladder.on_outcome(id, FeedbackKind::StateConfirmed), None);
    }

    fn clock(minute: u8, second: u8) -> Option<ClockTime> {
        Some(ClockTime {
            hour: 9,
            minute,
            second,
        })
    }

    #[test]
    fn status_is_sent_on_change_minute_rollover_and_refresh_only() {
        let mut publisher = StatusPublisher::default();
        assert!(publisher.due(clock(0, 5), ms(0)).is_some(), "first status");
        assert!(
            publisher.due(clock(0, 6), ms(1_000)).is_none(),
            "seconds only"
        );
        assert!(
            publisher.due(clock(1, 0), ms(55_000)).is_some(),
            "minute rolled"
        );

        publisher.observe(Observed {
            muted: Some(true),
            ..Observed::default()
        });
        let sent = publisher.due(clock(1, 1), ms(56_000)).unwrap();
        assert_eq!(sent.muted, Some(true));
        assert_eq!(sent.clock, clock(1, 1), "the real seconds go out");

        assert!(publisher.due(clock(1, 2), ms(57_000)).is_none());
        assert!(
            publisher.due(clock(1, 3), ms(56_000 + 30_000)).is_some(),
            "periodic refresh"
        );

        publisher.set_mode(DisplayMode::Clock);
        assert_eq!(
            publisher.due(clock(1, 4), ms(87_000)).map(|s| s.mode),
            Some(DisplayMode::Clock)
        );
        publisher.invalidate();
        assert!(
            publisher.due(clock(1, 5), ms(88_000)).is_some(),
            "new session"
        );
    }

    #[test]
    fn press_and_hold_follow_the_bindings_and_rotation_does_not() {
        let bindings = Bindings::default();
        assert_eq!(
            bound_action(&bindings, &LogicalInput::Press { gesture_id: 1 }),
            Some(&Action::PlayPause)
        );
        assert_eq!(
            bound_action(&bindings, &LogicalInput::Hold { gesture_id: 2 }),
            Some(&Action::ToggleMute)
        );
        assert_eq!(
            bound_action(&bindings, &LogicalInput::GestureStarted { gesture_id: 3 }),
            None
        );
    }
}
