//! Deterministic mascot motion and state-transition resolution.
//!
//! The controller is deliberately small and allocation-free so desktop preview and firmware can
//! resolve the same pose from the same integer timestamp. It owns no sprites or rendering state.

use crate::{CompanionState, ElapsedMs, Point};
use serde::{Deserialize, Serialize};

/// Standard duration for changing between expressive mascot states.
pub const MASCOT_TRANSITION_MS: u32 = 350;
/// Duration for entering the slower sleeping state.
pub const MASCOT_SLEEP_TRANSITION_MS: u32 = 600;
/// Fixed-point identity scale (`1.0`) in Q8 units.
pub const SCALE_Q8_ONE: u16 = 256;
/// Bottom-centre pivot shared by the compiled body and face layers.
pub const MASCOT_ANCHOR: Point = Point::new(120, 208);
/// Stable default seed used by firmware and native preview for ambient facial motion.
pub const DEFAULT_IDLE_SEED: u32 = 0x4B49_564F;

const IDLE_EPOCH_MS: u32 = 6_000;
const BLINK_HALF_WINDOW_MS: u32 = 80;

/// User-selectable temperament controlling mascot motion strength and autonomous cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum MascotPersonality {
    /// Warm, curious, moderately animated default.
    #[default]
    Cozy,
    /// Frequent, exaggerated playful movement.
    Playful,
    /// Quiet, attentive movement with restrained amplitude.
    Calm,
}

/// Direct social interactions accepted by the companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MascotAction {
    /// Friendly forward acknowledgment.
    Greet,
    /// Relaxed affectionate response.
    Pet,
    /// Short laughing wiggle.
    Tickle,
    /// Wide-eyed recoil and recovery.
    Surprise,
    /// Slow reassuring response.
    Comfort,
}

impl MascotAction {
    /// Duration of this complete transient reaction in milliseconds.
    #[must_use]
    pub const fn duration_ms(self) -> u32 {
        match self {
            Self::Greet => 1_800,
            Self::Pet => 2_200,
            Self::Tickle => 2_400,
            Self::Surprise => 1_600,
            Self::Comfort => 2_600,
        }
    }
}

/// Visual face selected independently from the mascot's semantic work/connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MascotExpression {
    /// Power-on face.
    Booting = 0,
    /// Neutral friendly face.
    Idle = 1,
    /// Existing happy face.
    Happy = 2,
    /// Existing focused face.
    Busy = 3,
    /// Existing sleeping face.
    Sleeping = 4,
    /// Existing disconnected face.
    Offline = 5,
    /// Bright greeting face.
    Delighted = 6,
    /// Relaxed affectionate face.
    Affectionate = 7,
    /// Closed-eye laugh.
    Laughing = 8,
    /// Wide-eyed surprised face.
    Surprised = 9,
    /// Soft reassuring face.
    Reassuring = 10,
}

impl MascotExpression {
    /// Stable zero-based sprite-sheet frame index.
    #[must_use]
    pub const fn frame(self) -> u16 {
        self as u16
    }

    /// Stable eye sprite-sheet frame. Expressive combinations reuse compact source parts.
    #[must_use]
    pub const fn eye_frame(self) -> u16 {
        match self {
            Self::Booting => 0,
            Self::Idle | Self::Reassuring | Self::Delighted | Self::Surprised => 1,
            Self::Happy | Self::Laughing => 2,
            Self::Busy => 3,
            Self::Sleeping => 4,
            Self::Offline => 5,
            Self::Affectionate => 6,
        }
    }

    /// Stable mouth sprite-sheet frame. Expressive combinations reuse compact source parts.
    #[must_use]
    pub const fn mouth_frame(self) -> u16 {
        match self {
            Self::Booting | Self::Surprised => 0,
            Self::Idle => 1,
            Self::Happy | Self::Delighted | Self::Affectionate => 2,
            Self::Busy | Self::Reassuring => 3,
            Self::Sleeping => 4,
            Self::Offline => 5,
            Self::Laughing => 6,
        }
    }

    /// Base expression for one semantic companion state.
    #[must_use]
    pub const fn for_state(state: CompanionState) -> Self {
        match state {
            CompanionState::Booting => Self::Booting,
            CompanionState::Idle => Self::Idle,
            CompanionState::Happy => Self::Happy,
            CompanionState::Busy => Self::Busy,
            CompanionState::Sleeping => Self::Sleeping,
            CompanionState::Offline => Self::Offline,
        }
    }
}

