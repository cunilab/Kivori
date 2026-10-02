//! The production device runtime (T074): the real run loop, written against the four ports.
//!
//! This is **the** firmware behaviour. It is not a harness, not a self-test, and not simulation-specific:
//! the physical binary and the Wokwi `wokwi-runtime` mode call the same [`run`] with the same [`Runtime`],
//! differing only in which [`Clock`], [`Transport`], [`InputSource`], and [`DisplaySink`] instances they
//! hand it and which panel profile built the sink. There is deliberately no second loop implementation to
//! drift.
//!
//! Each tick, in order:
//!
//! 1. finish boot once — `booting` → `offline`, the device-originated transition (FR-014/015);
//! 2. drain inbound bytes through the real decoder, sequence policy, and [`Dispatcher`], which answers
//!    `Hello`/`Ping`/`SetState` and drops malformed frames without side effects (SC-008); any session
//!    boundary — `Bye`, a transport failure, or a fresh `Hello` with no prior `Bye` — also resets the
//!    rotary decoder/gesture state so no gesture survives into a new (or recovered) session;
//! 3. sample physical input once, decode validated detents, and emit semantic `InputEvent`s — gated by
//!    the negotiated `PHYSICAL_INPUT_V1` capability and an accepted session inside the dispatcher;
//! 4. render the current state through the shared renderer, flushing only changed tiles (FR-013);
//! 5. emit at most one safe `Diagnostic` (allowlisted category + code — never payload bytes);
//! 6. emit `Health` on a fixed interval.
//!
//! The loop never returns and never panics on bad input: a transport failure is reported as a diagnostic
//! and the link drops to `offline`, from which a new `Hello` can bring the session back up.
//!
//! # What running this proves, and what it does not
//!
//! Executing it in a simulator exercises the real protocol, lifecycle, renderer, and tile-output paths on
//! the target ISA. It says nothing about the physical panel — its controller, offsets, orientation, colour
//! order, and backlight are supplied from outside this module and remain unconfirmed.

use crate::health::{build_diagnostic, build_health, DeviceDiagnostic};
use crate::input::button::{ButtonEvent, ButtonGesture};
use crate::input::gesture::{RotaryEvent, RotaryGesture};
use crate::input::quadrature::QuadratureDecoder;
use crate::ports::{Clock, DisplaySink, InputSource, Transport};
use crate::proto::{DeviceIdentity, Dispatcher};
use crate::render::TileRenderer;
use crate::state::{DeviceEvent, DeviceState};
use kivori_assets::AssetBlob;
use kivori_model::desk::{ActionFeedback, DeskStatus, DeskView, DisplayMode};
use kivori_model::input::InputLevels;
use kivori_model::presentation::{PrimaryState, ValueDisplay};
use kivori_model::{CompanionState, ElapsedMs, MascotAnimator};
use kivori_protocol::{Bye, ByeReason, Feedback, InputKind, Message, Nonce, Presentation, Status};

/// Inactivity window, in milliseconds, after which an open rotary gesture ends
/// (docs/product.md, gesture boundary). Firmware-wide: both the production runtime and the host-sim
/// scenario helper (`sim::drive_rotary`) commit to this same boundary.
pub const GESTURE_END_MS: u32 = 250;

/// Input snapshots processed per tick: the physical edge queue plus the closing current sample.
const INPUT_SAMPLES: usize = 129;

/// How far the keycap sinks while the switch is held: the device's own instant acknowledgement,
/// shallower than Happy's 16 px press so it never reads as that state (Q8 pixels).
const PRESS_ACK_Q8: i32 = 8 * 256;

/// Loop timings. Both are integer milliseconds, so behaviour is deterministic (ADR-0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Minimum gap between render passes. A pass still flushes nothing when no tile changed.
    pub frame_interval_ms: ElapsedMs,
    /// Gap between `Health` reports.
    pub health_interval_ms: ElapsedMs,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        // 30 fps is the canonical animation rate (ADR-0003); health once a second is enough for a
        // heartbeat without competing with frames for the FIFO.
        Self {
            frame_interval_ms: 33,
            health_interval_ms: 1000,
        }
    }
}

