//! DEVELOPMENT ONLY: detent -> on-panel feedback latency probe (Slice 002 checklist row 14).
//!
//! Compiled only with the `latency-probe` feature. One clock — the device's own ms clock — spans the
//! whole loop: a `Detent` `InputEvent` leaves the device at `t_detent`; the desktop answers with a
//! `Presentation`; the first frame composed after that presentation (with a new revision) was
//! applied finishes its blocking SPI/DMA tile writes at `t_flush_done`. The readout shows
//! `t_flush_done - t_detent` (last) and the running max in the top-left corner.
//!
//! Only the oldest unmatched detent is tracked: later detents in the same burst are not measured
//! separately, so each reading is the worst case for its burst.

use kivori_framebuffer::TileBand;
use kivori_model::Rgb565;

/// A detent with no applied `Presentation` within this window is dropped.
pub const EXPIRE_MS: u32 = 1000;
/// Readings clamp to three digits.
pub const MAX_SHOWN: u16 = 999;

/// What the panel shows: last reading and running max, `None` until a first measurement.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Readout {
    /// Most recent latency, ms.
    pub last: Option<u16>,
    /// Largest latency seen since boot, ms.
    pub max: Option<u16>,
}

/// Pairs a detent with the first flushed frame that carries the desktop's answer to it.
#[derive(Debug, Default)]
pub struct LatencyProbe {
    detent_at: Option<u32>,
    presented: bool,
    readout: Readout,
}

impl LatencyProbe {
    /// No pending detent, no readings.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            detent_at: None,
            presented: false,
            readout: Readout {
                last: None,
                max: None,
            },
        }
    }

    /// A `Detent` `InputEvent` was actually transmitted at `now`.
    pub fn on_detent(&mut self, now: u32) {
        self.expire(now);
        if self.detent_at.is_none() {
            self.detent_at = Some(now);
        }
    }

    /// A `Presentation` with a new revision was applied at `now`. Only counts if a detent is already
    /// pending, so an answer to an earlier (dropped) detent cannot complete a later one.
    pub fn on_presentation(&mut self, now: u32) {
        self.expire(now);
        if self.detent_at.is_some() {
            self.presented = true;
        }
    }

    /// A render pass finished writing every changed tile at `now`. Returns the completed reading.
    pub fn on_frame_flushed(&mut self, now: u32) -> Option<u16> {
        self.expire(now);
        if !self.presented {
            return None;
        }
        let start = self.detent_at.take()?;
        self.presented = false;
        let ms = now.wrapping_sub(start).min(u32::from(MAX_SHOWN)) as u16;
        self.readout.last = Some(ms);
        self.readout.max = Some(self.readout.max.map_or(ms, |m| m.max(ms)));
        Some(ms)
    }

    /// The values the panel should show.
    #[must_use]
    pub const fn readout(&self) -> Readout {
        self.readout
    }

    fn expire(&mut self, now: u32) {
        if let Some(start) = self.detent_at {
            if !self.presented && now.wrapping_sub(start) >= EXPIRE_MS {
                self.detent_at = None;
            }
        }
    }
}

// 7-segment glyphs from solid rects. Segment bits: a=0 b=1 c=2 d=3 e=4 f=5 g=6.
const T: u16 = 2; // stroke
const S: u16 = 6; // segment length
const GW: u16 = S + 2 * T;
const GH: u16 = 2 * S + 3 * T;
const GAP: u16 = 2;
/// Top-left of the readout box, frame-absolute.
pub const BOX_X: u16 = 4;
/// Top-left of the readout box, frame-absolute.
pub const BOX_Y: u16 = 4;
const BOX_W: u16 = GAP + 4 * (GW + GAP);
const BOX_H: u16 = GAP + 2 * (GH + GAP);
const DIGITS: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];
const GLYPH_L: u8 = 0x38;
const GLYPH_H: u8 = 0x76;
/// `(x, y, w, h)` of each segment relative to the glyph origin.
const SEGMENTS: [(u16, u16, u16, u16); 7] = [
    (T, 0, S, T),
    (T + S, T, T, S),
    (T + S, 2 * T + S, T, S),
    (T, 2 * S + 2 * T, S, T),
    (0, 2 * T + S, T, S),
    (0, T, T, S),
    (T, S + T, S, T),
];

/// Draws `L<last>` over `H<max>` into `band` as the final layer. Draws nothing before the first
/// measurement (so the frame is pixel-identical to a non-probe build until then); tiles the box does
/// not cover are left untouched.
pub fn draw(band: &mut TileBand, readout: Readout) {
    let (Some(last), Some(max)) = (readout.last, readout.max) else {
        return;
    };
    let r = band.rect();
    if r.x >= BOX_X + BOX_W
        || r.x.saturating_add(r.w) <= BOX_X
        || r.y >= BOX_Y + BOX_H
        || r.y.saturating_add(r.h) <= BOX_Y
    {
        return;
    }
    fill(band, BOX_X, BOX_Y, BOX_W, BOX_H, Rgb565::BLACK);
    for (row, (label, v)) in [(GLYPH_L, last), (GLYPH_H, max)].into_iter().enumerate() {
        let v = v.min(MAX_SHOWN);
        let digit = |place: u16| {
            if v >= place || place == 1 {
                DIGITS[usize::from(v / place % 10)]
            } else {
                0 // leading blank
            }
        };
        let y = BOX_Y + GAP + row as u16 * (GH + GAP);
        for (i, glyph) in [label, digit(100), digit(10), digit(1)]
            .into_iter()
            .enumerate()
        {
            let x = BOX_X + GAP + i as u16 * (GW + GAP);
            for (bit, &(sx, sy, sw, sh)) in SEGMENTS.iter().enumerate() {
                if glyph & (1 << bit) != 0 {
                    fill(band, x + sx, y + sy, sw, sh, Rgb565::WHITE);
                }
            }
        }
    }
}

fn fill(band: &mut TileBand, x: u16, y: u16, w: u16, h: u16, color: Rgb565) {
    for py in y..y + h {
        for px in x..x + w {
            band.set(px, py, color);
        }
    }
}
