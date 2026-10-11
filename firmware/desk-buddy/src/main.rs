#![no_std]
#![no_main]

//! Kivori ESP32-C3 firmware entry point.
//!
//! The binary selects exactly one firmware mode through Cargo features:
//!
//! - `physical-st7789` — production Kivori runtime on the hardware-validated
//!   ESP32-C3 + ST7789 240x240 physical profile.
//! - `wokwi-runtime` — production [`kivori_firmware::runtime::run`] loop using
//!   the simulation-only Wokwi board profile.
//! - `wokwi-serial` — external serial protocol test. Wokwi injects real wire
//!   frames into the ESP32-C3 USB Serial/JTAG transport.
//! - `wokwi-spi` — generic SPI/RGB565 tile-transfer probe. This validates the
//!   SPI/render transport path only and is not physical-panel evidence.
//! - `wokwi` — internal on-target integration/self-test.
//! - `embedded` only — brings up the ESP32-C3 clock and native USB
//!   Serial/JTAG transport without selecting a display profile.
//!
//! Building this binary requires the `embedded` feature. Higher-level modes
//! such as `physical-st7789` enable `embedded` through `Cargo.toml`.

#[cfg(feature = "embedded")]
use esp_backtrace as _;

/// Canonical Kivori asset blob compiled at build time.
///
/// The device consumes the compiled asset representation and never parses the
/// source artwork at runtime.
#[cfg(feature = "embedded")]
static ASSETS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kivori.assets"));

/// ESP-IDF-compatible application descriptor required by the ESP32-C3
/// bootloader/flash tooling.
#[cfg(feature = "embedded")]
esp_bootloader_esp_idf::esp_app_desc!();

/// Firmware entry point.
///
/// `esp_hal::main` installs the ESP32-C3 vector table. This function never
/// returns.
#[cfg(feature = "embedded")]
#[esp_hal::main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let clock = kivori_firmware::bsp::clock();

    // -------------------------------------------------------------------------
    // Physical production runtime
    //
    // Hardware-validated ESP32-C3 + ST7789 240x240 profile.
    //
    // Owns:
    // - native USB Serial/JTAG
    // - SPI2
    // - physical display GPIOs
    // - ST7789
    // - Kivori asset blob
    // - production runtime
    //
    // Never returns.
    // -------------------------------------------------------------------------

    #[cfg(feature = "physical-st7789")]
    {
        kivori_firmware::physical_st7789::run_mode(peripherals, clock, ASSETS);
    }

    // -------------------------------------------------------------------------
    // Wokwi production-runtime simulation
    //
    // Executes the same runtime::run loop as physical firmware, but with the
    // simulation-only board/display profile.
    // -------------------------------------------------------------------------

    #[cfg(all(feature = "wokwi-runtime", not(feature = "physical-st7789")))]
    {
        kivori_firmware::wokwi_runtime::run_mode(peripherals, clock, ASSETS);
    }

    // -------------------------------------------------------------------------
    // External Wokwi serial/protocol test
    //
    // Owns native USB Serial/JTAG and receives externally injected protocol
    // frames.
    // -------------------------------------------------------------------------

    #[cfg(all(
        feature = "wokwi-serial",
        not(feature = "wokwi-runtime"),
        not(feature = "physical-st7789")
    ))]
    {
        use kivori_firmware::ports::Clock;

        let mut io = kivori_firmware::bsp::serial(peripherals.USB_DEVICE);

        kivori_firmware::external::run(&mut io, || clock.now_ms());
    }

    // -------------------------------------------------------------------------
    // Generic Wokwi SPI/RGB565 probe
    //
    // Simulation-only. It verifies SPI activity and tile transfer but does not
    // establish physical controller, pin, orientation, color, or offset facts.
    // -------------------------------------------------------------------------

    #[cfg(all(
        feature = "wokwi-spi",
        not(feature = "wokwi-serial"),
        not(feature = "wokwi-runtime"),
        not(feature = "physical-st7789")
    ))]
    {
        let _passed = kivori_firmware::spi_probe::run(peripherals);

        let _ = &clock;
    }

    // -------------------------------------------------------------------------
    // Internal on-target Wokwi self-test
    // -------------------------------------------------------------------------

    #[cfg(all(
        feature = "wokwi",
        not(feature = "wokwi-serial"),
        not(feature = "wokwi-spi"),
        not(feature = "wokwi-runtime"),
        not(feature = "physical-st7789")
    ))]
    {
        let _passed = kivori_firmware::selftest::run(&clock);

        let _ = peripherals;
    }

    // -------------------------------------------------------------------------
    // Plain `embedded` fallback
    //
    // This mode intentionally performs only basic board bring-up. A physical
    // display is selected explicitly through a hardware-profile feature such
    // as `physical-st7789`.
    // -------------------------------------------------------------------------

    #[cfg(not(any(
        feature = "wokwi",
        feature = "wokwi-serial",
        feature = "wokwi-spi",
        feature = "wokwi-runtime",
        feature = "physical-st7789"
    )))]
    {
        use kivori_firmware::ports::Clock;

        let mut serial = kivori_firmware::bsp::serial(peripherals.USB_DEVICE);

        let boot_ms = clock.now_ms();

        esp_println::println!(
            "KIVORI boot: clock+serial up at {boot_ms}ms; \
             no display runtime selected"
        );

        let _ = (&mut serial, ASSETS);
    }

    // Self-test/probe/plain-embedded modes that return park here.
    //
    // The physical and Wokwi production runtimes are excluded because their
    // run loops already return `!`.
    #[cfg(not(any(
        feature = "wokwi-serial",
        feature = "wokwi-runtime",
        feature = "physical-st7789"
    )))]
    loop {
        esp_hal::delay::Delay::new().delay_millis(1000);
    }
}
