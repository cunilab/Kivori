//! Board profiles: the one place concrete panel and pin-map facts are recorded.
//!
//! [`physical_st7789`] is the RP2040-Zero + ST7789 240x240 profile. Changes to it require
//! validation against real hardware.
//!
//! The pin map is fixed by the enclosure's carrier (`hardware/enclosure-v7`, #179); it is checked
//! against the carrier's net list by `hardware/enclosure-v7/test_pin_map.py`. Pins are GPxx numbers
//! of the Waveshare RP2040-Zero.

/// Physical RP2040-Zero + ST7789 240x240 hardware profile.
///
/// Gated on `host-sim` as well as `physical-st7789` so the plain pin-map data (and the
/// profile-collision test in `tests/rotary_input.rs`) is host-testable. The helpers below that
/// name a HAL type stay gated on `physical-st7789` alone.
#[cfg(any(feature = "physical-st7789", feature = "host-sim"))]
pub mod physical_st7789 {
    use crate::display::PanelGeometry;
    #[cfg(feature = "physical-st7789")]
    use embedded_hal::spi::Mode;
    use kivori_model::desk::BRIGHTNESS_STEPS;
    use mipidsi::{
        models::ST7789,
        options::{ColorInversion, ColorOrder, Orientation, Rotation},
    };

    /// Capabilities advertised by the shipping Pip runtime and its host simulator. The simulator
    /// removes `RGB_LED_V1` for the v1 board (ESP32-C3), which has no LED.
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
            .union(kivori_model::Capabilities::BUILD_INFO_V1)
            .union(kivori_model::Capabilities::HELD_ROTATION_V1)
            .union(kivori_model::Capabilities::RGB_LED_V1);

    /// SPI1 clock pin (display SCL).
    pub const SCK: u8 = 26;

    /// SPI1 TX pin (display SDA).
    pub const MOSI: u8 = 15;

    /// Display data/command pin.
    pub const DC: u8 = 13;

    /// Display reset pin.
    pub const RST: u8 = 14;

    /// Display backlight pin: PWM slice 6, channel A.
    pub const BL: u8 = 12;

    /// RGB LED pins (red, green, blue), each through 1 k to a common cathode, so active high. PWM
    /// slices 4B, 5A, 5B.
    pub const LED: [u8; 3] = [9, 10, 11];

    /// Per-colour duty scale (255 = full) so a mix matches the app's colour: on 3.3 V with 200 R each,
    /// red (Vf ~2.0 V) draws ~4x the current of green and blue (Vf ~3.0 V). Tuned by eye (#179).
    pub const LED_GAIN: [u8; 3] = [77, 255, 255];

    /// This physical panel does not use chip select.
    pub const CS: Option<u8> = None;

    /// Verified SPI clock. The next throughput lever once frame time is measured on hardware
    /// (needs a signal-integrity check; a full frame is 46 ms of wire time at 20 MHz; the RP2040
    /// SPI tops out at 62.5 MHz).
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
        embedded_hal::spi::MODE_3
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