/// A fully-resolved mascot pose suitable for direct composition.
///
/// Body coordinates use Q8 pixels to keep eased motion continuous until the renderer converts them
/// to output pixels. `state_weights` are indexed by [`CompanionState::index`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MascotPose {
    /// Translation from the canonical artwork position, in Q8 pixels.
    pub body_offset_q8: (i32, i32),
    /// Uniform body scale in Q8 units.
    pub body_scale_q8: u16,
    /// Whole-mascot opacity (`0..=255`).
    pub opacity: u8,
    /// Vertical eye scale in Q8 units; blinking squashes this value.
    pub eyes_scale_y_q8: u16,
    /// Extra vertical scale for the left eye, enabling winks without changing the right eye.
    pub left_eye_scale_y_q8: u16,
    /// Extra vertical scale for the right eye, enabling winks without changing the left eye.
    pub right_eye_scale_y_q8: u16,
    /// Shared eye translation from the canonical face position, in Q8 pixels.
    pub eye_offset_q8: (i32, i32),
    /// Face expression selected independently from semantic companion state.
    pub expression: MascotExpression,
    /// Facial-expression blend weights, in companion-state declaration order.
    pub state_weights: [u8; 6],
}

impl MascotPose {
    /// The neutral opaque pose with one fully selected face expression.
    #[must_use]
    pub const fn for_state(state: CompanionState) -> Self {
        let mut state_weights = [0; 6];
        state_weights[state.index()] = u8::MAX;
        Self {
            body_offset_q8: (0, 0),
            body_scale_q8: SCALE_Q8_ONE,
            opacity: u8::MAX,
            eyes_scale_y_q8: SCALE_Q8_ONE,
            left_eye_scale_y_q8: SCALE_Q8_ONE,
            right_eye_scale_y_q8: SCALE_Q8_ONE,
            eye_offset_q8: (0, 0),
            expression: MascotExpression::for_state(state),
            state_weights,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveAction {
    action: MascotAction,
    personality: MascotPersonality,
    seed: u32,
    started_at: ElapsedMs,
    from: MascotPose,
}

/// Stateful controller which preserves the resolved pose when its target state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MascotAnimator {
    target: CompanionState,
    transition_started_at: ElapsedMs,
    transition_duration_ms: u32,
    from: MascotPose,
    action: Option<ActiveAction>,
    idle_seed: u32,
}

// Hard firmware budget: the controller and resolved pose never become framebuffer-sized.
const _: () = assert!(
    core::mem::size_of::<MascotAnimator>() + core::mem::size_of::<MascotPose>() <= 8 * 1024
);

impl MascotAnimator {
    /// Starts an animator already settled in `state` at `now_ms`.
    #[must_use]
    pub const fn new(state: CompanionState, now_ms: ElapsedMs) -> Self {
        Self::new_seeded(state, now_ms, DEFAULT_IDLE_SEED)
    }

    /// Starts an animator with an explicit deterministic ambient-motion seed.
    #[must_use]
    pub const fn new_seeded(state: CompanionState, now_ms: ElapsedMs, idle_seed: u32) -> Self {
        Self {
            target: state,
            transition_started_at: now_ms,
            transition_duration_ms: 0,
            from: MascotPose::for_state(state),
            action: None,
            idle_seed,
        }
    }

    /// The state toward which this animator is moving.
    #[must_use]
    pub const fn target(&self) -> CompanionState {
        self.target
    }

    /// Retargets the animator while retaining its exact currently-resolved pose.
    pub fn set_state(&mut self, state: CompanionState, now_ms: ElapsedMs) {
        if state == self.target {
            return;
        }
        let current = self.pose_at(now_ms);
        self.target = state;
        self.transition_started_at = now_ms;
        self.transition_duration_ms = if state == CompanionState::Sleeping {
            MASCOT_SLEEP_TRANSITION_MS
        } else {
            MASCOT_TRANSITION_MS
        };
        self.from = current;
        self.action = None;
    }

    /// Starts a social reaction. A new action replaces any earlier reaction immediately.
    ///
    /// Reactions give way to meaningful states (PRD §9.5): outside
    /// [`CompanionState::allows_reaction`] the request is ignored and `false` is returned.
    pub fn trigger_action(
        &mut self,
        action: MascotAction,
        personality: MascotPersonality,
        seed: u32,
        now_ms: ElapsedMs,
    ) -> bool {
        if !self.target.allows_reaction() {
            return false;
        }
        let from = self.pose_at(now_ms);
        self.action = Some(ActiveAction {
            action,
            personality,
            seed,
            started_at: now_ms,
            from,
        });
        true
    }

    /// Resolves the target loop and any in-flight eased transition at `now_ms`.
    #[must_use]
    pub fn pose_at(&self, now_ms: ElapsedMs) -> MascotPose {
        let target = target_pose(
            self.target,
            now_ms,
            now_ms.saturating_sub(self.transition_started_at),
            self.idle_seed,
        );
        let elapsed = now_ms.saturating_sub(self.transition_started_at);
        let mut pose = if self.transition_duration_ms == 0 || elapsed >= self.transition_duration_ms
        {
            target
        } else {
            let mut transitioning = blend_pose(
                self.from,
                target,
                smoothstep_q8(elapsed, self.transition_duration_ms),
            );
            transitioning.eyes_scale_y_q8 = transitioning
                .eyes_scale_y_q8
                .min(transition_blink_scale(elapsed, self.transition_duration_ms));
            transitioning
        };
        if let Some(action) = self.action {
            let action_elapsed = now_ms.saturating_sub(action.started_at);
            if action_elapsed < action.action.duration_ms() {
                apply_action(&mut pose, self.target, action, action_elapsed);
            }
        }
        pose
    }
}

impl CompanionState {
    /// Whether a social reaction may play over this state (PRD §9.5). Busy, Booting and Offline are
    /// meaningful states a reaction must not mask. Sleeping allows the deliberate gentle, sleepy
    /// response; autonomous self-play is stricter still (see the desktop companion director).
    #[must_use]
    pub const fn allows_reaction(self) -> bool {
        matches!(self, Self::Idle | Self::Happy | Self::Sleeping)
    }

    /// Stable zero-based index used by mascot expression weights.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Booting => 0,
            Self::Idle => 1,
            Self::Happy => 2,
            Self::Busy => 3,
            Self::Sleeping => 4,
            Self::Offline => 5,
        }
    }
}