/// What one [`Runtime::step`] observed. Returned so tests and the Wokwi mode can assert on real work
/// instead of guessing from side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tick {
    /// The state the device holds after this tick.
    pub state: Option<CompanionState>,
    /// A frame was composed on this tick, even if every tile matched the previous frame.
    pub frame_rendered: bool,
    /// Tiles flushed to the display this tick.
    pub tiles_flushed: u32,
    /// A safe diagnostic was transmitted.
    pub diagnostic: Option<DeviceDiagnostic>,
    /// A `Health` report was transmitted.
    pub health_sent: bool,
    /// The transport reported a failure and the link was dropped.
    pub link_dropped: bool,
    /// The recovery hold completed: the caller must reboot the MCU now. Set once; the runtime
    /// already rendered the final frame and queued a `Bye` for an accepted session.
    pub reboot: bool,
    /// Frames the decoder has rejected since boot (cumulative).
    pub rejected_frames: u32,
    /// `HelloAck` replies transmitted since boot (cumulative).
    pub hello_acks: u32,
    /// `Pong` replies transmitted since boot (cumulative).
    pub pongs: u32,
    /// `StateReport` messages transmitted since boot (cumulative).
    pub state_reports: u32,
}

/// A [`DisplaySink`] decorator counting flushes, so the runtime can report tile activity without the sink
/// having to.
struct CountingSink<'s, S> {
    inner: &'s mut S,
    flushes: u32,
    failed: bool,
}

impl<S: DisplaySink> DisplaySink for CountingSink<'_, S> {
    type Error = S::Error;

    fn blit_tile(
        &mut self,
        rect: kivori_model::Rect,
        pixels: &[kivori_model::Rgb565],
    ) -> Result<(), Self::Error> {
        match self.inner.blit_tile(rect, pixels) {
            Ok(()) => {
                self.flushes += 1;
                Ok(())
            }
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }
}

/// Device-side presentation acceptance and local transient expiry.
///
/// Firmware expires the overlay itself so it cannot stick if the host disappears mid-transient,
/// restoring the underlying `primary` (product invariant 50).
#[derive(Debug)]
pub struct PresentationState {
    session: Option<Nonce>,
    last_revision: u32,
    primary: PrimaryState,
    value: Option<ValueDisplay>,
    /// The device-ms timestamp `value` was applied at. Meaningless when `transient_ms == 0`
    /// (persistent) or `value` is `None`.
    applied_at_ms: u32,
    /// `0` = persistent; otherwise `value` expires this many ms after `applied_at_ms`, compared
    /// via `wrapping_sub` (the same idiom `RotaryGesture`'s inactivity window uses) rather than a
    /// precomputed absolute deadline — a precomputed `applied_at_ms + transient_ms` can itself
    /// wrap past `u32::MAX` (~49.7 days of device uptime) while `now_ms` has not yet wrapped,
    /// which would make a plain `now_ms >= deadline` comparison see a small deadline and a huge
    /// `now_ms` and report the overlay expired instantly.
    transient_ms: u16,
}

impl PresentationState {
    /// A fresh presentation state, before any session is accepted.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            session: None,
            last_revision: 0,
            primary: PrimaryState::Idle,
            value: None,
            applied_at_ms: 0,
            transient_ms: 0,
        }
    }

    /// A new compatible host session: rebind identity and reset revision scoping.
    pub fn begin_session(&mut self, session: Nonce) {
        self.session = Some(session);
        self.last_revision = 0;
        self.value = None;
        self.transient_ms = 0;
    }

    /// Ends the current session: no session, no lingering overlay.
    pub fn end_session(&mut self) {
        self.session = None;
        self.last_revision = 0;
        self.value = None;
        self.transient_ms = 0;
    }

    /// Returns true when the presentation was accepted and applied.
    pub fn apply(&mut self, p: &Presentation, now_ms: u32) -> bool {
        if self.session != Some(p.session) {
            return false;
        }
        if p.revision <= self.last_revision {
            return false;
        }
        self.last_revision = p.revision;
        self.primary = p.primary;
        self.value = p.value;
        self.applied_at_ms = now_ms;
        self.transient_ms = if p.value.is_some() { p.transient_ms } else { 0 };
        true
    }

    /// The overlay still in force at `now_ms`, if any.
    #[must_use]
    pub fn value_at(&self, now_ms: u32) -> Option<ValueDisplay> {
        if self.transient_ms == 0 {
            return self.value;
        }
        // Wrap-aware elapsed time (see the `transient_ms` field doc): correct across a device
        // uptime rollover, unlike comparing `now_ms` against a precomputed absolute deadline.
        if now_ms.wrapping_sub(self.applied_at_ms) >= u32::from(self.transient_ms) {
            None
        } else {
            self.value
        }
    }

    /// The underlying truth beneath any transient overlay.
    #[must_use]
    pub const fn primary(&self) -> PrimaryState {
        self.primary
    }
}

