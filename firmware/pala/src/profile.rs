//! Board profiles: the one place concrete panel and pin-map facts are recorded.
//!
//! [`physical_st7789`] is the hardware-validated ESP32-C3 + ST7789 240x240 profile. Changes to it
//! require validation against real hardware.

/// Verified physical ESP32-C3 + ST7789 240x240 hardware profile.
///
/// Gated on `host-sim` as well as `physical-st7789` so the plain pin-map data (and the
/// profile-collision test in `tests/rotary_input.rs`) is host-testable. The esp-hal-typed
/// helpers below (which cannot build for a host target at all) stay gated on
/// `physical-st7789` alone.
#[cfg(any(feature = "physical-st7789", feature = "host-sim"))]
pub mod physical_st7789 {
    use crate::display::PanelGeometry;
    #[cfg(feature = "physical-st7789")]
    use esp_hal::{gpio::Level, spi::Mode};
    use kivori_model::desk::BRIGHTNESS_STEPS;
    use mipidsi::{
        models::ST7789,
        options::{ColorInversion, ColorOrder, Orientation, Rotation},
    };

    /// SPI clock pin.
    pub const SCK: u8 = 6;

    /// SPI MOSI pin.
    pub const MOSI: u8 = 7;

    /// Display data/command pin.
    pub const DC: u8 = 2;

    /// Display reset pin.
    pub const RST: u8 = 3;

    /// Display backlight pin.
    pub const BL: u8 = 8;

    /// This physical panel does not use chip select.
    pub const CS: Option<u8> = None;

    /// Verified SPI clock. The next throughput lever once frame time is measured on hardware
    /// (needs a signal-integrity check; a full frame is 46 ms of wire time at 20 MHz).
    pub const SPI_CLOCK_HZ: u32 = 20_000_000;

    /// Visible panel width.
    pub const WIDTH: u16 = 240;

    /// Visible panel height.
    pub const HEIGHT: u16 = 240;

    /// Verified visible-area X offset.
    pub const OFFSET_X: u16 = 0;

    /// Verified visible-area Y offset.
    pub const OFFSET_Y: u16 = 0;

    /// Backlight is enabled by driving the pin high.
    pub const BACKLIGHT_ACTIVE_HIGH: bool = true;

    /// Backlight PWM duty (percent of "on") for each brightness step, dimmest first. The lowest
    /// step stays clearly lit, so it can never be mistaken for Display Sleep.
    pub const BACKLIGHT_DUTY_PCT: [u8; BRIGHTNESS_STEPS as usize] = [25, 40, 60, 80, 100];

    /// The PWM duty to program for brightness `step` (clamped to `1..=BRIGHTNESS_STEPS`), as the
    /// pin's own duty: inverted when the backlight is active low.
    #[must_use]
    pub const fn backlight_duty_pct(step: u8) -> u8 {
        let step = if step < 1 {
            1
        } else if step > BRIGHTNESS_STEPS {
            BRIGHTNESS_STEPS
        } else {
            step
        };
        let on = BACKLIGHT_DUTY_PCT[(step - 1) as usize];
        if BACKLIGHT_ACTIVE_HIGH {
            on
        } else {
            100 - on
        }
    }

    /// Verified physical display geometry.
    #[must_use]
    pub const fn geometry() -> PanelGeometry {
        PanelGeometry::new(WIDTH, HEIGHT, OFFSET_X, OFFSET_Y)
    }

    /// Verified SPI mode.
    #[cfg(feature = "physical-st7789")]
    #[must_use]
    pub const fn spi_mode() -> Mode {
        Mode::_3
    }

    /// Verified physical panel controller.
    #[must_use]
    pub const fn panel_model() -> ST7789 {
        ST7789
    }

    /// Verified panel orientation.
    #[must_use]
    pub fn orientation() -> Orientation {
        Orientation::new().rotate(Rotation::Deg90)
    }

    /// Verified panel color order.
    #[must_use]
    pub const fn color_order() -> ColorOrder {
        ColorOrder::Rgb
    }

    /// Verified panel color inversion.
    #[must_use]
    pub const fn color_inversion() -> ColorInversion {
        ColorInversion::Inverted
    }

    /// Backlight output level.
    #[cfg(feature = "physical-st7789")]
    #[must_use]
    pub const fn backlight_level() -> Level {
        if BACKLIGHT_ACTIVE_HIGH {
            Level::High
        } else {
            Level::Low
        }
    }