fn target_pose(
    state: CompanionState,
    now_ms: ElapsedMs,
    state_age_ms: ElapsedMs,
    idle_seed: u32,
) -> MascotPose {
    let mut pose = MascotPose::for_state(state);
    let (x, y, scale) = match state {
        CompanionState::Booting => (
            0,
            0,
            248 + (u32::from(smoothstep_q8(state_age_ms.min(800), 800)) * 8 / 256) as u16,
        ),
        CompanionState::Idle => (0, 0, SCALE_Q8_ONE),
        CompanionState::Happy => {
            let bounce = positive_wave(now_ms, 900, 1_536);
            let scale = positive_wave(now_ms, 900, 4);
            (0, -bounce, (SCALE_Q8_ONE as i32 + scale) as u16)
        }
        CompanionState::Busy => {
            let sway = centred_wave(now_ms, 1_600, 768);
            (sway, 0, SCALE_Q8_ONE)
        }
        CompanionState::Sleeping => (0, 0, SCALE_Q8_ONE),
        CompanionState::Offline => (0, i32::from(SCALE_Q8_ONE), SCALE_Q8_ONE),
    };
    pose.body_offset_q8 = (x, y);
    pose.body_scale_q8 = scale;
    pose.eyes_scale_y_q8 = match state {
        CompanionState::Sleeping => SCALE_Q8_ONE,
        CompanionState::Booting => {
            let open = smoothstep_q8(state_age_ms.min(300), 300);
            48 + (u32::from(SCALE_Q8_ONE - 48) * u32::from(open) / 256) as u16
        }
        CompanionState::Idle => idle_life(&mut pose, now_ms, idle_seed),
        CompanionState::Happy | CompanionState::Busy | CompanionState::Offline => {
            varied_blink_scale(now_ms, idle_seed ^ state.index() as u32)
        }
    };
    pose
}

