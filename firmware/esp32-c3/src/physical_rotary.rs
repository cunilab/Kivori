//! Physical HW-040 [`InputSource`].
//!
//! This adapter does NOTHING but read pin levels. All conditioning and semantics live above
//! the port in `input::quadrature` and `input::gesture`, which is exactly what makes them
//! provable without hardware.
//!
//! HW-040 modules are open-collector with the common pin to ground, so inputs use internal
//! pull-ups and read ACTIVE-LOW. `sample()` inverts to logical levels. The concrete GPIO
//! assignments live in [`crate::profile::physical_st7789::RotaryProfile`], not here.

use esp_hal::gpio::Input;
use kivori_model::input::InputLevels;

use crate::ports::InputSource;

/// Reads the HW-040 CLK/DT/SW lines and nothing else.
///
/// No debounce, no edge detection, no state: `sample()` is a direct, inverted read of the three
/// pins on every call.
pub struct PhysicalRotary<'d> {
    clk: Input<'d>,
    dt: Input<'d>,
    sw: Input<'d>,
}

impl<'d> PhysicalRotary<'d> {
    /// Wraps already-configured `clk`/`dt`/`sw` GPIO inputs.
    ///
    /// Callers are expected to have configured each pin with an internal pull-up (the HW-040
    /// lines are active-low), matching [`crate::profile::physical_st7789::ROTARY`].
    #[must_use]
    pub fn new(clk: Input<'d>, dt: Input<'d>, sw: Input<'d>) -> Self {
        Self { clk, dt, sw }
    }
}

impl InputSource for PhysicalRotary<'_> {
    fn sample(&mut self) -> InputLevels {
        InputLevels {
            a: self.clk.is_low(),
            b: self.dt.is_low(),
            sw: self.sw.is_low(),
        }
    }
}
