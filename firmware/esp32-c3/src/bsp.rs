//! Board support: clocks, USB Serial/JTAG bring-up, and SPI display bring-up (T070; research R-1/R-2/R-3).
//!
//! # Nothing here chooses hardware facts
//!
//! Every board-specific decision is a **parameter**, never a default:
//!
//! * which GPIOs carry SCK/MOSI/CS/D/C/RST — the caller passes the concrete `esp-hal` pin handles;
//! * which panel controller drives the bus — [`crate::display::init_panel`] is generic over `mipidsi`'s
//!   `Model` and the caller supplies it;
//! * where the visible area sits inside the controller framebuffer — [`crate::display::PanelGeometry`]
//!   takes the offset explicitly and has no `Default`.
//!
//! Consequently this module compiles for a board whose panel is still unknown, and **no physical Kivori
//! profile exists here**. The only concrete profile in the tree is the simulation-only one in
//! [`crate::profile`], which belongs to `sim/wokwi/diagram-spi.json`. The real profile must come from the
//! hardware design (see `docs/validation.md` rows 1.23-1.25: controller identity, pin map, offsets).
//!
//! What simulation cannot establish: SPI clock rate margins, drive strength, pull-ups, level shifting,
//! backlight polarity, or whether the chosen GPIOs are actually routed on the physical board.

use crate::clock::EspClock;
use crate::transport::UsbJtagTransport;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::delay::Delay;
use esp_hal::gpio::interconnect::PeripheralOutput;
use esp_hal::peripherals::USB_DEVICE;
use esp_hal::spi::master::{Config as SpiConfig, ConfigError, Instance as SpiInstance, Spi};
use esp_hal::spi::Mode;
use esp_hal::time::Rate;
use esp_hal::Blocking;

/// SPI clock for the display bus.
///
/// 20 MHz is inside the range every candidate 240x240 controller documents, but the *physical* ceiling
/// depends on trace length and the panel module, so this is a starting point to be confirmed on hardware
/// (T119), not a validated value.
pub const DISPLAY_SPI_HZ: u32 = 20_000_000;

/// Why board bring-up failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BringUpError {
    /// The SPI peripheral rejected the requested configuration.
    SpiConfig(ConfigError),
    /// The chip-select pin could not be driven to its idle level.
    ChipSelect,
}

/// A brought-up SPI bus paired with its chip-select line.
pub type DisplayBus<'d, CS> = ExclusiveDevice<Spi<'d, Blocking>, CS, Delay>;

/// Brings up the monotonic clock (T073).
///
/// Backed by the system timer, so it reads identically on hardware and in simulation.
#[must_use]
pub const fn clock() -> EspClock {
    EspClock::new()
}

/// Brings up USB Serial/JTAG as the device [`crate::ports::Transport`] (T071).
pub fn serial<'d>(usb_device: USB_DEVICE<'d>) -> UsbJtagTransport<'d> {
    UsbJtagTransport::new(usb_device)
}

/// Brings up the display SPI bus in mode 0, MSB-first, at [`DISPLAY_SPI_HZ`].
///
/// `sck`/`mosi` are the caller's pin handles; MISO is deliberately not wired, because the tile pipeline
/// only ever writes. The returned device owns `cs`, which it asserts around each transaction.
///
/// # Errors
/// [`BringUpError`] if the frequency cannot be configured on this SoC, or the CS pin cannot be driven.
pub fn display_bus<'d, CS>(
    spi: impl SpiInstance + 'd,
    sck: impl PeripheralOutput<'d>,
    mosi: impl PeripheralOutput<'d>,
    cs: CS,
) -> Result<DisplayBus<'d, CS>, BringUpError>
where
    CS: embedded_hal::digital::OutputPin,
{
    let config = SpiConfig::default()
        .with_frequency(Rate::from_hz(DISPLAY_SPI_HZ))
        .with_mode(Mode::_0);
    let bus = Spi::new(spi, config)
        .map_err(BringUpError::SpiConfig)?
        .with_sck(sck)
        .with_mosi(mosi);
    ExclusiveDevice::new(bus, cs, Delay::new()).map_err(|_| BringUpError::ChipSelect)
}