fn apply_action(
    pose: &mut MascotPose,
    state: CompanionState,
    active: ActiveAction,
    elapsed_ms: ElapsedMs,
) {
    let duration = active.action.duration_ms();
    let base = *pose;
    let sleeping = state == CompanionState::Sleeping;
    pose.expression = if sleeping {
        MascotExpression::Affectionate
    } else {
        match active.action {
            MascotAction::Greet => MascotExpression::Delighted,
            MascotAction::Pet => MascotExpression::Affectionate,
            MascotAction::Tickle => MascotExpression::Laughing,
            MascotAction::Surprise => MascotExpression::Surprised,
            MascotAction::Comfort => MascotExpression::Reassuring,
        }
    };

    let amplitude = match active.personality {
        MascotPersonality::Calm => 1,
        MascotPersonality::Cozy => 2,
        MascotPersonality::Playful => 3,
    };
    let direction = if active.seed & 1 == 0 { -1 } else { 1 };
    let wave = centred_wave(elapsed_ms, 600, 128 * amplitude);
    let settle = if elapsed_ms + 350 >= duration {
        smoothstep_q8(duration - elapsed_ms, 350)
    } else {
        SCALE_Q8_ONE
    };
    let scaled_wave = wave * i32::from(settle) / 256;
    pose.body_offset_q8 = match active.action {
        MascotAction::Greet => (direction * scaled_wave / 2, -scaled_wave.abs() / 2),
        MascotAction::Pet => (direction * 96 * amplitude, scaled_wave.abs() / 5),
        MascotAction::Tickle => (scaled_wave, 0),
        MascotAction::Surprise => (0, 128 * amplitude - scaled_wave.abs() / 2),
        MascotAction::Comfort => (direction * scaled_wave / 4, scaled_wave.abs() / 8),
    };
    if sleeping {
        pose.body_offset_q8.0 = pose.body_offset_q8.0.clamp(-256, 256);
        pose.body_offset_q8.1 = pose.body_offset_q8.1.clamp(-128, 128);
    }

    let gaze_x = direction * 256;
    let gaze_y = if active.seed & 2 == 0 { -128 } else { 128 };
    pose.eye_offset_q8 = (gaze_x, gaze_y);
    if active.action == MascotAction::Greet && (850..1_150).contains(&elapsed_ms) && !sleeping {
        pose.left_eye_scale_y_q8 = 48;
    }

    let swap_in = transition_blink_scale(elapsed_ms.min(350), 350);
    let remaining = duration.saturating_sub(elapsed_ms);
    let swap_out = transition_blink_scale(remaining.min(350), 350);
    pose.eyes_scale_y_q8 = pose.eyes_scale_y_q8.min(swap_in.min(swap_out));
    let entered = smoothstep_q8(elapsed_ms.min(350), 350);
    *pose = blend_pose(active.from, *pose, entered);
    if elapsed_ms + 350 >= duration {
        let recovery = smoothstep_q8(duration - elapsed_ms, 350);
        *pose = blend_pose(base, *pose, recovery);
    }
}

fn centred_wave(now_ms: ElapsedMs, period_ms: u32, amplitude: i32) -> i32 {
    let phase = now_ms % period_ms;
    let half = period_ms / 2;
    if phase <= half {
        -amplitude + (i32::from(smoothstep_q8(phase, half)) * amplitude * 2 / 256)
    } else {
        amplitude - (i32::from(smoothstep_q8(phase - half, half)) * amplitude * 2 / 256)
    }
}

fn positive_wave(now_ms: ElapsedMs, period_ms: u32, amplitude: i32) -> i32 {
    let phase = now_ms % period_ms;
    let half = period_ms / 2;
    if phase <= half {
        i32::from(smoothstep_q8(phase, half)) * amplitude / 256
    } else {
        i32::from(smoothstep_q8(period_ms - phase, half)) * amplitude / 256
    }
}

fn idle_life(pose: &mut MascotPose, now_ms: ElapsedMs, seed: u32) -> u16 {
    let epoch = now_ms / IDLE_EPOCH_MS;
    let phase = now_ms % IDLE_EPOCH_MS;
    let random = mix_seed(seed ^ epoch.wrapping_mul(0x9E37_79B9));
    let primary = 2_000 + random % 2_401;
    let expressive = random & 0b11 == 0;
    let expression_end = primary + 900;

    let mut scale = blink_at(phase, primary);
    if random & 0b111 == 0 {
        scale = scale.min(blink_at(phase, primary + 260));
    }
    if expressive {
        scale = scale.min(blink_at(phase, expression_end));
        if (primary..expression_end).contains(&phase) {
            pose.expression = match (random >> 8) % 3 {
                0 => MascotExpression::Reassuring,
                1 => MascotExpression::Affectionate,
                _ => MascotExpression::Delighted,
            };
        }
    }

    let gaze_start = primary.saturating_sub(500);
    let gaze_end = if expressive {
        (expression_end + 350).min(IDLE_EPOCH_MS)
    } else {
        (primary + 650).min(IDLE_EPOCH_MS)
    };
    if (gaze_start..gaze_end).contains(&phase) {
        let fade_in_end = gaze_start + 300;
        let fade_out_start = gaze_end.saturating_sub(300);
        let strength = if phase < fade_in_end {
            smoothstep_q8(phase - gaze_start, 300)
        } else if phase > fade_out_start {
            smoothstep_q8(gaze_end - phase, 300)
        } else {
            SCALE_Q8_ONE
        };
        let direction = if random & (1 << 5) == 0 { -1 } else { 1 };
        let vertical = if random & (1 << 6) == 0 { -1 } else { 1 };
        let x = direction * (128 + ((random >> 16) & 1) as i32 * 128);
        pose.eye_offset_q8 = (
            x * i32::from(strength) / 256,
            vertical * 128 * i32::from(strength) / 256,
        );
    }
    scale
}

