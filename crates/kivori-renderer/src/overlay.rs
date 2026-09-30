//! The canonical volume overlay.
//!
//! Solid rects only — no glyphs, no font tables, no asset-blob content. Integer
//! arithmetic throughout, so host preview and firmware produce byte-identical
//! output (engineering principles 2 and 3).
//!
//! Each `ValueConfidence` MUST render distinguishably: an optimistic preview that
//! looked like confirmed truth would defeat the reason the field exists.

use kivori_framebuffer::TileBand;
use kivori_model::presentation::ValueConfidence;
use kivori_model::Rgb565;

const BAR_X: u16 = 24;
const BAR_W: u16 = 192;
const BAR_Y: u16 = 180;
const BAR_H: u16 = 16;
const BORDER: u16 = 2;

/// Dashed-fill period used by `Unverified`, in pixels.
const DASH_PERIOD: u16 = 6;
const DASH_ON: u16 = 3;

/// Draw the volume bar into `band`. Coordinates are frame-absolute; pixels outside
/// the band are clipped by `TileBand::set`.
pub fn render_volume_overlay(
    band: &mut TileBand,
    percent: u8,
    confidence: ValueConfidence,
    at_boundary: bool,
) {
    // Tiles the bar does not cover are left untouched (and skip the per-pixel clip loop).
    let r = band.rect();
    if r.x >= BAR_X + BAR_W
        || r.x.saturating_add(r.w) <= BAR_X
        || r.y >= BAR_Y + BAR_H
        || r.y.saturating_add(r.h) <= BAR_Y
    {
        return;
    }
    let percent = if percent > 100 { 100 } else { percent };

    let track = if at_boundary {
        Rgb565::WHITE
    } else {
        Rgb565::from_rgb888(96, 96, 96)
    };
    let fill = Rgb565::WHITE;

    // Track outline.
    let inner_x_span = BAR_X + BORDER..BAR_X + BAR_W - BORDER;
    let inner_y_span = BAR_Y + BORDER..BAR_Y + BAR_H - BORDER;
    for x in BAR_X..BAR_X + BAR_W {
        for y in BAR_Y..BAR_Y + BAR_H {
            let on_edge = !inner_x_span.contains(&x) || !inner_y_span.contains(&y);
            if on_edge {
                band.set(x, y, track);
            }
        }
    }

    // Filled width: integer-only, exact at both bounds.
    let inner_x = BAR_X + BORDER;
    let inner_w = BAR_W - 2 * BORDER;
    let inner_y = BAR_Y + BORDER;
    let inner_h = BAR_H - 2 * BORDER;
    let filled = (u32::from(inner_w) * u32::from(percent) / 100) as u16;

    for x in inner_x..inner_x + filled {
        for y in inner_y..inner_y + inner_h {
            let paint = match confidence {
                // Solid: observed OS truth.
                ValueConfidence::Confirmed => true,
                // Hollow: only the top and bottom rows, so a preview reads as an
                // outline rather than a filled, settled value.
                ValueConfidence::Preview => y == inner_y || y == inner_y + inner_h - 1,
                // Broken fill: dispatched but unobservable.
                ValueConfidence::Unverified => (x - inner_x) % DASH_PERIOD < DASH_ON,
            };
            if paint {
                band.set(x, y, fill);
            }
        }
    }
}