impl Default for PresentationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Device-side desk status and action feedback for the current session (M1).
///
/// Session-scoped like [`PresentationState`]: anything from another session is dropped, and a
/// session boundary forgets everything, so the device falls back to the Buddy view with every
/// value unknown rather than showing stale desktop state.
#[derive(Debug, Default)]
pub struct DeskState {
    session: Option<Nonce>,
    status: DeskStatus,
    /// Device-ms the status arrived, to keep its clock running locally.
    status_at_ms: u32,
    feedback: Option<(ActionFeedback, u32)>,
}

impl DeskState {
    /// A new compatible host session: nothing known yet.
    pub fn begin_session(&mut self, session: Nonce) {
        *self = Self {
            session: Some(session),
            ..Self::default()
        };
    }

    /// The session ended: forget everything.
    pub fn end_session(&mut self) {
        *self = Self::default();
    }

    /// Applies a status of the current session. Returns whether it was accepted.
    pub fn apply_status(&mut self, status: &Status, now_ms: u32) -> bool {
        if self.session != Some(status.session) {
            return false;
        }
        self.status = status.status;
        self.status_at_ms = now_ms;
        true
    }

    /// Shows a feedback of the current session, replacing any older one at once.
    pub fn apply_feedback(&mut self, feedback: &Feedback, now_ms: u32) -> bool {
        if self.session != Some(feedback.session) {
            return false;
        }
        self.feedback = Some((
            ActionFeedback {
                action: feedback.action,
                kind: feedback.kind,
            },
            now_ms,
        ));
        true
    }

    /// What to render at `now_ms`: the clock advanced locally, expired feedback dropped
    /// (wrap-safe elapsed comparison, as in [`PresentationState::value_at`]).
    #[must_use]
    pub fn view_at(&self, now_ms: u32, button_down: bool, recovery: Option<u8>) -> DeskView {
        let mut status = self.status;
        status.clock = status
            .clock
            .map(|clock| clock.advanced_by(now_ms.wrapping_sub(self.status_at_ms)));
        let feedback = self
            .feedback
            .filter(|(f, at)| now_ms.wrapping_sub(*at) < f.kind.transient_ms())
            .map(|(f, _)| f);
        DeskView {
            status,
            feedback,
            button_down,
            recovery_percent: recovery,
            elapsed_ms: now_ms,
        }
    }
}

/// The production device runtime: lifecycle, protocol, rendering, and diagnostics.
pub struct Runtime<'a> {
    dispatcher: Dispatcher,
    device: DeviceState,
    renderer: TileRenderer<'a>,
    config: RuntimeConfig,
    booted: bool,
    next_frame_ms: ElapsedMs,
    next_health_ms: ElapsedMs,
    last_rendered: Option<CompanionState>,
    animator: MascotAnimator,
    /// Turns raw quadrature levels into validated logical detents (never raw electrical edges).
    decoder: QuadratureDecoder,
    /// Groups validated detents into gestures (identity + the 250 ms inactivity boundary).
    gesture: RotaryGesture,
    /// Session-scoped acceptance and local expiry of the device-rendered `Presentation` overlay.
    presentation: PresentationState,
    /// Push-switch debounce, Press / Hold and the recovery hold.
    button: ButtonGesture,
    /// Identifier of the last push-switch event sent; session-unique, never 0.
    button_id: u16,
    /// Session-scoped desk status and feedback.
    desk: DeskState,
    /// A switch edge or recovery step wants the panel redrawn on this tick.
    redraw: bool,
    reboot: bool,
    #[cfg(feature = "latency-probe")]
    latency: crate::latency_probe::LatencyProbe,
}