fn varied_blink_scale(now_ms: ElapsedMs, seed: u32) -> u16 {
    let epoch = now_ms / IDLE_EPOCH_MS;
    let phase = now_ms % IDLE_EPOCH_MS;
    let random = mix_seed(seed ^ epoch.wrapping_mul(0x9E37_79B9));
    let primary = 2_000 + random % 2_401;
    let mut scale = blink_at(phase, primary);
    if random & 0b111 == 0 {
        scale = scale.min(blink_at(phase, primary + 260));
    }
    scale
}

fn blink_at(phase: u32, centre: u32) -> u16 {
    let distance = phase.abs_diff(centre);
    if distance >= BLINK_HALF_WINDOW_MS {
        SCALE_Q8_ONE
    } else {
        48 + (u32::from(SCALE_Q8_ONE - 48) * distance / BLINK_HALF_WINDOW_MS) as u16
    }
}

fn mix_seed(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

fn transition_blink_scale(elapsed_ms: ElapsedMs, duration_ms: ElapsedMs) -> u16 {
    let midpoint = duration_ms / 2;
    let distance = elapsed_ms.abs_diff(midpoint);
    let half_window = (duration_ms / 4).max(1);
    if distance >= half_window {
        SCALE_Q8_ONE
    } else {
        48 + (u32::from(SCALE_Q8_ONE - 48) * distance / half_window) as u16
    }
}

fn smoothstep_q8(elapsed_ms: u32, duration_ms: u32) -> u16 {
    let x = (u64::from(elapsed_ms) * 256 / u64::from(duration_ms)) as u32;
    let x = x.min(256);
    ((3 * x * x * 256 - 2 * x * x * x) / (256 * 256)) as u16
}

fn lerp_i32(from: i32, to: i32, progress_q8: u16) -> i32 {
    from + ((i64::from(to - from) * i64::from(progress_q8)) / 256) as i32
}

fn lerp_u16(from: u16, to: u16, progress_q8: u16) -> u16 {
    lerp_i32(i32::from(from), i32::from(to), progress_q8) as u16
}

fn lerp_u8(from: u8, to: u8, progress_q8: u16) -> u8 {
    lerp_i32(i32::from(from), i32::from(to), progress_q8) as u8
}

fn blend_pose(from: MascotPose, to: MascotPose, progress_q8: u16) -> MascotPose {
    let state_weights = if progress_q8 < SCALE_Q8_ONE / 2 {
        from.state_weights
    } else {
        to.state_weights
    };
    MascotPose {
        body_offset_q8: (
            lerp_i32(from.body_offset_q8.0, to.body_offset_q8.0, progress_q8),
            lerp_i32(from.body_offset_q8.1, to.body_offset_q8.1, progress_q8),
        ),
        body_scale_q8: lerp_u16(from.body_scale_q8, to.body_scale_q8, progress_q8),
        opacity: lerp_u8(from.opacity, to.opacity, progress_q8),
        eyes_scale_y_q8: lerp_u16(from.eyes_scale_y_q8, to.eyes_scale_y_q8, progress_q8),
        left_eye_scale_y_q8: lerp_u16(
            from.left_eye_scale_y_q8,
            to.left_eye_scale_y_q8,
            progress_q8,
        ),
        right_eye_scale_y_q8: lerp_u16(
            from.right_eye_scale_y_q8,
            to.right_eye_scale_y_q8,
            progress_q8,
        ),
        eye_offset_q8: (
            lerp_i32(from.eye_offset_q8.0, to.eye_offset_q8.0, progress_q8),
            lerp_i32(from.eye_offset_q8.1, to.eye_offset_q8.1, progress_q8),
        ),
        expression: if progress_q8 < SCALE_Q8_ONE / 2 {
            from.expression
        } else {
            to.expression
        },
        state_weights,
    }
}
