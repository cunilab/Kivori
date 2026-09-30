//! Rotary volume value model: fixed step, immediate clamp, boundary flag.
//!
//! Acceleration is deliberately absent in Slice 002. With a fixed step there is
//! no multiplier state, so direction reversal is correct by construction.

use kivori_model::input::Direction;

/// Volume points moved per validated detent. Provisional; UX tuning parameter.
pub const BASE_STEP_PERCENT: u8 = 2;

const MIN_PERCENT: i16 = 0;
const MAX_PERCENT: i16 = 100;

/// Apply one detent. Returns `(next_value, at_boundary)`.
///
/// `at_boundary` is true only when the motion was ENTIRELY absorbed by the clamp,
/// so continued pressure into a bound does not re-trigger feedback while a value
/// that merely lands on the bound does not raise it.
pub fn apply_step(current: u8, direction: Direction) -> (u8, bool) {
    let delta = match direction {
        Direction::Cw => i16::from(BASE_STEP_PERCENT),
        Direction::Ccw => -i16::from(BASE_STEP_PERCENT),
    };
    let raw = i16::from(current) + delta;
    let clamped = raw.clamp(MIN_PERCENT, MAX_PERCENT);
    let absorbed = clamped == i16::from(current);
    (clamped as u8, absorbed)
}