    /// HW-040 rotary encoder pin map.
    ///
    /// ISOLATED HERE ON PURPOSE: this is the single place to correct if the wiring changes.
    /// `sw` is sampled by the adapter and consumed by the push-switch gesture machine
    /// (`input::gesture`).
    ///
    /// **This is a SPECIFICATION, not measured evidence.** The pin map moved to the RP2040-Zero
    /// carrier (#179), so until `docs/validation.md` rows 3.15 and 5.1 are checked on a board in
    /// the v7 enclosure, none of these GPIOs is confirmed hardware fact. The panel parameters
    /// (geometry, SPI mode, orientation, colour order, inversion) stay verified.
    ///
    /// **SW = GP3, DT = GP4, CLK = GP5.** None is a boot-critical pin (the RP2040 has no strapping
    /// pins; BOOTSEL is a dedicated button), so holding the encoder switch at power-on, the
    /// recovery gesture, boots Kivori normally.
    ///
    /// The HW-040 module has its own pull-ups to 3V3 and its common pin to ground, so the lines
    /// are active-low; the adapter in `physical_rotary` inverts them to logical levels.
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
        clk: 5,
        dt: 4,
        sw: 3,
    };

    /// The three contextual buttons, left to right (GP6 / GP7 / GP8). Each is a plain momentary
    /// switch from the GPIO to GND, read active-low with the internal pull-up; no external parts.
    ///
    /// Like the encoder map, a specification until physically verified.
    pub const CONTEXT_BUTTONS: [u8; 3] = [6, 7, 8];

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

        /// Every pin the firmware drives or reads, with what it is for.
        const PINS: [(&str, u8); 14] = [
            ("SCK", SCK),
            ("MOSI", MOSI),
            ("DC", DC),
            ("RST", RST),
            ("BL", BL),
            ("LED R", LED[0]),
            ("LED G", LED[1]),
            ("LED B", LED[2]),
            ("ENC CLK", ROTARY.clk),
            ("ENC DT", ROTARY.dt),
            ("ENC SW", ROTARY.sw),
            ("KEY 1", CONTEXT_BUTTONS[0]),
            ("KEY 2", CONTEXT_BUTTONS[1]),
            ("KEY 3", CONTEXT_BUTTONS[2]),
        ];

        #[test]
        fn no_pin_is_used_for_two_things() {
            for (i, (name, pin)) in PINS.iter().enumerate() {
                for (other, other_pin) in &PINS[i + 1..] {
                    assert_ne!(pin, other_pin, "GP{pin} is both {name} and {other}");
                }
            }
        }

        #[test]
        fn only_pins_the_rp2040_zero_brings_out_are_used_and_never_the_reserved_ones() {
            for (name, pin) in PINS {
                // The Zero breaks out GP0 to GP15 and GP26 to GP29; GP16 is its WS2812. All of them
                // are bank 0 pins, which the one edge handler (IO_IRQ_BANK0) serves.
                assert!(
                    pin <= 15 || (26..=29).contains(&pin),
                    "{name} on GP{pin} is not a header pin of the RP2040-Zero"
                );
                assert!(
                    pin > 1,
                    "{name} on GP{pin} is the debug UART (GP0 TX, GP1 RX)"
                );
                assert!(
                    !(27..=29).contains(&pin),
                    "{name} on GP{pin}: GP27 to GP29 are kept free on the v7 carrier"
                );
            }
        }

        #[test]
        fn the_display_pins_are_spi1_clock_and_data_out() {
            // RP2040 GPIO function table: SPI1 SCK is GP10/14/26, SPI1 TX is GP11/15/27 (SPI0 is
            // GP2/6/18 and GP3/7/19). One bus must carry both signals.
            assert!(
                [10, 14, 26].contains(&SCK),
                "GP{SCK} is not an SPI1 SCK pin"
            );
            assert!(
                [11, 15, 27].contains(&MOSI),
                "GP{MOSI} is not an SPI1 TX pin"
            );
            // The panel is write-only and has no chip select.
            assert_eq!(CS, None);
        }

        /// The PWM slice and channel (`false` = A, `true` = B) an RP2040 GPIO is wired to.
        const fn pwm(pin: u8) -> (u8, bool) {
            ((pin / 2) % 8, pin % 2 == 1)
        }

        #[test]
        fn the_backlight_and_led_sit_on_the_pwm_channels_the_driver_code_names() {
            // `physical_st7789.rs` types these as Pwm6/A, then Pwm4/B, Pwm5/A and Pwm5/B.
            assert_eq!(pwm(BL), (6, false));
            assert_eq!(
                LED.map(pwm),
                [(4, true), (5, false), (5, true)],
                "LED pins moved: the driver's channel types no longer match"
            );
        }
    }
}