impl<'a> Runtime<'a> {
    /// Creates the runtime for a device advertising `identity`.
    #[must_use]
    pub fn new(identity: DeviceIdentity, config: RuntimeConfig) -> Self {
        Self {
            dispatcher: Dispatcher::new(identity),
            device: DeviceState::new(),
            renderer: TileRenderer::new(),
            config,
            booted: false,
            next_frame_ms: 0,
            next_health_ms: 0,
            last_rendered: None,
            animator: MascotAnimator::new(CompanionState::Booting, 0),
            decoder: QuadratureDecoder::new(),
            gesture: RotaryGesture::new(GESTURE_END_MS),
            presentation: PresentationState::new(),
            button: ButtonGesture::new(),
            button_id: 0,
            desk: DeskState::default(),
            redraw: false,
            reboot: false,
            #[cfg(feature = "latency-probe")]
            latency: crate::latency_probe::LatencyProbe::new(),
        }
    }

    /// Creates a runtime that composes a complete frame before starting display transfers.
    pub fn with_frame_buffer(
        identity: DeviceIdentity,
        config: RuntimeConfig,
        frame_buffer: &'a mut [kivori_model::Rgb565; crate::render::FRAME_PIXELS],
    ) -> Self {
        Self {
            renderer: TileRenderer::with_frame_buffer(frame_buffer),
            ..Self::new(identity, config)
        }
    }

    /// The companion state the device currently holds.
    #[must_use]
    pub const fn state(&self) -> CompanionState {
        self.device.current()
    }

    /// Frames the decoder has rejected this session.
    #[must_use]
    pub const fn rejected_frames(&self) -> u32 {
        self.dispatcher.rejected_frames()
    }

    /// The on-panel latency readout (development-only `latency-probe`).
    #[cfg(feature = "latency-probe")]
    #[must_use]
    pub const fn latency_readout(&self) -> crate::latency_probe::Readout {
        self.latency.readout()
    }

