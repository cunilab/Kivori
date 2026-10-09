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
//!
//! Next to `L`/`H` a second column shows what separates a slow frame or a lost detent from a late
//! one (all clamped to three digits): `F` ms between the last two rendered frames, `t` tiles
//! flushed by the last frame (the probe's own box is among them), `o` rotary edges dropped by the
//! full interrupt queue, `n` invalid quadrature transitions. The previous frame's values are
//! shown, since the current frame is still being composed.

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

/// Frame and input health shown beside the latency readout; every value is already clamped to
/// [`MAX_SHOWN`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Milliseconds between the starts of the last two rendered frames.
    pub frame_ms: u16,
    /// Tiles the last rendered frame flushed.
    pub tiles: u16,
    /// Rotary edges the interrupt queue dropped because it was full.
    pub dropped_edges: u16,
    /// Electrically impossible quadrature transitions seen.
    pub invalid_transitions: u16,
}

impl Stats {
    /// Whether a counter that should never move has: worth showing even before a first reading.
    const fn has_faults(&self) -> bool {
        self.dropped_edges > 0 || self.invalid_transitions > 0
    }
}

/// Pairs a detent with the first flushed frame that carries the desktop's answer to it.
#[derive(Debug, Default)]
pub struct LatencyProbe {
    detent_at: Option<u32>,
    presented: bool,
    readout: Readout,
    frame_started: Option<u32>,
    frame_ms: u16,
    tiles: u16,
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
            frame_started: None,
            frame_ms: 0,
            tiles: 0,
        }
    }

    /// A render pass starts at `now`: the gap to the previous one is the frame interval.
    pub fn on_frame_start(&mut self, now: u32) {
        if let Some(previous) = self.frame_started {
            self.frame_ms = now.wrapping_sub(previous).min(u32::from(MAX_SHOWN)) as u16;
        }
        self.frame_started = Some(now);
    }

    /// The render pass that just ran flushed `tiles` tiles.
    pub fn on_tiles_flushed(&mut self, tiles: u32) {
        self.tiles = tiles.min(u32::from(MAX_SHOWN)) as u16;
    }

    /// The frame figures plus the input counters the caller owns.
    #[must_use]
    pub fn stats(&self, dropped_edges: u32, invalid_transitions: u32) -> Stats {
        let clamp = |n: u32| n.min(u32::from(MAX_SHOWN)) as u16;
        Stats {
            frame_ms: self.frame_ms,
            tiles: self.tiles,
            dropped_edges: clamp(dropped_edges),
            invalid_transitions: clamp(invalid_transitions),
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
const CELL_W: u16 = GAP + 4 * (GW + GAP);
const CELL_H: u16 = GH + GAP;
const BOX_W: u16 = 2 * CELL_W;
const BOX_H: u16 = GAP + 3 * CELL_H;
const DIGITS: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];
const GLYPH_L: u8 = 0x38;
const GLYPH_H: u8 = 0x76;
const GLYPH_F: u8 = 0x71;
const GLYPH_T: u8 = 0x78;
const GLYPH_O: u8 = 0x5C;
const GLYPH_N: u8 = 0x54;
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

/// Draws the readout box as the final layer: `L<last>` `H<max>` `F<frame ms>` in the left column and
/// `t<tiles>` `o<dropped edges>` `n<invalid>` in the right. Draws nothing until a first latency
/// reading exists or a fault counter moves (so the frame is pixel-identical to a non-probe build
/// until then); tiles the box does not cover are left untouched.
pub fn draw(band: &mut TileBand, readout: Readout, stats: Stats) {
    let (last, max) = match (readout.last, readout.max) {
        (Some(last), Some(max)) => (last, max),
        _ if stats.has_faults() => (0, 0),
        _ => return,
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
    let cells = [
        (GLYPH_L, last),
        (GLYPH_H, max),
        (GLYPH_F, stats.frame_ms),
        (GLYPH_T, stats.tiles),
        (GLYPH_O, stats.dropped_edges),
        (GLYPH_N, stats.invalid_transitions),
    ];
    for (i, (label, v)) in cells.into_iter().enumerate() {
        let (column, row) = (i / 3, i % 3);
        cell(
            band,
            BOX_X + column as u16 * CELL_W,
            BOX_Y + GAP + row as u16 * CELL_H,
            label,
            v,
        );
    }
}

/// One label glyph and three digits (leading zeros blank) at `(x0, y)`.
fn cell(band: &mut TileBand, x0: u16, y: u16, label: u8, v: u16) {
    let v = v.min(MAX_SHOWN);
    let digit = |place: u16| {
        if v >= place || place == 1 {
            DIGITS[usize::from(v / place % 10)]
        } else {
            0 // leading blank
        }
    };
    for (i, glyph) in [label, digit(100), digit(10), digit(1)]
        .into_iter()
        .enumerate()
    {
        let x = x0 + GAP + i as u16 * (GW + GAP);
        for (bit, &(sx, sy, sw, sh)) in SEGMENTS.iter().enumerate() {
            if glyph & (1 << bit) != 0 {
                fill(band, x + sx, y + sy, sw, sh, Rgb565::WHITE);
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
