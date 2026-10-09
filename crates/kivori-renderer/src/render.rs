//! The renderer core: render a [`Scene`] into a [`TileBand`] at a canonical elapsed time.

use crate::frame_select::select_frame;
use kivori_framebuffer::TileBand;
use kivori_model::{ElapsedMs, FrameRate, Rgb565};

/// A renderable scene: a pure, deterministic mapping from `(x, y, frame)` to an RGB565 color, plus
/// the scene's own animation rate and loop length.
///
/// Implementations MUST be deterministic and integer-only (no floating point, no wall-clock). The
/// canonical compiled-asset scene implements this trait.
pub trait Scene {
    /// The scene's effective animation frame rate.
    fn frame_rate(&self) -> FrameRate;

    /// The number of frames in the animation loop (`0`/`1` = static).
    fn frame_count(&self) -> u16;

    /// The color at absolute display coordinate `(x, y)` on the given `frame`.
    fn pixel(&self, x: u16, y: u16, frame: u16) -> Rgb565;
}

/// Renders `scene` at `elapsed_ms` into `band`, filling every pixel in the band's rectangle.
///
/// The active frame is selected deterministically from `elapsed_ms` via the scene's frame rate
/// (see [`select_frame`]); rendering the same band at the same `elapsed_ms` always produces identical
/// bytes (FR-019).
pub fn render_tile<S: Scene + ?Sized>(scene: &S, elapsed_ms: ElapsedMs, band: &mut TileBand) {
    let frame = select_frame(scene, elapsed_ms);
    let rect = band.rect();
    let mut y = rect.y;
    while (y as u32) < rect.bottom() {
        let mut x = rect.x;
        while (x as u32) < rect.right() {
            band.set(x, y, scene.pixel(x, y, frame));
            x += 1;
        }
        y += 1;
    }
}