    /// Runs one tick of the production loop.
    ///
    /// Never fails: a transport error becomes a `LinkLost` diagnostic and an `offline` transition, because a
    /// device that gives up on a bad cable is worse than one that waits for the host to come back.
    pub fn step<C, T, I, D>(
        &mut self,
        clock: &C,
        transport: &mut T,
        input: &mut I,
        display: &mut D,
        blob: &AssetBlob,
    ) -> Tick
    where
        C: Clock,
        T: Transport,
        I: InputSource,
        D: DisplaySink,
    {
        let mut tick = Tick::default();
        let now = clock.now_ms();

        // 1. The device owns `booting` at power-on and falls back to `offline` with no host present.
        if !self.booted {
            self.booted = true;
            tick.state = self.device.apply(DeviceEvent::BootComplete);
        }

        // 2. Inbound: real framing, CRC, sequence policy, and dispatch. Malformed frames are dropped
        //    inside the dispatcher and surface below as a diagnostic.
        if self
            .dispatcher
            .poll(transport, &mut self.device, now)
            .is_err()
        {
            let _ = self.device.apply(DeviceEvent::LinkDown);
            self.dispatcher.note_diagnostic(DeviceDiagnostic::LinkLost);
            // A transport failure is a session boundary too, even with no `Bye`: clear the
            // accepted session/negotiated capability so a stale gesture cannot keep emitting
            // once the link recovers.
            self.dispatcher.link_lost();
            tick.link_dropped = true;
        }
        // A `Bye`, a transport failure, or a new `Hello` this poll closed/opened a session: a
        // gesture (or partial motion) from the old session must never complete in a new one
        // (no-stale-replay invariant).
        if self.dispatcher.take_session_ended() {
            self.decoder.reset();
            self.gesture.reset();
            // A session boundary is also a presentation-scoping boundary: `revision` is strictly
            // increasing WITHIN a session and resets with it, so a restarted desktop starting
            // again at revision 1 is never rejected as stale traffic from the old, higher-revision
            // session. `accepted_session()` tells us whether this boundary opened a new session
            // (`Some`) or closed one (`None`).
            match self.dispatcher.accepted_session() {
                Some(session) => {
                    self.presentation.begin_session(session);
                    self.desk.begin_session(session);
                }
                None => {
                    self.presentation.end_session();
                    self.desk.end_session();
                }
            }
            // A half-done press must not fire into the next session; a running recovery hold is
            // not session-scoped and keeps going (invariant 24).
            self.button.reset();
            self.button_id = 0;
            self.redraw = true;
        }
        if let Some(status) = self.dispatcher.take_status() {
            self.redraw |= self.desk.apply_status(&status, now);
        }
        if let Some(feedback) = self.dispatcher.take_feedback() {
            self.redraw |= self.desk.apply_feedback(&feedback, now);
        }

        // Apply any `Presentation` the dispatcher accepted this poll. Capability negotiation and
        // session/revision freshness are already enforced by the dispatcher and `PresentationState`
        // themselves, so this is unconditional.
        let mut presentation_applied = false;
        if let Some(presentation) = self.dispatcher.take_presentation() {
            presentation_applied = self.presentation.apply(&presentation, now);
            #[cfg(feature = "latency-probe")]
            if presentation_applied {
                self.latency.on_presentation(now);
            }
        }

        // 3. Physical input: sample once per tick, turn validated detents into gestures, and emit
        //    semantic `InputEvent`s. `send_input_event` itself gates on capability/session, so this
        //    stays silent until the desktop has negotiated `PHYSICAL_INPUT_V1`.
        //    Each snapshot first closes the idle boundary at its own capture time, so a loop that
        //    got back late neither splits a gesture whose detents were inside the window nor
        //    extends one that had already gone quiet; the boundary is closed at `now` last.
        let mut samples = heapless::Vec::<(InputLevels, ElapsedMs), INPUT_SAMPLES>::new();
        input.drain(now, &mut |levels, at_ms| {
            // Overflow drops the OLDEST levels, so the latest edges (a switch release) keep their
            // timing; the decoder counts the gap as one invalid transition, which can lose a
            // detent but never invents one.
            if samples.is_full() {
                samples.remove(0);
            }
            let _ = samples.push((levels, at_ms));
        });
        for (levels, at_ms) in samples {
            self.close_idle_gesture(transport, at_ms);
            self.on_levels(transport, levels, at_ms);
        }
        self.close_idle_gesture(transport, now);

        // Resolve changes immediately after protocol handling. Reporting remains semantic and does
        // not wait for the visual transition. Pixel hashes still detect which bands changed.
        let state = self.device.current();
        if self.animator.target() != state {
            self.animator.set_state(state, now);
        }
        if let Some(action) = self.dispatcher.take_mascot_action() {
            self.animator
                .trigger_action(action.action, action.personality, action.seed, now);
        }
        // 4. Render on the frame cadence, or at once when fresh feedback arrived, so detent ->
        //    panel latency does not wait out the frame interval (validation row 3.14). Only changed
        //    tiles reach the panel (FR-013).
        // A switch edge also renders at once: the press acknowledgement is the device's own
        // < 50 ms feedback and must not wait out the frame interval.
        if let Some(event) = self.button.poll(now) {
            self.on_button(transport, event, now);
        }
        let redraw = core::mem::take(&mut self.redraw);
        if reached(now, self.next_frame_ms) || presentation_applied || redraw {
            if reached(now, self.next_frame_ms) {
                self.next_frame_ms =
                    next_frame_deadline(self.next_frame_ms, now, self.config.frame_interval_ms);
            }
            tick.frame_rendered = true;
            let state = self.device.current();
            if self.last_rendered != Some(state) {
                self.last_rendered = Some(state);
            }
            let mut counting = CountingSink {
                inner: display,
                flushes: 0,
                failed: false,
            };
            let view = self.desk.view_at(
                now,
                self.button.is_down(),
                self.button.recovery_percent(now),
            );
            let mut pose = self.animator.pose_at(now);
            if view.button_down && view.status.mode == DisplayMode::Buddy {
                pose.press_q8 = pose.press_q8.max(PRESS_ACK_Q8);
            }
            // The transient overlay, if still in force, is composited on top of the mascot pose;
            // it expires locally back to `None` so the panel shows the plain pose once it lapses,
            // with no host timer or round trip needed.
            let overlay = self.presentation.value_at(now);
            #[cfg(feature = "latency-probe")]
            self.renderer.set_latency_readout(self.latency.readout());
            let outcome =
                self.renderer
                    .render_frame(blob, state, &pose, overlay, &view, &mut counting);
            tick.tiles_flushed = counting.flushes;
            // Tile writes block until the SPI DMA transfer completes, so this clock read is
            // "frame fully flushed", not "presentation received".
            #[cfg(feature = "latency-probe")]
            if outcome.is_ok() {
                self.latency.on_frame_flushed(clock.now_ms());
            }
            let failed = counting.failed;
            if outcome.is_err() {
                // A sink failure is reportable; a missing scene is a build-time bug that must not spin.
                if failed {
                    self.dispatcher
                        .note_diagnostic(DeviceDiagnostic::DisplayFault);
                }
                self.renderer.invalidate();
            }
        }

        // 5. At most one safe diagnostic per tick, category + code only (ADR-0005).
        if let Some(diagnostic) = self.dispatcher.take_diagnostic() {
            let message = Message::Diagnostic(build_diagnostic(diagnostic));
            if self.dispatcher.emit(transport, &message).is_ok() {
                tick.diagnostic = Some(diagnostic);
            }
        }

        // 6. Heartbeat health on a fixed cadence.
        if reached(now, self.next_health_ms) {
            self.next_health_ms = now.wrapping_add(self.config.health_interval_ms);
            let message = Message::Health(build_health(free_bytes()));
            if self.dispatcher.emit(transport, &message).is_ok() {
                tick.health_sent = true;
            }
        }

        if tick.state.is_none() {
            tick.state = Some(self.device.current());
        }
        tick.reboot = self.reboot;
        tick.rejected_frames = self.dispatcher.rejected_frames();
        tick.hello_acks = self.dispatcher.hello_acks();
        tick.pongs = self.dispatcher.pongs();
        tick.state_reports = self.dispatcher.state_reports();
        tick
    }

