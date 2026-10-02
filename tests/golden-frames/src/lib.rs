//! Golden-frame / frame-hash determinism harness (host, std). Phase 4 uses a **code-defined**
//! [`TestScene`] to exercise the renderer's determinism, tiling, per-scene frame selection, and
//! hashing (FR-033, SC-005, SC-009) without the compiled-asset pipeline (that arrives in Phase 5).

use kivori_framebuffer::TileBand;
use kivori_model::{FrameRate, Rect, Rgb565};
use kivori_renderer::{render_tile, Scene};

mod desk_frames;
pub use desk_frames::{desk_frames, render_desk, DeskFrame};

/// Display dimension (square) used by the harness.
pub const DIM: u16 = 240;

/// A deterministic, code-defined test scene: a white 40×40 square moving horizontally over a blue
/// background, looping at 10 fps across 8 frames.
pub struct TestScene;

impl TestScene {
    /// Background color.
    pub const BG: Rgb565 = Rgb565::from_rgb888(0, 0, 255);
    /// Foreground (square) color.
    pub const FG: Rgb565 = Rgb565::WHITE;
}

impl Scene for TestScene {
    fn frame_rate(&self) -> FrameRate {
        FrameRate::fps(10)
    }

    fn frame_count(&self) -> u16 {
        8
    }

    fn pixel(&self, x: u16, y: u16, frame: u16) -> Rgb565 {
        let sx = (frame * 20) % 200;
        if x >= sx && x < sx + 40 && (40..80).contains(&y) {
            Self::FG
        } else {
            Self::BG
        }
    }
}

/// Renders the full 240×240 frame of `scene` at `elapsed_ms` into a fresh buffer (row-major).
#[must_use]
pub fn render_full<S: Scene>(scene: &S, elapsed_ms: u32) -> Vec<Rgb565> {
    let mut buf = vec![Rgb565::from_raw(0); DIM as usize * DIM as usize];
    let mut band = TileBand::new(Rect::new(0, 0, DIM, DIM), &mut buf).expect("full-frame band");
    render_tile(scene, elapsed_ms, &mut band);
    buf
}
