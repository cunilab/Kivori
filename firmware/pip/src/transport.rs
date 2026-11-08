//! USB CDC-ACM transport (T071).
//!
//! The real device transport: a **bounded, non-blocking** [`Transport`] over the RP2040's USB
//! controller (`usbd-serial`), driven by the production run loop. The host sees a serial port;
//! the wire protocol on top is unchanged.
//!
//! # Interrupt-driven
//!
//! `usb-device` must be polled at least every 10 ms, and the RP2040 HAL only applies `SET_ADDRESS`
//! on the poll after the status stage. A display flush keeps the run loop away from the transport
//! for tens of milliseconds (a full frame is about 59 ms over SPI), so polling from `read`/`write`
//! alone could miss enumeration. The `USBCTRL_IRQ` handler therefore services the stack (and acts
//! on the bootloader touch) whenever the controller raises an event; the device and class live in a
//! critical-section mutex shared with [`Transport::write`], which also polls (harmless, and it moves
//! a packet out right away). The handler also moves what the host sent into a 1 KiB queue that
//! [`Transport::read`] drains (`usb_service`, host-tested): the controller re-raises the interrupt
//! until an OUT packet is read, so data left in the endpoint starved the loop on the first real
//! unit (no reply, a 750 ms loop).
//!
//! # Why bounded and non-blocking matters
//!
//! `usbd-serial` buffers 128 bytes each way. [`Transport::write`] accepts at most one 64-byte packet
//! per call and reports the bytes genuinely taken (`0` while the buffer is full, e.g. with no host
//! reading), so a host that has stopped reading can never stall the render loop. [`TxBuffered`]
//! wraps this adapter for the run loop: the dispatcher writes whole frames into a queue that
//! comfortably exceeds `MAX_WIRE`, and the loop pumps the queue toward the hardware every tick.
//!
//! Physical USB behaviour is not covered by the host tests. Checked on an RP2040-Zero (#179,
//! 2026-11-06): enumeration as `0x2E8A:0x000A`, the handshake (HelloAck in ~60 ms), internals, and
//! the 1200-baud touch into the UF2 bootloader and back without a replug.

use core::cell::RefCell;

use crate::bootloader::is_touch;
use crate::ports::Transport;
use crate::usb_service::{service, UsbPort};
use critical_section::Mutex;
use rp2040_hal::pac::{self, interrupt};
use rp2040_hal::usb::UsbBus;
use usb_device::class_prelude::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDevice, UsbDeviceBuilder, UsbVidPid};
use usb_device::UsbError;
use usbd_serial::SerialPort;

pub use crate::tx_buffer::{TxBuffered, FIFO_BYTES, TX_QUEUE_BYTES};

/// Raspberry Pi's vendor id with the Pico SDK's CDC product id. Hosts only use it to nominate a
/// port; the Kivori handshake decides whether it is one (as with the old shared Espressif id).
const USB_VID_PID: UsbVidPid = UsbVidPid(0x2E8A, 0x000A);

type Usb = (UsbDevice<'static, UsbBus>, SerialPort<'static, UsbBus>);

/// The USB stack, shared between the `USBCTRL_IRQ` handler and the transport methods.
static USB: Mutex<RefCell<Option<Usb>>> = Mutex::new(RefCell::new(None));

/// Bytes received from the host, moved out of the endpoint by [`poll`]; the run loop drains them.
const RX_BYTES_BUFFERED: usize = 1024;
static RX: Mutex<RefCell<heapless::Deque<u8, RX_BYTES_BUFFERED>>> =
    Mutex::new(RefCell::new(heapless::Deque::new()));

impl UsbPort for Usb {
    fn poll(&mut self) {
        self.0.poll(&mut [&mut self.1]);
    }

    fn read_packet(&mut self, buf: &mut [u8]) -> Option<usize> {
        self.1.read(buf).ok()
    }
}

/// Services the USB stack and moves what the host sent into [`RX`] (`usb_service::service`, which
/// documents why the OUT data must be read here), then acts on a bootloader touch.
fn poll(cs: critical_section::CriticalSection<'_>, usb: &mut Usb) {
    service(usb, &mut RX.borrow_ref_mut(cs));
    if is_touch(usb.1.line_coding().data_rate(), usb.1.dtr()) {
        // Mass storage + PICOBOOT, no activity LED, nothing disabled. Does not return.
        rp2040_hal::rom_data::reset_to_usb_boot(0, 0);
    }
}

#[interrupt]
fn USBCTRL_IRQ() {
    critical_section::with(|cs| {
        if let Some(usb) = USB.borrow_ref_mut(cs).as_mut() {
            poll(cs, usb);
        }
    });
}

/// A [`Transport`] over USB CDC-ACM. A handle: the stack itself lives in [`USB`].
pub struct UsbCdcTransport;

impl UsbCdcTransport {
    /// Builds the CDC class and device on `bus`, then starts servicing it from the USB interrupt.
    /// `serial_number` is the text shown to the host (derived from the device id, never the raw
    /// flash id).
    pub fn new(bus: &'static UsbBusAllocator<UsbBus>, serial_number: &'static str) -> Self {
        let serial = SerialPort::new(bus);
        let device = UsbDeviceBuilder::new(bus, USB_VID_PID)
            .strings(&[StringDescriptors::default()
                .manufacturer("Kivori")
                .product("Kivori Pip")
                .serial_number(serial_number)])
            .expect("USB strings")
            .device_class(usbd_serial::USB_CLASS_CDC)
            .build();
        critical_section::with(|cs| USB.borrow_ref_mut(cs).replace((device, serial)));
        // SAFETY: the handler only touches `USB`, which is set up and behind a critical-section mutex.
        unsafe { pac::NVIC::unmask(pac::Interrupt::USBCTRL_IRQ) };
        Self
    }
}

impl Transport for UsbCdcTransport {
    /// Errors are reported as the unit type: the caller's recovery (drop the link, reconnect) does not
    /// depend on which USB error occurred, and no error detail may reach a log (ADR-0005).
    type Error = ();

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // What the USB interrupt has already moved out of the endpoint (see `poll`).
        let n = critical_section::with(|cs| {
            let mut rx = RX.borrow_ref_mut(cs);
            let mut n = 0;
            while n < buf.len() {
                let Some(byte) = rx.pop_front() else { break };
                buf[n] = byte;
                n += 1;
            }
            n
        });
        #[cfg(feature = "debug-payloads")]
        crate::debug_payloads::dump("rx", &buf[..n]);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        // Bounded: never offer the stack more than one packet per call.
        let limit = buf.len().min(FIFO_BYTES);
        let written = critical_section::with(|cs| {
            let mut usb = USB.borrow_ref_mut(cs);
            let usb = usb.as_mut().ok_or(UsbError::InvalidState)?;
            poll(cs, usb);
            let written = usb.1.write(&buf[..limit]);
            // Moves the packet toward the endpoint right away instead of waiting for the next poll.
            poll(cs, usb);
            written
        });
        let accepted = match written {
            Ok(n) => n,
            // The buffer is full: report nothing taken, the caller retries on the next tick.
            Err(UsbError::WouldBlock) => 0,
            Err(_) => return Err(()),
        };
        #[cfg(feature = "debug-payloads")]
        crate::debug_payloads::dump("tx", &buf[..accepted]);
        Ok(accepted)
    }
}