    /// HW-040 rotary encoder pin map.
    ///
    /// ISOLATED HERE ON PURPOSE: this is the single place to correct if the wiring changes.
    /// `sw` is sampled by the adapter and consumed by the push-switch gesture machine
    /// (`input::gesture`).
    ///
    /// **This is a SPECIFICATION, not measured evidence.** Unlike the rest of this module
    /// (`SCK`/`MOSI`/`DC`/`RST`/`BL`, all verified against a real board), nobody has verified
    /// continuity of this rotary wiring on physical hardware yet — the maintainer is wiring the
    /// HW-040 module to these GPIOs, and `docs/validation.md` row 3.15 carries the verification. Do not read this
    /// struct as confirmed hardware fact.
    ///
    /// Pin choices, decided deliberately:
    ///
    /// * **CLK = GPIO4, DT = GPIO0, SW = GPIO1.** All three sit on the SuperMini's left pin row,
    ///   next to the encoder, so the copper-tape PCB needs no trace crossing between the rows.
    /// * **GPIO9 was rejected for `sw`** despite being the devkit BOOT button. Holding GPIO9 low
    ///   at reset enters ROM download mode, and the encoder switch is Kivori's recovery control —
    ///   a user power-cycling while holding it for recovery would land in the downloader instead
    ///   of booting Kivori.
    /// * GPIO2 and GPIO8 are strapping pins and already taken (D/C, backlight). GPIO20, GPIO10
    ///   and GPIO5 carry the contextual buttons (see [`CONTEXT_BUTTONS`]); GPIO21 stays free as
    ///   the UART0 TX boot / log console.
    /// * **GPIO0 and GPIO1** are the XTAL_32K pair. They are plain GPIOs only when no 32.768 kHz
    ///   crystal is fitted: check the devkit first (validation row 5.1).
    /// * None of the three is a strapping pin, and all three are in GPIO0-5, so any of them can
    ///   wake the C3 from deep sleep if that is ever wanted (Kivori's Display Sleep is panel
    ///   blanking with the MCU awake).
    ///
    /// The three lines are wired active-low (HW-040 COM to GND) and read with internal pull-ups;
    /// the adapter in `physical_rotary` inverts them to logical levels.
    #[derive(Debug, Clone, Copy)]
    pub struct RotaryProfile {
        /// Quadrature channel A (CLK) pin.
        pub clk: u8,
        /// Quadrature channel B (DT) pin.
        pub dt: u8,
        /// Push-switch (SW) pin.
        pub sw: u8,
    }

    /// HW-040 wiring specification decided for this board (see [`RotaryProfile`]).
    pub const ROTARY: RotaryProfile = RotaryProfile {
        clk: 4,
        dt: 0,
        sw: 1,
    };

    /// The three contextual buttons, left to right. Each is a plain momentary switch from the
    /// GPIO to GND, read active-low with the internal pull-up; no external parts.
    ///
    /// These are the only free GPIOs left, decided deliberately:
    ///
    /// * All three sit on the SuperMini's right pin row, next to the buttons, so the copper-tape
    ///   PCB needs no trace crossing between the rows.
    /// * **GPIO20** is UART0 RX. Kivori never reads UART0 (the data link is USB Serial/JTAG and
    ///   `esp-println` only writes TX), so the boot log on GPIO21 keeps working.
    /// * **GPIO9 stays unused**: pressed at reset it would enter ROM download mode.
    /// * None of the three is a strapping pin, so a button held at power-on cannot change how the
    ///   chip boots.
    ///
    /// Like the encoder map, a specification until physically verified.
    pub const CONTEXT_BUTTONS: [u8; 3] = [20, 10, 5];

    #[cfg(all(test, feature = "host-sim"))]
    mod tests {
        use super::*;

        #[test]
        fn backlight_steps_rise_and_the_lowest_stays_clearly_on() {
            let mut on = [0u8; BRIGHTNESS_STEPS as usize];
            for step in 1..=BRIGHTNESS_STEPS {
                let duty = backlight_duty_pct(step);
                on[usize::from(step) - 1] = if BACKLIGHT_ACTIVE_HIGH {
                    duty
                } else {
                    100 - duty
                };
            }
            assert!(on.windows(2).all(|w| w[0] < w[1]), "{on:?}");
            assert!(
                on[0] >= 20,
                "the dimmest step must never look like Display Sleep"
            );
            assert_eq!(on[on.len() - 1], 100, "the top step is the old always-on");
        }

        #[test]
        fn out_of_range_steps_clamp() {
            assert_eq!(backlight_duty_pct(0), backlight_duty_pct(1));
            assert_eq!(backlight_duty_pct(99), backlight_duty_pct(BRIGHTNESS_STEPS));
        }
    }
}
