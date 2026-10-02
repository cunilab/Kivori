//! M1 desk rendering: the non-Buddy views, the indicator/feedback chrome drawn over any view, and
//! the recovery-hold takeover.
//!
//! Same rules as the rest of the renderer: integer math only, a pure function of its inputs, and
//! frame-absolute coordinates clipped by [`TileBand::set`], so a frame stitched from tiles equals
//! the full-frame render on the host and on the device.

use kivori_framebuffer::TileBand;
use kivori_model::desk::DeskView;
use kivori_model::presentation::ValueDisplay;
use kivori_model::Rgb565;

/// The recovery-hold takeover, owning the whole screen. `percent` is time-based hold progress,
/// 0..=100. Draws every pixel of the band.
pub fn render_recovery(band: &mut TileBand, percent: u8) {
    let _ = percent;
    band.fill(Rgb565::BLACK);
}

/// The full-screen view for a non-Buddy [`kivori_model::desk::DisplayMode`]. Draws every pixel
/// of the band. `overlay` is the live volume value, which the Volume view prefers over
/// `view.status.volume_percent` while it is in force.
pub fn render_mode(band: &mut TileBand, view: &DeskView, overlay: Option<ValueDisplay>) {
    let _ = (view, overlay);
    band.fill(Rgb565::BLACK);
}

/// Indicators, the high-load cue, the local press acknowledgement and the action feedback badge,
/// drawn over whatever view is below. Touches only the pixels it draws.
pub fn render_chrome(band: &mut TileBand, view: &DeskView) {
    let _ = (band, view);
}