    /// Ends the open rotary gesture if its inactivity window has passed by `at_ms`.
    fn close_idle_gesture<T: Transport>(&mut self, transport: &mut T, at_ms: ElapsedMs) {
        if let Some(RotaryEvent::GestureEnded { gesture_id }) = self.gesture.poll(at_ms) {
            self.dispatcher
                .send_input_event(transport, gesture_id, InputKind::GestureEnded, at_ms);
        }
    }

    /// Decodes one level snapshot captured at `at_ms` and emits the semantic events it completes.
    fn on_levels<T: Transport>(
        &mut self,
        transport: &mut T,
        levels: InputLevels,
        at_ms: ElapsedMs,
    ) {
        let rotary_open = self.gesture.is_open();
        for event in self
            .button
            .update(levels.sw, at_ms, rotary_open)
            .into_iter()
            .flatten()
        {
            self.on_button(transport, event, at_ms);
        }
        // The decoder always tracks the phase, but while the switch is down its detents are
        // swallowed: one gesture owns input, and rotation never cancels or acts during a hold
        // (invariant 33).
        let Some(direction) = self.decoder.update(levels.a, levels.b) else {
            return;
        };
        // The raw level counts too: a knob wobbles as it is pressed, and a detent inside the
        // switch's debounce window must not change the volume either.
        if levels.sw || self.button.is_down() {
            return;
        }
        let (started, detent) = self.gesture.on_detent(direction, at_ms);
        if let Some(RotaryEvent::GestureStarted { gesture_id }) = started {
            self.dispatcher.send_input_event(
                transport,
                gesture_id,
                InputKind::GestureStarted,
                at_ms,
            );
        }
        if let RotaryEvent::Detent {
            gesture_id,
            direction,
        } = detent
        {
            let _sent = self.dispatcher.send_input_event(
                transport,
                gesture_id,
                InputKind::Detent(direction),
                at_ms,
            );
            #[cfg(feature = "latency-probe")]
            if _sent {
                self.latency.on_detent(at_ms);
            }
        }
    }

