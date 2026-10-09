//! Hardware-neutral adapter ports.
//!
//! The device core depends only on these three traits. The real board implements them over
//! USB Serial/JTAG, SPI, and a hardware timer; the host [`crate::sim`] adapters implement them in
//! memory. Nothing above this layer knows which is in use (constraint 4).

use crate::render::{TILE_H, TILE_PIXELS, TILE_W};
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

    /// Advances bytes accepted by an outbound buffer without requiring another message.
    /// Unbuffered transports have no pending work.
    ///
    /// # Errors
    /// Returns the underlying transport error when buffered output cannot be advanced.
    fn drain_pending(&mut self) -> Result<usize, Self::Error> {
        Ok(0)
    }

    /// Drops outbound bytes not yet handed to the hardware, and makes sure a frame cut short on
    /// the wire is terminated. Called when a new session starts: frames queued for nobody (while
    /// no host was reading) are stale and must not crowd out the `HelloAck`. Unbuffered
    /// transports have nothing to drop.
    fn discard_unsent(&mut self) {}
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

    /// Blits a block of whole tiles as ONE panel window. `tiles` holds `cols` tiles per tile row,
    /// tile rows top to bottom, each tile `TILE_W x TILE_H` row-major and contiguous (exactly how
    /// the renderer's frame buffer stores them); `rect` covers all of them. The panel receives the
    /// pixels in window (row-major) order, so the result is identical to one [`Self::blit_tile`]
    /// per tile, with the per-window command overhead paid once.
    ///
    /// The default splits the block back into tiles, so a sink only overrides this to save windows.
    ///
    /// # Errors
    /// Returns [`Self::Error`] on an unrecoverable display failure.
    fn blit_tiles(&mut self, rect: Rect, tiles: &[Rgb565], cols: u16) -> Result<(), Self::Error> {
        let cols = usize::from(cols.max(1));
        for (i, tile) in tiles.as_chunks::<TILE_PIXELS>().0.iter().enumerate() {
            let at = Rect::new(
                rect.x + (i % cols) as u16 * TILE_W,
                rect.y + (i / cols) as u16 * TILE_H,
                TILE_W,
                TILE_H,
            );
            self.blit_tile(at, tile)?;
        }
        Ok(())
    }
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

    /// Hands every level snapshot observed since the last call to `f`, oldest first, each with the
    /// device-ms it was captured at, ending with the current levels at `now_ms`.
    ///
    /// The default is one [`Self::sample`] per call, which is all a polled source can offer. The
    /// physical adapter overrides it with snapshots captured on every pin edge, so encoder steps and
    /// switch edges that happen while a frame is composed or flushed are not lost. MUST NOT block.
    fn drain(&mut self, now_ms: ElapsedMs, f: &mut dyn FnMut(InputLevels, ElapsedMs)) {
        f(self.sample(), now_ms);
    }

    /// Edges the source had to drop because its capture queue was full (cumulative). Polled
    /// sources capture nothing and drop nothing; only a diagnostic readout reads this.
    fn dropped_edges(&self) -> u32 {
        0
    }
}
