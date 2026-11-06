//! Raw-payload tracing, behind the `debug-payloads` **development** feature (T102; FR-032, ADR-0005 §3).
//!
//! Raw wire bytes are the one thing the logging policy forbids on a shipping device: they can contain a
//! host-supplied identity or state history. This module is the only place in the firmware that can turn
//! bytes into text, and the whole module — type, function, and format string — is compiled out unless
//! `debug-payloads` is enabled. The guard is therefore structural, not a runtime `if`.
//!
//! `scripts/check-release-surface.sh` proves both directions: the symbols are absent from a production
//! firmware library and present when the feature is on.

use core::cell::RefCell;
use core::fmt::Write;

use critical_section::Mutex;
use rp2040_hal::gpio::bank0::{Gpio0, Gpio1};
use rp2040_hal::gpio::{FunctionUart, Pin, PullDown};
use rp2040_hal::pac::UART0;
use rp2040_hal::uart::{Enabled, UartPeripheral};

/// UART0 on GP0 (TX) / GP1 (RX): the only text channel, since the USB link carries the protocol.
/// Neither pin is fitted on the carrier; clip a serial adapter to the Zero's GP0 pad.
pub type DebugUart = UartPeripheral<
    Enabled,
    UART0,
    (
        Pin<Gpio0, FunctionUart, PullDown>,
        Pin<Gpio1, FunctionUart, PullDown>,
    ),
>;

static UART: Mutex<RefCell<Option<DebugUart>>> = Mutex::new(RefCell::new(None));

/// Installs the trace output. Until then [`dump`] prints nothing.
pub fn init(uart: DebugUart) {
    critical_section::with(|cs| UART.borrow_ref_mut(cs).replace(uart));
}

/// Bytes printed per direction, so a large frame cannot flood the serial link.
const MAX_DUMP_BYTES: usize = 32;

/// Hex-dumps up to [`MAX_DUMP_BYTES`] of `bytes`, tagged with a direction (`"rx"` / `"tx"`).
///
/// Only reachable in a `debug-payloads` build. Never call this from a code path that is compiled into a
/// release artifact.
pub fn dump(direction: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let shown = bytes.len().min(MAX_DUMP_BYTES);
    // Blocks with interrupts off for the few ms a line takes at 115200 baud: acceptable for a
    // development trace, and the input interrupt only fires late.
    critical_section::with(|cs| {
        let mut uart = UART.borrow_ref_mut(cs);
        let Some(uart) = uart.as_mut() else {
            return;
        };
        let _ = write!(
            uart,
            "KIVORI-DEBUG-PAYLOAD {direction} len={} bytes=",
            bytes.len()
        );
        for byte in &bytes[..shown] {
            let _ = write!(uart, "{byte:02x}");
        }
        if shown < bytes.len() {
            let _ = write!(uart, "…");
        }
        let _ = write!(uart, "\r\n");
    });
}