    /// Acts on one push-switch event. Press / Hold go to the desktop (and only there: the
    /// desktop owns action meaning); everything else is local.
    fn on_button<T: Transport>(&mut self, transport: &mut T, event: ButtonEvent, at_ms: ElapsedMs) {
        self.redraw = true;
        let kind = match event {
            ButtonEvent::Press => InputKind::Press,
            ButtonEvent::Hold => InputKind::Hold,
            ButtonEvent::Reboot => {
                if !self.reboot {
                    self.reboot = true;
                    // Tell an accepted session it is ending on purpose; the link drop follows.
                    if self.dispatcher.accepted_session().is_some() {
                        let bye = Message::Bye(Bye {
                            reason: ByeReason::Shutdown,
                        });
                        let _ = self.dispatcher.emit(transport, &bye);
                    }
                }
                return;
            }
            ButtonEvent::Down | ButtonEvent::Released | ButtonEvent::RecoveryStarted => return,
        };
        self.button_id = self.button_id.wrapping_add(1).max(1);
        self.dispatcher
            .send_button_event(transport, self.button_id, kind, at_ms);
    }
}

fn next_frame_deadline(
    previous_deadline_ms: ElapsedMs,
    now_ms: ElapsedMs,
    interval_ms: ElapsedMs,
) -> ElapsedMs {
    let interval_ms = interval_ms.max(1);
    let elapsed_intervals = now_ms.wrapping_sub(previous_deadline_ms) / interval_ms;
    previous_deadline_ms.wrapping_add(elapsed_intervals.wrapping_add(1).wrapping_mul(interval_ms))
}

/// `now` is at or past `deadline` on the wrapping millisecond clock (deadlines are always less
/// than ~24 days ahead, so the signed difference is exact).
const fn reached(now: ElapsedMs, deadline: ElapsedMs) -> bool {
    now.wrapping_sub(deadline) as i32 >= 0
}

/// Free SRAM in bytes.
///
/// Measuring genuine heap/stack headroom needs linker symbols and a stack-watermark scheme that only means
/// anything on real silicon, so this reports `0` — "not measured" — rather than a fabricated number that
/// would read as a real measurement on a dashboard. Producing a true figure is part of the on-device
/// validation work (T116/T119), not something simulation can establish.
const fn free_bytes() -> u32 {
    0
}

/// Runs the production loop forever.
///
/// The physical binary and the Wokwi runtime mode both call this; only the injected ports differ.
// One argument per port/config/observer — bundling them would obscure which port is which at every
// call site for no real benefit here.
#[allow(clippy::too_many_arguments)]
pub fn run<C, T, I, D>(
    identity: DeviceIdentity,
    config: RuntimeConfig,
    clock: &C,
    transport: &mut T,
    input: &mut I,
    display: &mut D,
    blob: &AssetBlob,
    observe: impl FnMut(&Tick, &mut T),
) -> !
where
    C: Clock,
    T: Transport,
    I: InputSource,
    D: DisplaySink,
{
    let mut runtime = Runtime::new(identity, config);
    run_loop(
        &mut runtime,
        clock,
        transport,
        input,
        display,
        blob,
        observe,
    )
}

/// Runs the same production loop with caller-owned frame staging memory.
#[allow(clippy::too_many_arguments)]
pub fn run_buffered<C, T, I, D>(
    identity: DeviceIdentity,
    config: RuntimeConfig,
    clock: &C,
    transport: &mut T,
    input: &mut I,
    display: &mut D,
    blob: &AssetBlob,
    frame_buffer: &mut [kivori_model::Rgb565; crate::render::FRAME_PIXELS],
    observe: impl FnMut(&Tick, &mut T),
) -> !
where
    C: Clock,
    T: Transport,
    I: InputSource,
    D: DisplaySink,
{
    let mut runtime = Runtime::with_frame_buffer(identity, config, frame_buffer);
    run_loop(
        &mut runtime,
        clock,
        transport,
        input,
        display,
        blob,
        observe,
    )
}

fn run_loop<C, T, I, D>(
    runtime: &mut Runtime<'_>,
    clock: &C,
    transport: &mut T,
    input: &mut I,
    display: &mut D,
    blob: &AssetBlob,
    mut observe: impl FnMut(&Tick, &mut T),
) -> !
where
    C: Clock,
    T: Transport,
    I: InputSource,
    D: DisplaySink,
{
    loop {
        let tick = runtime.step(clock, transport, input, display, blob);
        // The observer receives the transport so a simulation mode can emit text markers over the same
        // link the protocol uses. The production binary passes a closure that does nothing.
        observe(&tick, transport);
    }
}
