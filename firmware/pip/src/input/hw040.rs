//! HW-040 rotary encoder module: which pin feeds which quadrature channel.
//!
//! On the HW-040, CLK changes before DT on a clockwise turn, while [`super::quadrature`] counts
//! channel B leading channel A as clockwise. So DT is channel A and CLK is channel B. Measured
//! 2026-10-31 on the owner's unit: with CLK as A, a turn to the right arrived as `Detent(Ccw)` and
//! lowered the volume. It lives above the port so a host test (and the simulator) proves it.

use kivori_model::input::InputLevels;

/// Logical levels from the module's pins, each `true` while its line is pulled low (active).
#[must_use]
pub const fn levels(clk: bool, dt: bool, sw: bool, keys: [bool; 3]) -> InputLevels {
    InputLevels {
        a: dt,
        b: clk,
        sw,
        keys,
    }
}
