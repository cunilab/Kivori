//! Production USB-serial transport (research R-5).
//!
//! A blocking `serialport` handle behind the hardware-neutral [`SerialLink`] boundary, plus VID/PID
//! port enumeration. The background device thread drives it. Real-device behaviour (Windows control
//! lines, throughput, reconnect timing) is validated manually — there is no hardware in host CI, so
//! nothing here is exercised end-to-end by automated tests.

use crate::device::discovery::PortCandidate;
use crate::device::transport::SerialLink;
use std::io::{Read, Write};
use std::time::Duration;

/// Nominal baud (USB Serial/JTAG ignores the rate, but the API requires one).
const BAUD: u32 = 115_200;

/// Enumerates the host's serial ports, mapping USB ports to their VID/PID.
#[must_use]
pub fn enumerate() -> Vec<PortCandidate> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|port| {
            let (vid, pid) = match port.port_type {
                serialport::SerialPortType::UsbPort(info) => (Some(info.vid), Some(info.pid)),
                _ => (None, None),
            };
            PortCandidate::new(port.port_name, vid, pid)
        })
        .collect()
}

/// A [`SerialLink`] over a blocking `serialport` handle configured for non-blocking reads.
pub struct SerialPortLink {
    port: Box<dyn serialport::SerialPort>,
    /// Bytes [`SerialPortLink::wait`] received, handed out by the next [`SerialLink::read`].
    pending: Vec<u8>,
}

impl SerialPortLink {
    /// Opens `port_name` with a zero read timeout so [`SerialLink::read`] never blocks.
    ///
    /// # Errors
    /// Returns the `serialport` error if the port cannot be opened (in use, access denied, removed).
    pub fn open(port_name: &str) -> serialport::Result<Self> {
        let port = serialport::new(port_name, BAUD)
            .timeout(Duration::from_millis(0))
            .open()?;
        Ok(Self {
            port,
            pending: Vec::new(),
        })
    }

    /// Blocks until bytes arrive or `timeout` passes, so the device thread reacts to input as it
    /// lands instead of on its next fixed tick (validation row 3.14). Received bytes are kept for
    /// the next `read`; reads themselves stay non-blocking.
    ///
    /// # Errors
    /// Returns the I/O error if the port failed; the caller's next `read` surfaces it as link loss.
    pub fn wait(&mut self, timeout: Duration) -> std::io::Result<()> {
        if !self.pending.is_empty() {
            return Ok(());
        }
        self.port.set_timeout(timeout)?;
        let mut chunk = [0u8; 256];
        let result = self.port.read(&mut chunk);
        self.port.set_timeout(Duration::ZERO)?;
        match result {
            Ok(n) => {
                self.pending.extend_from_slice(&chunk[..n]);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl SerialLink for SerialPortLink {
    type Error = std::io::Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if !self.pending.is_empty() {
            let n = buf.len().min(self.pending.len());
            buf[..n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            return Ok(n);
        }
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            // A zero-timeout read reports "no data available now" as a timeout; that is 0 bytes, not an error.
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(e),
        }
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        Write::write(&mut self.port, buf)
    }
}
