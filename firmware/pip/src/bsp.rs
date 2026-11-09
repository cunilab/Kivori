//! Board support shared by every `embedded` mode: clocks and the monotonic timer (T070).
//!
//! Display, input and USB wiring are not here: the only board profile is the physical one in
//! [`crate::profile::physical_st7789`], and `physical_st7789` brings its SPI bus, panel, pins and
//! USB up itself from the [`Board`] peripherals.

use crate::clock::TimerClock;
use rp2040_hal::clocks::{init_clocks_and_plls, ClocksManager};
use rp2040_hal::{pac, Timer, Watchdog};

/// The RP2040-Zero's crystal.
const XOSC_HZ: u32 = 12_000_000;

/// What is left of the chip after clock bring-up.
pub struct Board {
    /// The running monotonic clock (the timer is started).
    pub clock: TimerClock,
    /// The same timer as a blocking delay (panel bring-up).
    pub timer: Timer,
    /// System, peripheral and USB clocks.
    pub clocks: ClocksManager,
    /// Peripheral reset control, needed to bring up each block.
    pub resets: pac::RESETS,
    /// GPIO bank 0 (pin routing) and its pads.
    pub io: pac::IO_BANK0,
    /// Pad control for bank 0.
    pub pads: pac::PADS_BANK0,
    /// Single-cycle IO: the GPIO output/input registers.
    pub sio: pac::SIO,
    /// The display's SPI block.
    pub spi1: pac::SPI1,
    /// PWM slices: the backlight.
    pub pwm: pac::PWM,
    /// USB controller registers and its dual-port RAM.
    pub usb: (pac::USBCTRL_REGS, pac::USBCTRL_DPRAM),
    /// ADC, for the die temperature.
    pub adc: pac::ADC,
    /// UART0, the `debug-payloads` trace output.
    pub uart0: pac::UART0,
}

/// Starts the crystal, PLLs, watchdog tick and the 1 MHz timer. Panics when the crystal or a PLL
/// does not start: there is nothing to run on.
#[must_use]
pub fn init(mut pac: pac::Peripherals) -> Board {
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let clocks = init_clocks_and_plls(
        XOSC_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .expect("clock bring-up");
    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    Board {
        clock: TimerClock::new(),
        timer,
        clocks,
        resets: pac.RESETS,
        io: pac.IO_BANK0,
        pads: pac.PADS_BANK0,
        sio: pac.SIO,
        spi1: pac.SPI1,
        pwm: pac.PWM,
        usb: (pac.USBCTRL_REGS, pac.USBCTRL_DPRAM),
        adc: pac.ADC,
        uart0: pac.UART0,
    }
}
