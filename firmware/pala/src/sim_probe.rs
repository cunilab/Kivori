//! Simulation probes (`wokwi` feature): the adapters the in-firmware self-test drives.
//!
//! Both sit behind the SAME hardware-neutral ports the real board adapters implement, so the device
//! core under test is byte-identical to the shipping one:
//!
//! * [`LoopbackTransport`] — a [`Transport`] whose "host" side the internal self-test writes into. The
//!   framing, CRC, sequence, and dispatch paths under it are the real ones, but the serial peripheral is
//!   not involved; for that, see [`crate::external`], which takes bytes injected from outside the device
//!   through the real USB Serial/JTAG transport.
//! * [`TileProbe`] — a [`DisplaySink`] that records tile geometry and content hashes instead of driving
//!   a panel. It proves RGB565 tile generation and change-driven flushing without a full framebuffer
//!   (an ESP32-C3 cannot spare 115 KB for one) and without asserting anything about a real controller.
//!
//! Neither probe is a renderer: pixels come from the canonical `kivori-renderer` exactly as on device.

use crate::ports::{DisplaySink, Transport};
use core::convert::Infallible;
use heapless::Vec;
use kivori_framebuffer::hash_rgb565;
use kivori_model::{Rect, Rgb565};

/// Byte capacity of each direction of the loopback pipe (one full wire packet plus slack).
pub const PIPE_CAPACITY: usize = 2048;
/// Maximum tile records the probe keeps (one frame plus transition slack).
pub const MAX_TILE_RECORDS: usize = 64;

/// An in-firmware byte pipe standing in for the host serial link.
#[derive(Default)]
pub struct LoopbackTransport {
    to_device: Vec<u8, PIPE_CAPACITY>,
    from_device: Vec<u8, PIPE_CAPACITY>,
}

impl LoopbackTransport {
    /// An empty pipe.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            to_device: Vec::new(),
            from_device: Vec::new(),
        }
    }

    /// Simulated host → device: queues `bytes` for the dispatcher to read.
    pub fn host_send(&mut self, bytes: &[u8]) -> bool {
        self.to_device.extend_from_slice(bytes).is_ok()
    }

    /// Device → simulated host: borrows everything the device has written.
    #[must_use]
    pub fn device_output(&self) -> &[u8] {
        &self.from_device
    }

    /// Clears the device's output buffer.
    pub fn clear_output(&mut self) {
        self.from_device.clear();
    }
}

impl Transport for LoopbackTransport {
    type Error = Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let n = buf.len().min(self.to_device.len());
        buf[..n].copy_from_slice(&self.to_device[..n]);
        let mut rest: Vec<u8, PIPE_CAPACITY> = Vec::new();
        let _ = rest.extend_from_slice(&self.to_device[n..]);
        self.to_device = rest;
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let space = PIPE_CAPACITY - self.from_device.len();
        let n = space.min(buf.len());
        let _ = self.from_device.extend_from_slice(&buf[..n]);
        Ok(n)
    }
}

/// One observed tile flush: where it went and what it contained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRecord {
    /// Target window on the panel.
    pub rect: Rect,
    /// Content hash of the tile's RGB565 pixels (the same hash the goldens use).
    pub hash: u64,
    /// Pixel count received for this tile.
    pub pixels: usize,
}

/// A [`DisplaySink`] that records tile flushes instead of driving a panel.
#[derive(Default)]
pub struct TileProbe {
    records: Vec<TileRecord, MAX_TILE_RECORDS>,
    /// Total flushes seen (may exceed the record capacity).
    pub flushes: u32,
}

impl TileProbe {
    /// A probe with no observations.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
            flushes: 0,
        }
    }

    /// The recorded tile flushes, oldest first.
    #[must_use]
    pub fn records(&self) -> &[TileRecord] {
        &self.records
    }

    /// Forgets all observations (used to measure the next frame's flushes in isolation).
    pub fn reset(&mut self) {
        self.records.clear();
        self.flushes = 0;
    }
}

impl DisplaySink for TileProbe {
    type Error = Infallible;

    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        self.flushes = self.flushes.saturating_add(1);
        let _ = self.records.push(TileRecord {
            rect,
            hash: hash_rgb565(pixels),
            pixels: pixels.len(),
        });
        Ok(())
    }
}
