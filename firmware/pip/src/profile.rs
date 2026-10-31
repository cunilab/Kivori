//! Board profiles: the one place concrete panel and pin-map facts are recorded.
//!
//! [`physical_st7789`] is the hardware-validated ESP32-C3 + ST7789 240x240 profile. Changes to it
//! require validation against real hardware.
//!
//! The pin map follows the enclosure's wiring (#123): seen from the SuperMini's component side
//! with USB-C at the top, the right row is 5V, GND, 3V3, IO4..IO0 and the left row is IO5..IO10,
//! IO20, IO21. The display sits on the right row (IO4..IO0), the encoder and the buttons on the
//! left row (IO5..IO10), so no wire crosses between the rows.

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

    /// Capabilities advertised by the shipping Pip runtime and its host simulator.
    pub const CAPABILITIES: kivori_model::Capabilities =
        kivori_model::Capabilities::MASCOT_INTERACTION
            .union(kivori_model::Capabilities::PHYSICAL_INPUT_V1)
            .union(kivori_model::Capabilities::PRESENTATION_V1)
            .union(kivori_model::Capabilities::BUTTON_INPUT_V1)
            .union(kivori_model::Capabilities::DESK_STATUS_V1)
            .union(kivori_model::Capabilities::ACTION_FEEDBACK_V1)
            .union(kivori_model::Capabilities::DOUBLE_PRESS_V1)
            .union(kivori_model::Capabilities::MEDIA_INFO_V1)
            .union(kivori_model::Capabilities::CONTROL_LABELS_V1)
            .union(kivori_model::Capabilities::CONTEXT_BUTTONS_V1)
            .union(kivori_model::Capabilities::HOST_TAKEOVERS_V1)
            .union(kivori_model::Capabilities::QUIET_ACK_V1)
            .union(kivori_model::Capabilities::DEVICE_INTERNALS_V1)
            .union(kivori_model::Capabilities::IDLE_LIFE_V1)
            .union(kivori_model::Capabilities::MIC_STATE_V1)
            .union(kivori_model::Capabilities::PRODUCT_IDENTITY_V1)
            .union(kivori_model::Capabilities::BUILD_INFO_V1);

    /// SPI clock pin (display SCL).
    pub const SCK: u8 = 4;

    /// SPI MOSI pin (display SDA).
    pub const MOSI: u8 = 3;

    /// Display data/command pin. GPIO1 is an XTAL_32K pin: a plain GPIO only on a board without
    /// a 32.768 kHz crystal (validation row 5.1).
    pub const DC: u8 = 1;

    /// Display reset pin. GPIO2 is a strapping pin; RES is an input on the display, so the pin
    /// is left alone at reset, as on a bare board.
    pub const RST: u8 = 2;

    /// Display backlight pin. GPIO0 is an XTAL_32K pin, like [`DC`].
    pub const BL: u8 = 0;

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
        // Deg270 since 2026-10-31: the owner's assembled unit showed the picture upside down at
        // Deg90. mipidsi moves the 240x240 window inside the 240x320 controller RAM itself.
        Orientation::new().rotate(Rotation::Deg270)
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
    /// **This is a SPECIFICATION, not measured evidence.** The whole pin map moved to match the
    /// enclosure (#123), so until `docs/validation.md` rows 3.15 and 5.1 are checked on a board
    /// wired to an enclosure PCB, none of these GPIOs is confirmed hardware fact. The panel
    /// parameters (geometry, SPI mode, orientation, colour order, inversion) stay verified.
    ///
    /// Pin choices, decided deliberately:
    ///
    /// * **CLK = GPIO7, DT = GPIO6, SW = GPIO5.** All three sit on the SuperMini's left pin row,
    ///   next to the encoder, so the enclosure needs no wire crossing between the rows.
    /// * **GPIO9 stays off `sw`** despite being the devkit BOOT button. Holding GPIO9 low at reset
    ///   enters ROM download mode, and the encoder switch is Kivori's recovery control: a user
    ///   power-cycling while holding it for recovery would land in the downloader instead of
    ///   booting Kivori. GPIO9 carries button B1 instead (see [`CONTEXT_BUTTONS`]).
    /// * None of the three is a strapping pin. GPIO21 stays free as the UART0 TX boot / log
    ///   console, and GPIO20 is free.
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
        clk: 7,
        dt: 6,
        sw: 5,
    };

    /// The three contextual buttons, left to right. Each is a plain momentary switch from the
    /// GPIO to GND, read active-low with the internal pull-up; no external parts.
    ///
    /// Pin choices, decided deliberately:
    ///
    /// * **B1 / B2 / B3 = GPIO9 / GPIO8 / GPIO10.** All three sit on the SuperMini's left pin
    ///   row, next to the buttons, so the enclosure needs no wire crossing between the rows.
    /// * **GPIO9 (BOOT) is B1.** Holding B1 while the board resets enters ROM download mode,
    ///   exactly like the SuperMini's own BOOT button. That is harmless and sometimes handy;
    ///   recovery is unaffected because it belongs to the encoder switch, which is not on GPIO9.
    /// * **GPIO8 is B2**, a strapping pin that only matters for download mode (it must be high
    ///   then), so do not hold B2 while flashing. On the SuperMini it also drives the onboard
    ///   LED, which can light while B2 is pressed.
    /// * Holding B2 or B3 alone at power-on boots Kivori normally.
    ///
    /// Like the encoder map, a specification until physically verified.
    pub const CONTEXT_BUTTONS: [u8; 3] = [9, 8, 10];

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
