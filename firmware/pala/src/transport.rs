//! USB Serial/JTAG transport (T071).
//!
//! The real device transport: a **FIFO-aware, bounded, non-blocking** [`Transport`] over the ESP32-C3's
//! USB Serial/JTAG peripheral. The production run loop and the Wokwi external-serial mode both drive this
//! adapter, so the receive/transmit path under test is the shipping one, not a stand-in.
//!
//! # Why bounded and non-blocking matters
//!
//! The endpoint FIFO is 64 bytes ("Up to 64-byte data" per the SoC, and esp-hal's own `write` documents
//! "chunks of up to 64 bytes"). `esp_hal`'s `write` + `flush_tx` pair **blocks until the host drains the
//! FIFO**, which on a device whose host has stopped reading would stall the render loop indefinitely.
//! This adapter therefore uses the `_nb` primitives: it accepts at most one FIFO's worth per call, stops
//! the moment the hardware reports `WouldBlock`, and returns the number of bytes genuinely accepted — the
//! [`Transport`] contract. Receiving uses `drain_rx_fifo`, which empties the RX FIFO in one call instead of
//! byte-at-a-time polling.
//!
//! Because a single `write` can be short, [`TxBuffered`] wraps this adapter for the run loop: the
//! dispatcher writes whole frames into a queue that comfortably exceeds `MAX_WIRE`, and the loop pumps the
//! queue toward the hardware every tick. Without it, a full FIFO would truncate a frame mid-flight.
//!
//! Physical USB behaviour (host enumeration as `0x303A:0x1001`, real throughput, genuine transmit stalls
//! when the host stops draining) is NOT verified by simulation and remains a hardware task (T115/T116).

use crate::ports::Transport;
use esp_hal::peripherals::USB_DEVICE;
use esp_hal::usb_serial_jtag::{UsbSerialJtag, UsbSerialJtagRx, UsbSerialJtagTx};
use esp_hal::Blocking;

pub use crate::tx_buffer::{TxBuffered, FIFO_BYTES, TX_QUEUE_BYTES};

/// A [`Transport`] over USB Serial/JTAG.
///
/// The peripheral is split into its receive and transmit halves, because `drain_rx_fifo` — the FIFO-aware
/// bulk read — only exists on the receive half.
pub struct UsbJtagTransport<'d> {
    rx: UsbSerialJtagRx<'d, Blocking>,
    tx: UsbSerialJtagTx<'d, Blocking>,
}

impl<'d> UsbJtagTransport<'d> {
    /// Takes the USB Serial/JTAG peripheral and exposes it as a transport.
    pub fn new(usb_device: USB_DEVICE<'d>) -> Self {
        let (rx, tx) = UsbSerialJtag::new(usb_device).split();
        Self { rx, tx }
    }

    /// Writes raw bytes, **blocking** until the FIFO drains. Used only for the simulation harnesses' text
    /// markers, where a stalled marker is better than a lost one; protocol frames go through
    /// [`Transport::write`], which never blocks.
    pub fn write_all(&mut self, bytes: &[u8]) {
        let _ = self.tx.write(bytes);
        let _ = self.tx.flush_tx();
    }
}

impl Transport for UsbJtagTransport<'_> {
    /// Errors are reported as the unit type: the caller's recovery (drop the link, reconnect) does not
    /// depend on which USB error occurred, and no error detail may reach a log (ADR-0005).
    type Error = ();

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // Drains whatever the RX FIFO holds right now and returns immediately when it is empty.
        let n = self.rx.drain_rx_fifo(buf);
        #[cfg(feature = "debug-payloads")]
        crate::debug_payloads::dump("rx", &buf[..n]);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        // Bounded: never offer the hardware more than one FIFO per call.
        let limit = buf.len().min(FIFO_BYTES);
        let mut accepted = 0;
        while accepted < limit {
            // Non-blocking: `WouldBlock` means the FIFO is full, so stop and report what was taken.
            if self.tx.write_byte_nb(buf[accepted]).is_err() {
                break;
            }
            accepted += 1;
        }
        // Best-effort, non-blocking flush: a busy FIFO must not stall the caller.
        let _ = self.tx.flush_tx_nb();
        #[cfg(feature = "debug-payloads")]
        crate::debug_payloads::dump("tx", &buf[..accepted]);
        Ok(accepted)
    }
}
