#![no_std]
#![no_main]

//! Kivori RP2040-Zero firmware entry point.
//!
//! The binary selects its firmware mode through Cargo features:
//!
//! - `physical-st7789` — production Kivori runtime on the RP2040-Zero + ST7789 240x240 physical
//!   profile.
//! - `embedded` only — brings up the RP2040 clocks and timer without selecting a display profile.
//!
//! Building this binary requires the `embedded` feature. `physical-st7789`
//! enables `embedded` through `Cargo.toml`.

#[cfg(feature = "embedded")]
use rp2040_hal::pac;

/// Canonical Kivori asset blob compiled at build time.
///
/// The device consumes the compiled asset representation and never parses the
/// source artwork at runtime.
#[cfg(feature = "embedded")]
static ASSETS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kivori.assets"));

/// The second-stage bootloader the RP2040 ROM runs from the first 256 bytes of flash. It sets up
/// quad-SPI XIP for the Zero's Winbond W25Q16 flash.
#[cfg(feature = "embedded")]
#[link_section = ".boot2"]
#[used]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

/// A panic parks the core: there is no console to report to (the USB link carries the protocol),
/// and the BOOTSEL button recovers the board.
#[cfg(feature = "embedded")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        cortex_m::asm::wfi();
    }
}

/// Firmware entry point.
///
/// `rp2040_hal::entry` installs the reset handler (and clears stale spinlocks). This function never
/// returns.
#[cfg(feature = "embedded")]
#[rp2040_hal::entry]
fn main() -> ! {
    let peripherals = pac::Peripherals::take().expect("peripherals are taken once");
    let board = kivori_firmware::bsp::init(peripherals);

    // -------------------------------------------------------------------------
    // Physical production runtime
    //
    // RP2040-Zero + ST7789 240x240 profile.
    //
    // Owns:
    // - USB CDC serial
    // - SPI1
    // - physical display GPIOs
    // - ST7789
    // - Kivori asset blob
    // - production runtime
    //
    // Never returns.
    // -------------------------------------------------------------------------

    #[cfg(feature = "physical-st7789")]
    {
        // Paint the unused stack now, while little of it is in use, so the device internals snapshot
        // can report how deep it has ever gone (the asset parse below is the deepest it goes).
        kivori_firmware::chip_probe::paint_stack();
        // Parsed first, while the stack is empty: it is the deepest call chain in the firmware.
        let blob = kivori_firmware::physical_st7789::load_assets(ASSETS);
        kivori_firmware::physical_st7789::run_mode(board, blob, ASSETS.len());
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
        let _ = (&board, ASSETS);

        // Nothing else runs in this mode, so it parks here.
        loop {
            cortex_m::asm::wfi();
        }
    }
}
