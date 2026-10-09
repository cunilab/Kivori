//! The M1 desk pipeline owned by the device task: push-switch bindings, the action worker, the
//! feedback ladder, and the desk status the device displays.
//!
//! Everything here is driven by the device thread with an injected elapsed time, so the timing
//! rules are host-testable: Processing at ~500 ms, Unverified at ~1,500 ms (timeout alone is never
//! Error), newest deliberate input replaces older feedback, and a status is re-sent whenever what
//! it shows changes (docs/product.md, actions and confirmation).

pub mod actions;
pub mod profile;

use std::sync::Arc;
use std::time::{Duration, Instant};

use kivori_model::desk::{
    ActionFeedback, ActionKind, ClockTime, ControlLabels, DeskStatus, DisplayMode, FeedbackKind,
    MediaInfo, MediaStatus, MediaText,
};

use crate::activity::{ActivityEventKind, ActivityMetadata, SessionActivity};
use crate::config::{DisplaySettings, ResolvedConfig, SecondaryView};
use crate::input::LogicalInput;
use crate::platform::system::{SystemMonitor, SystemProbe, SystemSample};
use crate::platform::{
    Foreground, ForegroundObserver, LocalClock, MediaObserver, NowPlaying, OsServices, Shortcut,
    VolumeBackend, VolumeChange,
};
pub use actions::{Action, ActionWorker, Bindings, ButtonSlots, Finished, Outcome, Platform, Slot};
use kivori_model::input::Direction;
use profile::{Context, Resolved, RotateBinding};

