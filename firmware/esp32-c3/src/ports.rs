//! Hardware-neutral adapter ports.
//!
//! The device core depends only on these three traits. The real board (Phase 8) implements them over
//! USB Serial/JTAG, SPI, and a hardware timer; the host [`crate::sim`] adapters implement them in
//! memory. Nothing above this layer knows which is in use (constraint 4).

use kivori_model::input::InputLevels;
use kivori_model::{ElapsedMs, Rect, Rgb565};

/// A bounded, non-blocking byte transport (USB Serial/JTAG on device; an in-memory pipe in sim).
pub trait Transport {
    /// Transport-specific error type.
    type Error;

    /// Reads any immediately-available bytes into `buf`, returning how many were read (`0` if none).
    /// Never blocks.
    ///
    /// # Errors
    /// Returns [`Self::Error`] on an unrecoverable transport failure.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;

    /// Writes as many bytes of `buf` as fit without blocking, returning how many were accepted.
    ///
    /// # Errors
    /// Returns [`Self::Error`] on an unrecoverable transport failure.
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error>;
}

/// A pixel sink for one tile region (the SPI panel on device; a capture buffer in sim).
pub trait DisplaySink {
    /// Sink-specific error type.
    type Error;

    /// Blits `pixels` (row-major RGB565, `rect.w * rect.h` long) to the panel window `rect`.
    ///
    /// # Errors
    /// Returns [`Self::Error`] on an unrecoverable display failure.
    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error>;
}

/// A monotonic millisecond clock, measured from device boot.
pub trait Clock {
    /// Milliseconds since boot (monotonic, non-decreasing).
    fn now_ms(&self) -> ElapsedMs;
}

/// Instantaneous physical input levels.
///
/// Sampling and pin mapping live in the adapter; ALL semantics (debounce,
/// detent qualification, gesture formation) live above this port so they are
/// provable without hardware.
pub trait InputSource {
    /// Read the current levels. MUST NOT block.
    fn sample(&mut self) -> InputLevels;
}
