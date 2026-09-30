//! Detent-qualified quadrature decoding.
//!
//! Phase is the 2-bit Gray code `(a << 1) | b`; the HW-040 rests at `00` (both pins high,
//! inverted). Each legal transition moves one quarter-step. A `Direction` is emitted only on
//! arrival back at rest after four quarter-steps in the same rotational sense, i.e. a complete
//! mechanical detent. The accumulator is re-zeroed at every rest arrival, so partial motion or
//! a count broken by a missed state can never carry over into a bounce at rest.

use kivori_model::input::Direction;

/// Quarter-steps in one full HW-040 detent.
const QUARTER_STEPS_PER_DETENT: i8 = 4;

/// Logical phase of a knob sitting in a detent.
const REST_PHASE: u8 = 0b00;

/// Pure state machine turning raw quadrature `(a, b)` samples into validated logical
/// detents.
///
/// A legal electrical transition is NOT automatically a completed detent: mechanical
/// bounce can legally walk between adjacent Gray states and back without the knob
/// completing a detent (user-story-contract invariant 46). Only a fully traversed
/// four-quarter-step cycle emits a [`Direction`].
#[derive(Debug)]
pub struct QuadratureDecoder {
    /// Last observed Gray phase, or `None` before the first sample.
    phase: Option<u8>,
    /// Signed quarter-step accumulator; positive is clockwise.
    accumulator: i8,
    /// Count of electrically impossible transitions (both bits changed at once).
    invalid: u32,
}

impl Default for QuadratureDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl QuadratureDecoder {
    /// Creates a decoder with no reference phase and no accumulated motion.
    pub const fn new() -> Self {
        Self {
            phase: None,
            accumulator: 0,
            invalid: 0,
        }
    }

    /// Feed one sample. Returns a direction only when a full detent completed.
    pub fn update(&mut self, a: bool, b: bool) -> Option<Direction> {
        let next = ((a as u8) << 1) | (b as u8);

        let Some(prev) = self.phase else {
            // First sample establishes the reference phase without producing motion.
            self.phase = Some(next);
            return None;
        };

        if next == prev {
            return None;
        }

        let step = match quarter_step(prev, next) {
            Some(step) => step,
            None => {
                // Both bits changed: impossible for a real contact, so a state was missed
                // or the signal was corrupted. The traversal is no longer continuous, so
                // banked quarter-steps cannot be trusted to splice with what comes after
                // this gap — reset the accumulator rather than risk completing a detent
                // that was never fully, legally observed (contract invariant 42; R-77).
                self.invalid = self.invalid.saturating_add(1);
                self.phase = Some(next);
                self.accumulator = 0;
                return None;
            }
        };

        self.phase = Some(next);
        self.accumulator += step;
        if next != REST_PHASE {
            return None;
        }

        // Back at rest: a detent only if a full, continuous cycle was observed since the last
        // rest arrival. Anything short of that (bounce, a count restarted mid-cycle by an
        // invalid transition or `reset`) is discarded here rather than completed later.
        let accumulated = core::mem::take(&mut self.accumulator);
        match accumulated {
            QUARTER_STEPS_PER_DETENT => Some(Direction::Cw),
            a if a == -QUARTER_STEPS_PER_DETENT => Some(Direction::Ccw),
            _ => None,
        }
    }

    /// Count of electrically impossible transitions observed so far (both bits changed
    /// at once between consecutive samples).
    pub const fn invalid_transitions(&self) -> u32 {
        self.invalid
    }

    /// Drop all motion state. Used when a session ends so partial motion cannot
    /// leak across a reconnect.
    pub fn reset(&mut self) {
        self.phase = None;
        self.accumulator = 0;
    }
}

/// Gray-code adjacency: `+1` clockwise, `-1` counter-clockwise, `None` if illegal.
///
/// Clockwise phase order is 00 -> 01 -> 11 -> 10 -> 00.
const fn quarter_step(prev: u8, next: u8) -> Option<i8> {
    match (prev, next) {
        (0b00, 0b01) | (0b01, 0b11) | (0b11, 0b10) | (0b10, 0b00) => Some(1),
        (0b00, 0b10) | (0b10, 0b11) | (0b11, 0b01) | (0b01, 0b00) => Some(-1),
        _ => None,
    }
}
