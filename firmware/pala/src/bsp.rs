//! Board support shared by every `embedded` mode: the monotonic clock and USB Serial/JTAG bring-up
//! (T070; research R-1/R-2).
//!
//! Display and input wiring are not here: the only board profile is the verified physical one in
//! [`crate::profile::physical_st7789`], and `physical_st7789` brings its SPI bus, panel, and pins up
//! itself.

use crate::clock::EspClock;
use crate::transport::UsbJtagTransport;
use esp_hal::peripherals::USB_DEVICE;

/// Brings up the monotonic clock (T073), backed by the system timer.
#[must_use]
pub const fn clock() -> EspClock {
    EspClock::new()
}

/// Brings up USB Serial/JTAG as the device [`crate::ports::Transport`] (T071).
pub fn serial<'d>(usb_device: USB_DEVICE<'d>) -> UsbJtagTransport<'d> {
    UsbJtagTransport::new(usb_device)
}
