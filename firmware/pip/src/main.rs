#![no_std]
#![no_main]

//! Kivori ESP32-C3 firmware entry point.
//!
//! The binary selects its firmware mode through Cargo features:
//!
//! - `physical-st7789` — production Kivori runtime on the hardware-validated
//!   ESP32-C3 + ST7789 240x240 physical profile.
//! - `embedded` only — brings up the ESP32-C3 clock and native USB
//!   Serial/JTAG transport without selecting a display profile.
//!
//! Building this binary requires the `embedded` feature. `physical-st7789`
//! enables `embedded` through `Cargo.toml`.

#[cfg(feature = "embedded")]
use esp_backtrace as _;

/// Canonical Kivori asset blob compiled at build time.
///
/// The device consumes the compiled asset representation and never parses the
/// source artwork at runtime.
#[cfg(feature = "embedded")]
static ASSETS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kivori.assets"));

// ESP-IDF-compatible application descriptor required by the ESP32-C3
// bootloader/flash tooling. (A plain comment: rustdoc cannot attach a doc comment to a macro call.)
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
    // Plain `embedded` fallback
    //
    // This mode intentionally performs only basic board bring-up. A physical
    // display is selected explicitly through a hardware-profile feature such
    // as `physical-st7789`.
    // -------------------------------------------------------------------------

    #[cfg(not(feature = "physical-st7789"))]
    {
        use kivori_firmware::ports::Clock;

        let mut serial = kivori_firmware::bsp::serial(peripherals.USB_DEVICE);

        let boot_ms = clock.now_ms();

        esp_println::println!(
            "KIVORI boot: clock+serial up at {boot_ms}ms; \
             no display runtime selected"
        );

        let _ = (&mut serial, ASSETS);

        // Nothing else runs in this mode, so it parks here.
        loop {
            esp_hal::delay::Delay::new().delay_millis(1000);
        }
    }
}