/// Show Processing when an action has not finished after this long.
pub const PROCESSING_AFTER: Duration = Duration::from_millis(500);
/// Give up waiting and show Unverified after this long.
pub const ACTION_TIMEOUT: Duration = Duration::from_millis(1_500);
/// Re-send an unchanged status this often, so the device clock never drifts far.
// A lost initial status must recover within the ten-second reconnect/restore budget.
pub const STATUS_REFRESH: Duration = Duration::from_secs(1);

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

    /// Action `id` never ran (it expired in the queue): nothing is shown for it.
    pub fn cancel(&mut self, id: u64) {
        if self.pending.is_some_and(|p| p.id == id) {
            self.pending = None;
        }
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
/// How often the focused app is observed.
pub const FOCUS_POLL: Duration = Duration::from_millis(100);
/// Worker ids of knob-shortcut detents carry this bit; the feedback ladder never tracks them.
const KNOB: u64 = 1 << 63;

/// A rotary gesture that owns a shortcut binding (a Volume gesture belongs to the volume loop).
#[derive(Debug, Clone)]
struct KnobGesture {
    gesture_id: u16,
    /// The focused app when the gesture began. Its keys go to whatever has focus, so once focus
    /// moves the gesture is Context Lost, never retargeted (invariant 12).
    target: Foreground,
    /// Worker id for its detents: `KNOB` plus a per-gesture counter.
    id: u64,
    /// `(cw, ccw)`; `None` = suspended (a protected context), every detent is refused.
    keys: Option<(Shortcut, Shortcut)>,
}

/// What one [`DeskRuntime::tick`] wants sent to the device (if the session negotiated it).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DeskOutput {
    pub status: Option<DeskStatus>,
    pub feedback: Vec<ActionFeedback>,
    /// New now-playing text to send (`Some(None)` clears it on the device).
    pub media_info: Option<Option<MediaInfo>>,
    /// Control labels to send: once per session, and again if a binding changes.
    pub controls: Option<ControlLabels>,
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
    context: Context,
    /// Which view is the home view and what a double press shows.
    display: DisplaySettings,
    foreground: Arc<dyn ForegroundObserver>,
    next_focus_poll: Duration,
    knob: Option<KnobGesture>,
    knob_count: u64,
    /// The knob gesture whose failure was already shown (once per gesture).
    knob_failed: Option<u64>,
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
    now_playing: Option<NowPlaying>,
    /// What the device was last sent, `None` = nothing sent this session.
    sent_media: Option<Option<MediaInfo>>,
    /// The control labels the device was last sent, `None` = nothing sent this session.
    sent_controls: Option<ControlLabels>,
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
            foreground,
        } = services;
        let worker = ActionWorker::spawn(Platform {
            volume: Arc::clone(&volume),
            synth,
            media: Arc::clone(&media),
            launch: crate::platform::launch::launch,
        });
        Self {
            context: Context::new(profile::builtins()),
            display: DisplaySettings::default(),
            foreground,
            next_focus_poll: Duration::ZERO,
            knob: None,
            knob_count: 0,
            knob_failed: None,
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
            now_playing: None,
            sent_media: None,
            sent_controls: None,
        }
    }

    /// Starts on `display`'s default view, quietly: nothing changed yet that anyone could log.
    #[must_use]
    pub fn with_display(mut self, display: DisplaySettings) -> Self {
        self.publisher.set_mode(display.default_view.mode());
        self.display = display;
        self
    }

    /// Starts on `config`'s profiles and display, quietly (like [`Self::with_display`]).
    #[must_use]
    pub fn with_config(mut self, config: &ResolvedConfig) -> Self {
        self.context.set_profiles(config.profiles.clone());
        self.with_display(config.display)
    }

    /// Adopts a saved config live: new profiles (the pin and the committed app's profile stay),
    /// the display settings, and a forced label re-send so the device never shows stale labels.
    /// A knob gesture in progress keeps the keys it began with (invariant 12).
    pub fn apply_config(
        &mut self,
        config: &ResolvedConfig,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        self.context.set_profiles(config.profiles.clone());
        self.apply_display(config.display, observe);
        self.sent_controls = None;
    }

    /// Adopts new display settings. Moving the default view takes the device there now; changing
    /// only the double-press view leaves what is showing alone.
    pub fn apply_display(
        &mut self,
        display: DisplaySettings,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        let moved = display.default_view != self.display.default_view;
        self.display = display;
        if moved {
            self.set_mode(display.default_view.mode(), observe);
        }
    }

    /// The active profile's bindings.
    #[must_use]
    pub fn bindings(&self) -> &Bindings {
        &self.context.profile().bindings
    }

    /// Which profile owns the controls.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }

    /// The device legend for the active profile.
    #[must_use]
    pub fn labels(&self) -> ControlLabels {
        self.context.labels()
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

    /// What is playing, as last observed.
    #[must_use]
    pub const fn now_playing(&self) -> Option<&NowPlaying> {
        self.now_playing.as_ref()
    }

    /// Handles a double press on the device. With a chosen double-press view it toggles between
    /// that and the default view (from any other view it returns to the default); `Cycle` steps
    /// through every view in turn.
    pub fn next_mode(&mut self, observe: &mut impl FnMut(SessionActivity)) {
        let next = match self.display.secondary_view {
            SecondaryView::View(secondary) => {
                let home = self.display.default_view.mode();
                if self.mode() == home {
                    secondary.mode()
                } else {
                    home
                }
            }
            SecondaryView::Cycle => {
                let all = DisplayMode::ALL;
                let at = all.iter().position(|m| *m == self.mode()).unwrap_or(0);
                all[(at + 1) % all.len()]
            }
        };
        self.set_mode(next, observe);
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
        self.run_within(action, now, None, observe);
    }

    /// [`Self::run`] for an action driven by device input: it must start within `remaining`
    /// (what is left of its freshness budget) or the worker skips it (issue #26).
    fn run_within(
        &mut self,
        action: Action,
        now: Duration,
        remaining: Option<Duration>,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        let kind = action.kind();
        let id = self.ladder.start(kind, now);
        observe(desk_activity(ActivityEventKind::DeskActionRequested, kind));
        let deadline = remaining.map(|remaining| Instant::now() + remaining);
        if !self.worker.request_by(id, action, deadline) {
            // The worker is gone: say so on the device too, never stay silent (gate 9).
            self.fail(id, kind, observe);
        }
    }

    /// A control whose action can't run here (suspended in a protected context): it says so,
    /// never silently (invariant 19).
    fn refuse(
        &mut self,
        kind: ActionKind,
        now: Duration,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        let id = self.ladder.start(kind, now);
        observe(desk_activity(ActivityEventKind::DeskActionRequested, kind));
        self.fail(id, kind, observe);
    }

    fn fail(&mut self, id: u64, kind: ActionKind, observe: &mut impl FnMut(SessionActivity)) {
        self.finish(kind, Outcome::error(), observe);
        if let Some(feedback) = self.ladder.on_outcome(id, FeedbackKind::Error) {
            self.outbox.push(feedback);
        }
    }

    /// A validated input from the device, resolved against the active profile. Returns whether
    /// the rotary volume loop owns it too (a detent or end of a gesture that began bound to
    /// Volume); every other input is handled here.
    pub fn on_input(
        &mut self,
        input: &LogicalInput,
        now: Duration,
        observe: &mut impl FnMut(SessionActivity),
    ) -> bool {
        self.handle_input(input, now, None, observe)
    }

    /// [`Self::on_input`] for input whose age is known: a discrete action it triggers must start
    /// within `remaining` of its freshness budget, or it is skipped and logged as `InputStale`.
    pub fn on_input_within(
        &mut self,
        input: &LogicalInput,
        now: Duration,
        remaining: Duration,
        observe: &mut impl FnMut(SessionActivity),
    ) -> bool {
        self.handle_input(input, now, Some(remaining), observe)
    }

    fn handle_input(
        &mut self,
        input: &LogicalInput,
        now: Duration,
        remaining: Option<Duration>,
        observe: &mut impl FnMut(SessionActivity),
    ) -> bool {
        match *input {
            LogicalInput::Detent {
                gesture_id,
                direction,
            } => return self.on_detent(gesture_id, direction, observe),
            LogicalInput::GestureEnded { gesture_id } => {
                let ours = self
                    .knob
                    .as_ref()
                    .is_some_and(|k| k.gesture_id == gesture_id);
                if ours {
                    self.knob = None;
                }
                return !ours;
            }
            _ => {}
        }
        // Deliberate input: Protected is classified first, then a pending app commits at once
        // (invariants 8 and 9).
        self.poll_focus(now);
        self.context.commit_pending();
        match *input {
            LogicalInput::DoublePress { .. } => self.next_mode(observe),
            // A gesture keeps the binding it began with until it ends (invariant 12).
            LogicalInput::GestureStarted { gesture_id } => {
                self.knob = None;
                let keys = match self.context.rotate() {
                    Some(RotateBinding::Volume) => return true,
                    Some(RotateBinding::Shortcuts { cw, ccw, .. }) => Some((*cw, *ccw)),
                    None => None,
                };
                self.knob_count += 1;
                self.knob = Some(KnobGesture {
                    gesture_id,
                    target: self.context.latest().clone(),
                    id: KNOB | self.knob_count,
                    keys,
                });
            }
            _ => match self.context.resolve(input) {
                Resolved::Nothing => {}
                Resolved::Run(action) => self.run_within(action, now, remaining, observe),
                Resolved::Suspended(action) => self.refuse(action.kind(), now, observe),
                // Not an action: the new labels are the feedback.
                // ponytail: not in the activity log, no event kind fits; add one if users ask.
                Resolved::CyclePin => self.context.cycle_pin(),
            },
        }
        false
    }

    /// One detent of a knob-shortcut gesture: its key goes through the action worker with no
    /// feedback of its own (the label is the feedback). Returns `true` for a Volume gesture.
    fn on_detent(
        &mut self,
        gesture_id: u16,
        direction: Direction,
        observe: &mut impl FnMut(SessionActivity),
    ) -> bool {
        let Some(knob) = self.knob.clone().filter(|k| k.gesture_id == gesture_id) else {
            return true;
        };
        // Protected beats even the binding a gesture began with: no keys into a secure surface.
        // Focus moved since the gesture began: Context Lost, its remaining detents are ignored.
        let keys = knob
            .keys
            .filter(|_| !self.context.protected() && *self.context.latest() == knob.target);
        let sent = keys.is_some_and(|(cw, ccw)| {
            let key = if direction == Direction::Cw { cw } else { ccw };
            self.worker.request(knob.id, Action::Shortcut(key))
        });
        if !sent {
            self.knob_failure(knob.id, Outcome::error(), observe);
        }
        false
    }

    /// A knob gesture failed: shown once per gesture, never per detent.
    fn knob_failure(
        &mut self,
        id: u64,
        outcome: Outcome,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        if self.knob_failed == Some(id) {
            return;
        }
        self.knob_failed = Some(id);
        self.finish(ActionKind::Shortcut, outcome, observe);
        self.outbox.push(ActionFeedback {
            action: ActionKind::Shortcut,
            kind: FeedbackKind::Error,
        });
    }

    fn poll_focus(&mut self, now: Duration) {
        self.next_focus_poll = now + FOCUS_POLL;
        self.context.observe(self.foreground.foreground(), now);
    }

    /// An outcome known elsewhere (a failed volume write) shown through the same channel.
    pub fn report(&mut self, feedback: ActionFeedback) {
        self.outbox.push(feedback);
    }

    /// An OS-observed audio change: show it without waiting for the next sample.
    pub fn on_audio_change(&mut self, change: &VolumeChange) {
        self.publisher.observe(Observed {
            volume_percent: Some(change.percent),
            muted: change.muted,
            ..self.publisher.observed()
        });
    }

    /// A new session: the device knows nothing yet.
    pub fn on_session_begin(&mut self) {
        self.publisher.invalidate();
        self.sent_media = None;
        self.sent_controls = None;
    }

    /// The session ended: nothing in flight reaches a later session.
    pub fn on_session_end(&mut self) {
        self.ladder.clear();
        self.outbox.clear();
        // The device drops an open gesture with the session.
        self.knob = None;
        // Presses still queued for the worker belong to the old session: they must not run
        // (gate 2). One already executing cannot be recalled.
        self.worker.cancel_pending();
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
            self.now_playing = self.media.now_playing();
        }
        if now >= self.next_focus_poll {
            self.poll_focus(now);
        }
        let media_info = self.now_playing.as_ref().map(|np| MediaInfo {
            title: MediaText::from_text(&np.title),
            artist: MediaText::from_text(&np.artist),
        });
        let media_update = (self.sent_media != Some(media_info)).then(|| {
            self.sent_media = Some(media_info);
            media_info
        });
        // From the same context the inputs resolve against: labels and bindings switch together.
        let labels = self.context.labels();
        let controls = (self.sent_controls != Some(labels)).then(|| {
            self.sent_controls = Some(labels);
            labels
        });
        while let Some(id) = self.worker.try_expired() {
            // Skipped, so nothing ran: no outcome, no feedback (it would claim a dispatch).
            self.ladder.cancel(id);
            observe(SessionActivity::new(ActivityEventKind::InputStale, None));
        }
        while let Some(finished) = self.worker.try_finished() {
            if finished.id & KNOB != 0 {
                if finished.outcome.kind == FeedbackKind::Error {
                    self.knob_failure(finished.id, finished.outcome, observe);
                }
                continue;
            }
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
            media_info: media_update,
            controls,
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
        LogicalInput::Press { .. } => bindings.press.action.as_ref(),
        LogicalInput::Hold { .. } => bindings.hold.action.as_ref(),
        LogicalInput::ButtonPress { button, .. } => bindings
            .buttons
            .get(usize::from(*button))?
            .press
            .action
            .as_ref(),
        LogicalInput::ButtonHold { button, .. } => bindings
            .buttons
            .get(usize::from(*button))?
            .hold
            .action
            .as_ref(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::View;

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
            publisher.due(clock(0, 6), ms(500)).is_none(),
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

        assert!(publisher.due(clock(1, 2), ms(56_500)).is_none());
        assert!(
            publisher.due(clock(1, 3), ms(57_000)).is_some(),
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

    fn runtime(media: Arc<crate::platform::FakeMediaObserver>) -> DeskRuntime {
        use crate::platform::system::NoSystemProbe;
        use crate::platform::{FakeInputSynth, FakeVolumeBackend};
        DeskRuntime::new(OsServices {
            volume: Arc::new(FakeVolumeBackend::new(20)),
            synth: Arc::new(FakeInputSynth::new(Ok(()))),
            media,
            system: Box::new(NoSystemProbe),
            clock: || None,
            foreground: Arc::new(crate::platform::FakeForeground::default()),
        })
    }

    use crate::platform::{ActionError, FakeForeground, FakeInputSynth, Foreground};

    fn desk_with(foreground: &Arc<FakeForeground>, synth: &Arc<FakeInputSynth>) -> DeskRuntime {
        use crate::platform::system::NoSystemProbe;
        use crate::platform::FakeVolumeBackend;
        let mut desk = DeskRuntime::new(OsServices {
            volume: Arc::new(FakeVolumeBackend::new(20)),
            synth: synth.clone(),
            media: Arc::new(crate::platform::FakeMediaObserver::default()),
            system: Box::new(NoSystemProbe),
            clock: || None,
            foreground: foreground.clone(),
        });
        desk.on_session_begin();
        desk
    }

    fn focus(foreground: &FakeForeground, id: &str) {
        *foreground.0.lock().unwrap() = Foreground::App {
            id: id.into(),
            name: id.into(),
        };
    }

    fn sent(synth: &FakeInputSynth) -> Vec<String> {
        synth.sent.lock().unwrap().clone()
    }

    fn text(t: MediaText) -> String {
        t.as_latin1().iter().map(|&b| char::from(b)).collect()
    }

    /// Ticks until the worker has reported `n` more results' worth of time, collecting output.
    fn settle(
        desk: &mut DeskRuntime,
        at: Duration,
        until: impl Fn(&[ActionFeedback]) -> bool,
    ) -> Vec<ActionFeedback> {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut feedback = Vec::new();
        loop {
            feedback.extend(desk.tick(at, &mut |_| {}).feedback);
            if until(&feedback) || std::time::Instant::now() > deadline {
                return feedback;
            }
            std::thread::sleep(ms(5));
        }
    }

    const fn button(button: u8, gesture_id: u16) -> LogicalInput {
        LogicalInput::ButtonPress { button, gesture_id }
    }

    #[test]
    fn labels_and_bindings_switch_together_in_the_same_tick() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        let first = desk.tick(ms(0), &mut |_| {}).controls.unwrap();
        assert_eq!(text(first.profile), "", "General fallback is calm");
        focus(&fg, "chrome.exe");
        let pending = desk.tick(ms(100), &mut |_| {});
        assert_eq!(pending.controls, None, "not stable yet");
        assert_eq!(desk.bindings(), &Bindings::default());
        let out = desk.tick(ms(500), &mut |_| {});
        let labels = out.controls.expect("new labels in the commit tick");
        assert_eq!(text(labels.profile), "Browser");
        assert_eq!(text(labels.rotate), "Tabs");
        assert_eq!(labels.buttons.map(text), ["Back", "Reload", "New tab"]);
        assert!(matches!(
            bound_action(desk.bindings(), &button(1, 1)),
            Some(Action::Shortcut(_))
        ));
        assert_eq!(desk.labels(), labels);
    }

    #[test]
    fn deliberate_input_commits_the_pending_app_before_it_resolves() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        let _ = desk.tick(ms(0), &mut |_| {});
        focus(&fg, "chrome.exe");
        // 50 ms after focus moved, long before 400 ms: the press belongs to the browser.
        let _ = desk.tick(ms(10), &mut |_| {});
        desk.on_input(&button(1, 1), ms(50), &mut |_| {});
        let feedback = settle(&mut desk, ms(60), |f| !f.is_empty());
        let reload = if cfg!(target_os = "macos") {
            "Meta+R"
        } else {
            "Ctrl+R"
        };
        assert_eq!(sent(&synth), [reload]);
        assert_eq!(feedback[0].kind, FeedbackKind::Unverified);
        assert_eq!(text(desk.labels().profile), "Browser");
    }

    #[test]
    fn protected_suspends_shortcuts_with_an_error_and_keeps_system_actions() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        // Pin Browser (Auto -> General -> Browser), then a protected surface takes focus.
        for id in 1..=2 {
            desk.on_input(
                &LogicalInput::ButtonHold {
                    button: 1,
                    gesture_id: id,
                },
                ms(0),
                &mut |_| {},
            );
        }
        assert_eq!(text(desk.labels().profile), "Browser");
        assert!(
            desk.tick(ms(0), &mut |_| {}).feedback.is_empty(),
            "a pin is not an action"
        );
        *fg.0.lock().unwrap() = Foreground::Protected;
        // The press itself classifies Protected first, without waiting for a poll.
        let mut log = Vec::new();
        desk.on_input(&button(0, 3), ms(10), &mut |o| log.push(o.kind));
        let out = desk.tick(ms(10), &mut |_| {});
        assert_eq!(
            out.feedback,
            [ActionFeedback {
                action: ActionKind::Shortcut,
                kind: FeedbackKind::Error
            }]
        );
        assert_eq!(
            log,
            [
                ActivityEventKind::DeskActionRequested,
                ActivityEventKind::DeskActionFailed
            ]
        );
        let labels = out.controls.unwrap();
        assert_eq!(text(labels.profile), "Protected");
        assert!(labels.pinned);
        assert_eq!(text(labels.rotate), "");
        assert!(labels.buttons.iter().all(MediaText::is_empty));
        // The knob's shortcuts are suspended too: one Error for the gesture, no keys.
        assert!(!desk.on_input(
            &LogicalInput::GestureStarted { gesture_id: 4 },
            ms(20),
            &mut |_| {}
        ));
        for _ in 0..3 {
            let detent = LogicalInput::Detent {
                gesture_id: 4,
                direction: Direction::Cw,
            };
            assert!(!desk.on_input(&detent, ms(30), &mut |_| {}));
        }
        assert_eq!(desk.tick(ms(30), &mut |_| {}).feedback.len(), 1);
        // A system action still runs.
        desk.on_input(&LogicalInput::Hold { gesture_id: 5 }, ms(40), &mut |_| {});
        let feedback = settle(&mut desk, ms(50), |f| !f.is_empty());
        assert_eq!(feedback[0].action, ActionKind::Mute);
        assert_eq!(feedback[0].kind, FeedbackKind::StateConfirmed);
        assert!(
            sent(&synth).is_empty(),
            "no key ever reached the protected surface"
        );
    }

    #[test]
    fn a_gesture_keeps_the_binding_it_began_with() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        focus(&fg, "chrome.exe");
        let _ = desk.tick(ms(0), &mut |_| {});
        let _ = desk.tick(ms(400), &mut |_| {});
        let start = LogicalInput::GestureStarted { gesture_id: 1 };
        assert!(
            !desk.on_input(&start, ms(500), &mut |_| {}),
            "a Tabs gesture is not volume"
        );
        let detent = |direction| LogicalInput::Detent {
            gesture_id: 1,
            direction,
        };
        assert!(!desk.on_input(&detent(Direction::Cw), ms(520), &mut |_| {}));
        // Simulated tick time does not advance the action worker's OS thread.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while sent(&synth).is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(ms(1));
        }
        assert_eq!(sent(&synth), ["Ctrl+Tab"]);
        // Focus moves to the editor (a Volume knob) and commits mid-gesture. The gesture is not
        // retargeted to Volume, and its keys must not land in the editor: Context Lost, shown
        // once, remaining detents ignored (invariant 12).
        focus(&fg, "code.exe");
        let _ = desk.tick(ms(600), &mut |_| {});
        let _ = desk.tick(ms(1_000), &mut |_| {});
        assert_eq!(text(desk.labels().rotate), "Volume");
        for direction in [Direction::Cw, Direction::Ccw] {
            assert!(
                !desk.on_input(&detent(direction), ms(1_100), &mut |_| {}),
                "still the Tabs gesture, never the volume loop"
            );
        }
        assert!(!desk.on_input(
            &LogicalInput::GestureEnded { gesture_id: 1 },
            ms(1_200),
            &mut |_| {}
        ));
        let feedback = settle(&mut desk, ms(1_300), |f| !f.is_empty());
        assert_eq!(sent(&synth), ["Ctrl+Tab"], "nothing after focus moved");
        assert_eq!(
            feedback.iter().map(|f| f.kind).collect::<Vec<_>>(),
            [FeedbackKind::Error],
            "Context Lost is shown once, no badge per detent"
        );

        // The next gesture is a Volume gesture, and stays one after the browser comes back.
        assert!(desk.on_input(
            &LogicalInput::GestureStarted { gesture_id: 2 },
            ms(1_300),
            &mut |_| {}
        ));
        focus(&fg, "chrome.exe");
        let _ = desk.tick(ms(1_400), &mut |_| {});
        let _ = desk.tick(ms(1_800), &mut |_| {});
        assert_eq!(text(desk.labels().rotate), "Tabs");
        let detent = LogicalInput::Detent {
            gesture_id: 2,
            direction: Direction::Cw,
        };
        assert!(
            desk.on_input(&detent, ms(1_900), &mut |_| {}),
            "the volume loop owns it"
        );
        assert!(desk.on_input(
            &LogicalInput::GestureEnded { gesture_id: 2 },
            ms(2_000),
            &mut |_| {}
        ));
        assert_eq!(sent(&synth).len(), 1);
    }

    #[test]
    fn protected_mid_gesture_stops_the_knob_keys() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        focus(&fg, "brave.exe");
        let start = LogicalInput::GestureStarted { gesture_id: 1 };
        assert!(!desk.on_input(&start, ms(0), &mut |_| {}));
        *fg.0.lock().unwrap() = Foreground::Protected;
        let _ = desk.tick(ms(100), &mut |_| {});
        for _ in 0..2 {
            let detent = LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw,
            };
            assert!(!desk.on_input(&detent, ms(150), &mut |_| {}));
        }
        let out = desk.tick(ms(200), &mut |_| {});
        assert_eq!(out.feedback.len(), 1, "one Error for the gesture");
        assert_eq!(out.feedback[0].kind, FeedbackKind::Error);
        std::thread::sleep(ms(20));
        assert!(sent(&synth).is_empty());
    }

    #[test]
    fn a_failing_knob_shortcut_shows_one_error_per_gesture() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Err(ActionError::PermissionRequired)));
        let mut desk = desk_with(&fg, &synth);
        focus(&fg, "firefox.exe");
        for gesture_id in [1, 2] {
            let mut log = Vec::new();
            desk.on_input(
                &LogicalInput::GestureStarted { gesture_id },
                ms(0),
                &mut |o| log.push(o.kind),
            );
            for _ in 0..3 {
                let detent = LogicalInput::Detent {
                    gesture_id,
                    direction: Direction::Ccw,
                };
                desk.on_input(&detent, ms(10), &mut |o| log.push(o.kind));
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            let mut feedback = Vec::new();
            while synth.sent.lock().unwrap().len() < 3 * usize::from(gesture_id) {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(ms(5));
            }
            std::thread::sleep(ms(20));
            feedback.extend(desk.tick(ms(20), &mut |o| log.push(o.kind)).feedback);
            assert_eq!(
                feedback,
                [ActionFeedback {
                    action: ActionKind::Shortcut,
                    kind: FeedbackKind::Error
                }],
                "gesture {gesture_id}"
            );
            assert_eq!(log, [ActivityEventKind::DeskActionPermissionRequired]);
            assert!(desk.last_action().unwrap().permission_required);
            desk.on_input(
                &LogicalInput::GestureEnded { gesture_id },
                ms(30),
                &mut |_| {},
            );
        }
    }

    #[test]
    fn a_double_press_cycles_through_every_view_and_wraps() {
        let mut desk = runtime(Arc::default()).with_display(DisplaySettings {
            secondary_view: SecondaryView::Cycle,
            ..DisplaySettings::default()
        });
        let mut seen = vec![desk.mode()];
        for id in 1..=DisplayMode::ALL.len() as u16 {
            desk.on_input(
                &LogicalInput::DoublePress { gesture_id: id },
                ms(0),
                &mut |_| {},
            );
            seen.push(desk.mode());
        }
        assert_eq!(&seen[..5], &DisplayMode::ALL);
        assert_eq!(seen[5], DisplayMode::Buddy, "wraps back to the buddy");
        assert!(
            desk.tick(ms(0), &mut |_| {}).feedback.is_empty(),
            "not an action"
        );
    }

    fn double_press(desk: &mut DeskRuntime, id: u16) {
        desk.on_input(
            &LogicalInput::DoublePress { gesture_id: id },
            ms(0),
            &mut |_| {},
        );
    }

    #[test]
    fn a_double_press_toggles_between_the_default_and_the_chosen_view() {
        let mut desk = runtime(Arc::default());
        assert_eq!(
            desk.mode(),
            DisplayMode::Buddy,
            "default config starts on the buddy"
        );
        double_press(&mut desk, 1);
        assert_eq!(desk.mode(), DisplayMode::System);
        double_press(&mut desk, 2);
        assert_eq!(desk.mode(), DisplayMode::Buddy);
    }

    #[test]
    fn a_double_press_from_any_other_view_returns_to_the_default() {
        let mut desk = runtime(Arc::default()).with_display(DisplaySettings {
            default_view: View::Clock,
            secondary_view: SecondaryView::View(View::Media),
        });
        assert_eq!(
            desk.mode(),
            DisplayMode::Clock,
            "starts on the configured default"
        );
        desk.set_mode(DisplayMode::Volume, &mut |_| {});
        double_press(&mut desk, 1);
        assert_eq!(desk.mode(), DisplayMode::Clock);
        double_press(&mut desk, 2);
        assert_eq!(desk.mode(), DisplayMode::Media);
    }

    #[test]
    fn moving_the_default_view_takes_the_device_there_but_other_changes_do_not() {
        let mut desk = runtime(Arc::default());
        let mut log = Vec::new();
        let mut moved = DisplaySettings {
            default_view: View::Clock,
            ..DisplaySettings::default()
        };
        desk.apply_display(moved, &mut |o| log.push(o.kind));
        assert_eq!(desk.mode(), DisplayMode::Clock);
        assert_eq!(log, [ActivityEventKind::DisplayModeChanged]);

        desk.set_mode(DisplayMode::Volume, &mut |_| {});
        moved.secondary_view = SecondaryView::Cycle;
        desk.apply_display(moved, &mut |o| log.push(o.kind));
        assert_eq!(
            desk.mode(),
            DisplayMode::Volume,
            "only the double-press view changed"
        );
    }

    #[test]
    fn now_playing_is_sent_once_per_change_and_again_for_a_new_session() {
        use crate::platform::{FakeMediaObserver, NowPlaying};
        let media = Arc::new(FakeMediaObserver::default());
        let mut desk = runtime(Arc::clone(&media));
        desk.on_session_begin();
        assert_eq!(
            desk.tick(ms(0), &mut |_| {}).media_info,
            Some(None),
            "clears first"
        );
        assert_eq!(
            desk.tick(ms(100), &mut |_| {}).media_info,
            None,
            "unchanged"
        );

        *media.now_playing.lock().unwrap() = Some(NowPlaying {
            title: "Song".into(),
            artist: "Band".into(),
        });
        let sent = desk
            .tick(ms(1_000), &mut |_| {})
            .media_info
            .flatten()
            .unwrap();
        assert_eq!(sent.title.as_latin1(), b"Song");
        assert_eq!(sent.artist.as_latin1(), b"Band");
        assert_eq!(desk.tick(ms(1_100), &mut |_| {}).media_info, None);

        desk.on_session_begin();
        assert!(desk.tick(ms(1_200), &mut |_| {}).media_info.is_some());
    }

    #[test]
    fn control_labels_follow_the_bindings_once_per_session() {
        let mut desk = runtime(Arc::default());
        desk.on_session_begin();
        let labels = desk.tick(ms(0), &mut |_| {}).controls.unwrap();
        assert_eq!(labels.rotate.as_latin1(), b"Volume");
        assert_eq!(labels.press.as_latin1(), b"Play/Pause");
        assert_eq!(labels.hold.as_latin1(), b"Mute");
        let buttons = labels.buttons.map(|b| b.as_latin1().to_vec());
        assert_eq!(
            buttons,
            [
                b"Previous".to_vec(),
                b"Play/Pause".to_vec(),
                b"Next".to_vec()
            ]
        );
        assert_eq!(desk.tick(ms(100), &mut |_| {}).controls, None, "unchanged");
        desk.on_session_begin();
        assert!(desk.tick(ms(200), &mut |_| {}).controls.is_some());

        let launch = Action::Launch("/Applications/Spotify.app".into());
        assert_eq!(launch.label(), "Spotify", "an app name, not a path");
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
        let press = |button| LogicalInput::ButtonPress {
            button,
            gesture_id: 4,
        };
        assert_eq!(
            bound_action(&bindings, &press(0)),
            Some(&Action::PreviousTrack)
        );
        assert_eq!(bound_action(&bindings, &press(2)), Some(&Action::NextTrack));
        assert_eq!(bound_action(&bindings, &press(3)), None, "no such button");
        let hold = LogicalInput::ButtonHold {
            button: 0,
            gesture_id: 5,
        };
        assert_eq!(bound_action(&bindings, &hold), None, "unbound by default");
        let mut bound = Bindings::default();
        bound.buttons[0].hold = Slot::bound(Action::ToggleMute);
        assert_eq!(bound_action(&bound, &hold), Some(&Action::ToggleMute));
    }

    /// The built-ins with `edit` applied to their file, as the device thread would receive it.
    fn config(edit: impl FnOnce(&mut crate::config::ConfigFile)) -> ResolvedConfig {
        let mut file = crate::config::ConfigFile::default();
        edit(&mut file);
        crate::config::resolve::resolve(profile::builtins(), &file).unwrap()
    }

    fn override_of(
        file: &mut crate::config::ConfigFile,
        id: crate::config::ProfileId,
    ) -> &mut crate::config::ProfileOverride {
        file.profiles.entry(id).or_default()
    }

    #[test]
    fn apply_config_resends_the_labels_even_when_they_are_identical() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        assert!(desk.tick(ms(0), &mut |_| {}).controls.is_some());
        assert_eq!(
            desk.tick(ms(10), &mut |_| {}).controls,
            None,
            "nothing changed"
        );
        // Hold is not on the device legend, so these labels do not change.
        let hold_only = config(|file| {
            override_of(file, crate::config::ProfileId::General).hold =
                Some(crate::config::SlotSpec {
                    action: Some(crate::config::ActionSpec::PlayPause),
                    label: None,
                });
        });
        desk.apply_config(&hold_only, &mut |_| {});
        assert_eq!(desk.bindings().hold.action, Some(Action::PlayPause));
        let again = desk.tick(ms(20), &mut |_| {}).controls;
        assert_eq!(again, Some(desk.labels()), "re-sent although identical");
        assert_eq!(
            desk.tick(ms(30), &mut |_| {}).controls,
            None,
            "and only once"
        );
    }

    #[test]
    fn apply_config_swaps_labels_live_and_keeps_the_pin() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        let _ = desk.tick(ms(0), &mut |_| {});
        desk.on_input(
            &LogicalInput::ButtonHold {
                button: profile::PIN_BUTTON,
                gesture_id: 1,
            },
            ms(10),
            &mut |_| {},
        );
        assert!(desk.context().pinned());
        let renamed = config(|file| {
            override_of(file, crate::config::ProfileId::General).buttons[0].press =
                Some(crate::config::SlotSpec {
                    action: Some(crate::config::ActionSpec::NextTrack),
                    label: Some("Skip".into()),
                });
        });
        desk.apply_config(&renamed, &mut |_| {});
        assert!(desk.context().pinned(), "the pin survives");
        let labels = desk.tick(ms(20), &mut |_| {}).controls.unwrap();
        assert_eq!(labels.buttons.map(text), ["Skip", "Play/Pause", "Next"]);
    }

    #[test]
    fn a_gesture_in_progress_keeps_its_keys_after_a_rebind() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        focus(&fg, "chrome.exe");
        let _ = desk.tick(ms(0), &mut |_| {});
        let _ = desk.tick(ms(400), &mut |_| {});
        assert!(!desk.on_input(
            &LogicalInput::GestureStarted { gesture_id: 1 },
            ms(500),
            &mut |_| {}
        ));
        let rebound = config(|file| {
            override_of(file, crate::config::ProfileId::Browser).rotate =
                Some(crate::config::RotateSpec::Shortcuts {
                    cw: "Ctrl+Right".into(),
                    ccw: "Ctrl+Left".into(),
                    label: "Seek".into(),
                });
        });
        desk.apply_config(&rebound, &mut |_| {});
        assert_eq!(text(desk.labels().rotate), "Seek");
        let detent = LogicalInput::Detent {
            gesture_id: 1,
            direction: Direction::Cw,
        };
        assert!(!desk.on_input(&detent, ms(520), &mut |_| {}));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while sent(&synth).is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(ms(1));
        }
        assert_eq!(sent(&synth), ["Ctrl+Tab"], "the keys it began with");
        let _ = desk.on_input(
            &LogicalInput::GestureEnded { gesture_id: 1 },
            ms(600),
            &mut |_| {},
        );
        // The next gesture picks up the new binding.
        let _ = desk.on_input(
            &LogicalInput::GestureStarted { gesture_id: 2 },
            ms(700),
            &mut |_| {},
        );
        let detent = LogicalInput::Detent {
            gesture_id: 2,
            direction: Direction::Cw,
        };
        let _ = desk.on_input(&detent, ms(720), &mut |_| {});
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while sent(&synth).len() < 2 && std::time::Instant::now() < deadline {
            std::thread::sleep(ms(1));
        }
        assert_eq!(sent(&synth), ["Ctrl+Tab", "Ctrl+Right"]);
    }

    #[test]
    fn a_bound_button_hold_runs_and_the_middle_hold_still_pins() {
        let fg = Arc::new(FakeForeground::default());
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let mut desk = desk_with(&fg, &synth);
        let bound = config(|file| {
            override_of(file, crate::config::ProfileId::General).buttons[0].hold =
                Some(crate::config::SlotSpec {
                    action: Some(crate::config::ActionSpec::Shortcut {
                        keys: "Ctrl+Shift+K".into(),
                    }),
                    label: None,
                });
        });
        desk.apply_config(&bound, &mut |_| {});
        let hold = |button| LogicalInput::ButtonHold {
            button,
            gesture_id: 1,
        };
        desk.on_input(&hold(0), ms(10), &mut |_| {});
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while sent(&synth).is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(ms(1));
        }
        assert_eq!(sent(&synth), ["Ctrl+Shift+K"]);
        assert!(!desk.context().pinned());
        desk.on_input(&hold(1), ms(20), &mut |_| {});
        assert!(desk.context().pinned(), "the middle Hold is still CyclePin");
        assert_eq!(sent(&synth).len(), 1);
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn recovery_lost_status_refreshes_within_the_restore_budget() {
        let mut publisher = StatusPublisher::default();
        publisher.set_mode(DisplayMode::Volume);
        let _lost = publisher.due(None, Duration::ZERO).unwrap();
        let retried = (1..10).find_map(|second| publisher.due(None, Duration::from_secs(second)));
        assert_eq!(retried.map(|status| status.mode), Some(DisplayMode::Volume));
    }
}
