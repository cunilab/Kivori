//! M1 desk rendering: the non-Buddy views, the view-switch animation, the status/feedback chrome
//! drawn over any view, and the recovery-hold takeover.
//!
//! Same rules as the rest of the renderer: integer math only, a pure function of its inputs, and
//! frame-absolute coordinates clipped to the band, so a frame stitched from tiles equals the
//! full-frame render on the host and on the device. Shapes are per-pixel integer coverage
//! functions over a clipped bounding box (tiles a shape misses cost one rectangle test); edges are
//! anti-aliased by blending with what the band already holds, which depends only on the pixel
//! itself, so tiling cannot change the result. Text uses the `embedded-graphics` ISO-8859-1 mono
//! fonts, so now-playing text (Latin-1) renders as sent.
//!
//! Unknown (`None`) values are always drawn as an explicit unknown (`--`, `--:--`, a dashed
//! gauge, "No media info"), never as zero or a guess.
//!
//! Visual language: one dark palette around the mascot background `#0c101c`, a single cyan
//! accent for live values, amber for "attention" (high load, unverified, recovery), rounded
//! monoline digits for every big number, 270-degree ring gauges, a fixed top status row
//! (feedback badge left, time centre, indicators right) owned by the chrome, and on the Buddy
//! view a bottom legend saying what each control does.

use core::convert::Infallible;

use embedded_graphics::mono_font::{iso_8859_1 as fonts, MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::raw::RawU16;
use embedded_graphics::pixelcolor::{IntoStorage, Rgb565 as EgRgb565};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::Pixel;
use kivori_framebuffer::TileBand;
use kivori_model::desk::{
    ControlLabels, CpuHistory, DeskStatus, DeskView, DisplayMode, FeedbackKind, MediaInfo,
    MediaStatus, MediaText, VIEW_TRANSITION_MS,
};
use kivori_model::presentation::{ValueConfidence, ValueDisplay};
use kivori_model::Rgb565;

const fn rgb(r: u8, g: u8, b: u8) -> Rgb565 {
    Rgb565::from_rgb888(r, g, b)
}

// ---------------------------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------------------------

/// Background, matching the mascot scenes.
const BG: Rgb565 = rgb(0x0c, 0x10, 0x1c);
/// Raised surfaces: cards and chips.
const CARD: Rgb565 = rgb(0x17, 0x1e, 0x30);
/// Unfilled gauge track.
const TRACK: Rgb565 = rgb(0x25, 0x2e, 0x45);
/// Hairlines, gridlines, a gauge track at its limit.
const LINE: Rgb565 = rgb(0x3b, 0x46, 0x62);
/// Tertiary text; a value that is known but muted.
const FAINT: Rgb565 = rgb(0x5c, 0x66, 0x80);
/// Secondary text and unknown values.
const MUTED: Rgb565 = rgb(0x8a, 0x94, 0xa8);
const TEXT: Rgb565 = Rgb565::WHITE;
/// The one accent: live, observed values.
const ACCENT: Rgb565 = rgb(0x4f, 0xc3, 0xf7);
const ACCENT_DIM: Rgb565 = rgb(0x14, 0x33, 0x4a);
/// Attention: high load, unverified, recovery.
const AMBER: Rgb565 = rgb(0xf5, 0xa5, 0x24);
const AMBER_DIM: Rgb565 = rgb(0x46, 0x31, 0x12);
/// Feedback badges only.
const GREEN: Rgb565 = rgb(0x3c, 0xc4, 0x7c);
const BLUE: Rgb565 = rgb(0x4c, 0x8d, 0xf6);
const RED: Rgb565 = rgb(0xe5, 0x48, 0x4d);

// Typography: ISO-8859-1 mono fonts throughout (a superset of ASCII).
/// Track titles and large single-line messages.
const F_TITLE: &MonoFont<'static> = &fonts::FONT_10X20;
/// Secondary lines (artist).
const F_BODY: &MonoFont<'static> = &fonts::FONT_8X13;
/// Captions, chips and status labels.
const F_LABEL: &MonoFont<'static> = &fonts::FONT_7X13_BOLD;
/// Fine print inside cards.
const F_SMALL: &MonoFont<'static> = &fonts::FONT_6X10;
/// The status-row clock.
const F_STATUS: &MonoFont<'static> = &fonts::FONT_9X15_BOLD;

const SCREEN: i32 = 240;
const CENTER: i32 = SCREEN / 2;

// ---------------------------------------------------------------------------------------------
// Pixels and coverage
// ---------------------------------------------------------------------------------------------

/// Coverage is measured in sixteenths of a pixel.
const FULL: i32 = 16;

/// `a` moved `cov` sixteenths of the way to `b`, per RGB565 channel.
fn mix(a: Rgb565, b: Rgb565, cov: i32) -> Rgb565 {
    let (a, b) = (i32::from(a.raw()), i32::from(b.raw()));
    let ch = |shift: i32, mask: i32| {
        let (ca, cb) = ((a >> shift) & mask, (b >> shift) & mask);
        ((ca + (cb - ca) * cov / FULL) & mask) << shift
    };
    Rgb565::from_raw((ch(11, 0x1f) | ch(5, 0x3f) | ch(0, 0x1f)) as u16)
}

/// Calls `paint` for every `(x, y)` in `[x0, x1) x [y0, y1)` that lies inside the band and draws
/// the colour it returns at that coverage, blended over the band. The loop is clipped to the band
/// first, so tiles skip shapes they miss.
fn paint(
    band: &mut TileBand,
    (x0, y0, x1, y1): (i32, i32, i32, i32),
    paint: impl Fn(i32, i32) -> Option<(Rgb565, i32)>,
) {
    let r = band.rect();
    let x0 = x0.max(i32::from(r.x));
    let y0 = y0.max(i32::from(r.y));
    let x1 = x1.min(i32::from(r.x) + i32::from(r.w));
    let y1 = y1.min(i32::from(r.y) + i32::from(r.h));
    for y in y0..y1 {
        for x in x0..x1 {
            let Some((col, cov)) = paint(x, y) else {
                continue;
            };
            let (ux, uy) = (x as u16, y as u16);
            if cov >= FULL {
                band.set(ux, uy, col);
            } else if cov > 0 {
                if let Some(dst) = band.get(ux, uy) {
                    band.set(ux, uy, mix(dst, col, cov));
                }
            }
        }
    }
}

/// [`paint`] with one colour.
fn shape(
    band: &mut TileBand,
    bbox: (i32, i32, i32, i32),
    col: Rgb565,
    cov: impl Fn(i32, i32) -> i32,
) {
    paint(band, bbox, |x, y| Some((col, cov(x, y))));
}

fn rect(band: &mut TileBand, x: i32, y: i32, w: i32, h: i32, col: Rgb565) {
    shape(band, (x, y, x + w, y + h), col, |_, _| FULL);
}

fn isqrt(n: u64) -> u64 {
    let (mut op, mut res, mut one) = (n, 0u64, 1u64 << 62);
    while one > op {
        one >>= 2;
    }
    while one != 0 {
        if op >= res + one {
            op -= res + one;
            res = (res >> 1) + one;
        } else {
            res >>= 1;
        }
        one >>= 2;
    }
    res
}

/// Distance `sqrt(d2)` in sixteenths of a pixel.
fn dist16(d2: i64) -> i32 {
    isqrt(d2.max(0) as u64 * 256) as i32
}

/// Coverage of a pixel `sqrt(d2)` from a shape's core when the shape fully covers distances up
/// to `r` and fades out over the next pixel (so `d2 <= r*r` is solid, as an aliased predicate
/// would draw it, plus a soft rim).
fn round_cov(d2: i64, r: i32) -> i32 {
    let r = i64::from(r);
    if d2 <= r * r {
        FULL
    } else if d2 >= (r + 1) * (r + 1) {
        0
    } else {
        ((r as i32 + 1) * FULL - dist16(d2)).clamp(0, FULL)
    }
}

/// Coverage of a ring of pixels whose distance is in `ri..=ro`, soft on both edges.
fn ring_cov(d2: i64, ro: i32, ri: i32) -> i32 {
    let outer = round_cov(d2, ro);
    if outer == 0 || ri <= 0 {
        return outer;
    }
    let r = i64::from(ri);
    let inner = if d2 >= r * r {
        FULL
    } else if d2 <= (r - 1) * (r - 1) {
        0
    } else {
        (dist16(d2) - (ri - 1) * FULL).clamp(0, FULL)
    };
    outer.min(inner)
}

fn ring(band: &mut TileBand, cx: i32, cy: i32, ro: i32, ri: i32, col: Rgb565) {
    shape(
        band,
        (cx - ro - 1, cy - ro - 1, cx + ro + 2, cy + ro + 2),
        col,
        |x, y| ring_cov(i64::from((x - cx) * (x - cx) + (y - cy) * (y - cy)), ro, ri),
    );
}

fn disc(band: &mut TileBand, cx: i32, cy: i32, r: i32, col: Rgb565) {
    ring(band, cx, cy, r, 0, col);
}

/// A line segment with round caps: every pixel within `r` of the segment, soft-edged.
fn line(band: &mut TileBand, (x0, y0): (i32, i32), (x1, y1): (i32, i32), r: i32, col: Rgb565) {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len2 = i64::from(dx * dx + dy * dy);
    let r64 = i64::from(r);
    shape(
        band,
        (
            x0.min(x1) - r - 1,
            y0.min(y1) - r - 1,
            x0.max(x1) + r + 2,
            y0.max(y1) + r + 2,
        ),
        col,
        |x, y| {
            let (px, py) = (x - x0, y - y0);
            let dot = i64::from(px * dx + py * dy);
            if len2 == 0 || dot <= 0 {
                return round_cov(i64::from(px * px + py * py), r);
            }
            if dot >= len2 {
                let (qx, qy) = (x - x1, y - y1);
                return round_cov(i64::from(qx * qx + qy * qy), r);
            }
            let cross = i64::from(px * dy - py * dx);
            let c2 = cross * cross;
            if c2 <= r64 * r64 * len2 {
                FULL
            } else if c2 >= (r64 + 1) * (r64 + 1) * len2 {
                0
            } else {
                ((r + 1) * FULL - isqrt((c2 * 256 / len2) as u64) as i32).clamp(0, FULL)
            }
        },
    );
}

/// A filled rectangle with corner radius `r`.
fn rrect(band: &mut TileBand, x: i32, y: i32, w: i32, h: i32, r: i32, col: Rgb565) {
    let (ix0, iy0, ix1, iy1) = (x + r, y + r, x + w - 1 - r, y + h - 1 - r);
    shape(band, (x, y, x + w, y + h), col, |px, py| {
        let dx = (ix0 - px).max(px - ix1).max(0);
        let dy = (iy0 - py).max(py - iy1).max(0);
        round_cov(i64::from(dx * dx + dy * dy), r)
    });
}

/// A filled triangle, soft-edged.
fn tri(band: &mut TileBand, a: (i32, i32), b: (i32, i32), c: (i32, i32), col: Rgb565) {
    let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    if area == 0 {
        return;
    }
    // Wind so the edge function is positive inside.
    let (b, c) = if area < 0 { (c, b) } else { (b, c) };
    let edge = |p: (i32, i32), q: (i32, i32)| {
        let (ex, ey) = (q.0 - p.0, q.1 - p.1);
        (p, ex, ey, dist16(i64::from(ex * ex + ey * ey)).max(1))
    };
    let edges = [edge(a, b), edge(b, c), edge(c, a)];
    shape(
        band,
        (
            a.0.min(b.0).min(c.0) - 1,
            a.1.min(b.1).min(c.1) - 1,
            a.0.max(b.0).max(c.0) + 2,
            a.1.max(b.1).max(c.1) + 2,
        ),
        col,
        |x, y| {
            let mut cov = FULL;
            for (p, ex, ey, len16) in edges {
                let e = ex * (y - p.1) - ey * (x - p.0);
                // Signed distance to the edge in sixteenths: e / len px.
                cov = cov.min((e * 256 / len16 + FULL / 2).clamp(0, FULL));
            }
            cov
        },
    );
}

// ---------------------------------------------------------------------------------------------
// Angles (1/4096 turn, clockwise from 12 o'clock)
// ---------------------------------------------------------------------------------------------

const TURN: i32 = 4096;
/// `atan(i / 32)` in turn units, `i` in `0..=32`.
const ATAN: [i32; 33] = [
    0, 20, 41, 61, 81, 101, 121, 140, 160, 179, 197, 216, 234, 252, 269, 286, 302, 318, 334, 349,
    364, 379, 393, 406, 419, 432, 445, 457, 469, 480, 491, 502, 512,
];
/// `sin(i / 64 of a quarter turn)` in Q14, `i` in `0..=64`.
const SIN: [i32; 65] = [
    0, 402, 804, 1205, 1606, 2006, 2404, 2801, 3196, 3590, 3981, 4370, 4756, 5139, 5520, 5897,
    6270, 6639, 7005, 7366, 7723, 8076, 8423, 8765, 9102, 9434, 9760, 10080, 10394, 10702, 11003,
    11297, 11585, 11866, 12140, 12406, 12665, 12916, 13160, 13395, 13623, 13842, 14053, 14256,
    14449, 14635, 14811, 14978, 15137, 15286, 15426, 15557, 15679, 15791, 15893, 15986, 16069,
    16143, 16207, 16261, 16305, 16340, 16364, 16379, 16384,
];

/// Linear interpolation in a table sampled every `step`.
fn table(t: &[i32], v: i32, step: i32) -> i32 {
    let i = (v / step) as usize;
    match t.get(i + 1) {
        Some(next) => t[i] + (next - t[i]) * (v % step) / step,
        None => t[t.len() - 1],
    }
}

/// The angle of `(dx, dy)` (screen axes), `0..TURN`.
fn turn_of(dx: i32, dy: i32) -> i32 {
    let (ax, ay) = (dx.abs(), dy.abs());
    if ax == 0 && ay == 0 {
        return 0;
    }
    let (lo, hi) = if ax <= ay { (ax, ay) } else { (ay, ax) };
    let a = table(&ATAN, lo * 1024 / hi, 32);
    let base = if ax <= ay { a } else { TURN / 4 - a };
    match (dx >= 0, dy <= 0) {
        (true, true) => base,
        (true, false) => TURN / 2 - base,
        (false, false) => TURN / 2 + base,
        (false, true) => (TURN - base) % TURN,
    }
}

/// `sin` in Q14 of an angle in turn units.
fn sin_q14(t: i32) -> i32 {
    let t = t.rem_euclid(TURN);
    let q = TURN / 4;
    match t / q {
        0 => table(&SIN, t, 16),
        1 => table(&SIN, TURN / 2 - t, 16),
        2 => -table(&SIN, t - TURN / 2, 16),
        _ => -table(&SIN, TURN - t, 16),
    }
}

/// The point `r` px from `(cx, cy)` at angle `t`.
fn polar(cx: i32, cy: i32, r: i32, t: i32) -> (i32, i32) {
    let (s, c) = (sin_q14(t), sin_q14(t + TURN / 4));
    (
        cx + (r * s + 8192).div_euclid(16384),
        cy - (r * c + 8192).div_euclid(16384),
    )
}

/// A ring band from `start` clockwise through `sweep`.
#[derive(Clone, Copy)]
struct Arc {
    cx: i32,
    cy: i32,
    ro: i32,
    ri: i32,
    start: i32,
    sweep: i32,
}

impl Arc {
    /// The classic gauge: 270 degrees, open at the bottom.
    const fn gauge(cx: i32, cy: i32, ro: i32, ri: i32) -> Arc {
        Arc {
            cx,
            cy,
            ro,
            ri,
            start: TURN * 5 / 8,
            sweep: TURN * 3 / 4,
        }
    }

    fn cap_r(&self) -> i32 {
        (self.ro - self.ri) / 2
    }

    /// The centre line at `rel` turns past the start.
    fn at(&self, rel: i32) -> (i32, i32) {
        polar(self.cx, self.cy, (self.ro + self.ri) / 2, self.start + rel)
    }

    /// About 7 px of dash at the centre line, in turn units, sized so the sweep holds an odd
    /// number of runs: a dashed full sweep starts and ends on a dash, with no sliver at the end.
    fn dash(&self) -> i32 {
        let n = (self.sweep * ((self.ro + self.ri) / 2) / (7 * 652)).max(1) | 1;
        self.sweep / n
    }

    fn inset(&self, by: i32) -> Arc {
        Arc {
            ro: self.ro - by,
            ri: self.ri + by,
            ..*self
        }
    }
}

/// The ring pixels of `a` between `from` and `to` (turns past `a.start`). With `dash > 0` only
/// every other `dash`-long run is drawn.
fn arc(band: &mut TileBand, a: &Arc, from: i32, to: i32, dash: i32, col: Rgb565) {
    if to <= from {
        return;
    }
    let Arc { cx, cy, ro, ri, .. } = *a;
    shape(
        band,
        (cx - ro - 1, cy - ro - 1, cx + ro + 2, cy + ro + 2),
        col,
        |x, y| {
            let (dx, dy) = (x - cx, y - cy);
            let cov = ring_cov(i64::from(dx * dx + dy * dy), ro, ri);
            if cov == 0 {
                return 0;
            }
            let rel = (turn_of(dx, dy) - a.start).rem_euclid(TURN);
            let off = dash > 0 && ((rel - from) / dash) % 2 == 1;
            if rel < from || rel >= to || off {
                0
            } else {
                cov
            }
        },
    );
}

/// A round end on `a` at `rel`, `shrink` px smaller than the band.
fn cap(band: &mut TileBand, a: &Arc, rel: i32, shrink: i32, col: Rgb565) {
    let (x, y) = a.at(rel);
    disc(band, x, y, a.cap_r() - shrink, col);
}

/// A gauge at `pct`: a rounded track, and a fill whose style carries the value's confidence:
/// solid when confirmed, an outline when it is a preview, dashed when unverified.
fn gauge(
    band: &mut TileBand,
    a: &Arc,
    pct: u8,
    confidence: ValueConfidence,
    fill: Rgb565,
    track: Rgb565,
) {
    let end = a.sweep * i32::from(pct.min(100)) / 100;
    arc(band, a, 0, a.sweep, 0, track);
    cap(band, a, 0, 0, track);
    cap(band, a, a.sweep, 0, track);
    match confidence {
        ValueConfidence::Unverified => arc(band, a, 0, end, a.dash(), fill),
        ValueConfidence::Confirmed | ValueConfidence::Preview => {
            arc(band, a, 0, end, 0, fill);
            cap(band, a, 0, 0, fill);
            cap(band, a, end, 0, fill);
            if confidence == ValueConfidence::Preview {
                // Hollow: carve the track back into the fill, leaving a 2 px outline.
                arc(band, &a.inset(2), 0, end, 0, track);
                cap(band, a, 0, 2, track);
                cap(band, a, end, 2, track);
            }
        }
    }
}

/// The whole gauge as a dashed outline: "the value is not known". A known 0 has a solid track.
fn unknown_gauge(band: &mut TileBand, a: &Arc) {
    arc(band, a, 0, a.sweep, a.dash(), LINE);
}

// ---------------------------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------------------------

fn eg_color(c: Rgb565) -> EgRgb565 {
    EgRgb565::from(RawU16::new(c.raw()))
}

/// A band restricted to columns `x0..x1`, for marquee text. With `fade > 0` pixels within that
/// many columns of an edge blend into what is beneath, so scrolling text dissolves at the edges.
struct Clip<'a, 'b> {
    band: &'a mut TileBand<'b>,
    x0: i32,
    x1: i32,
    fade: i32,
}

impl Dimensions for Clip<'_, '_> {
    fn bounding_box(&self) -> Rectangle {
        self.band.bounding_box()
    }
}

impl DrawTarget for Clip<'_, '_> {
    type Color = EgRgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            if p.x < self.x0 || p.x >= self.x1 || p.y < 0 {
                continue;
            }
            let (x, y) = (p.x as u16, p.y as u16);
            let col = Rgb565::from_raw(c.into_storage());
            let edge = (p.x - self.x0).min(self.x1 - 1 - p.x);
            match (self.fade > 0 && edge < self.fade, self.band.get(x, y)) {
                (true, Some(dst)) => {
                    self.band
                        .set(x, y, mix(dst, col, (edge + 1) * FULL / (self.fade + 1)))
                }
                _ => self.band.set(x, y, col),
            }
        }
        Ok(())
    }
}

fn advance(font: &MonoFont) -> i32 {
    font.character_size.width as i32 + font.character_spacing as i32
}

fn text_width(font: &MonoFont, s: &[u8]) -> i32 {
    s.len() as i32 * advance(font)
}

/// Latin-1 `s` with its top-left at `(x, y)`, clipped to columns `clip`. Glyphs outside the band
/// or the clip are skipped before any pixel work.
fn text_clipped(
    band: &mut TileBand,
    font: &MonoFont,
    s: &[u8],
    (x, y): (i32, i32),
    col: Rgb565,
    clip: (i32, i32, i32),
) {
    let r = band.rect();
    let (ry0, ry1) = (i32::from(r.y), i32::from(r.y) + i32::from(r.h));
    if y >= ry1 || y + font.character_size.height as i32 <= ry0 {
        return;
    }
    // The fade measures from the clip edges, not the band's.
    let (cx0, cx1, fade) = clip;
    let style = MonoTextStyle::new(font, eg_color(col));
    let step = advance(font);
    let x0 = cx0.max(i32::from(r.x));
    let x1 = cx1.min(i32::from(r.x) + i32::from(r.w));
    let mut target = Clip {
        band,
        x0: cx0,
        x1: cx1,
        fade,
    };
    for (i, &b) in s.iter().enumerate() {
        let gx = x + i as i32 * step;
        if gx + step <= x0 || gx >= x1 {
            continue;
        }
        let mut buf = [0u8; 4];
        let glyph = char::from(b).encode_utf8(&mut buf);
        let _ =
            Text::with_baseline(glyph, Point::new(gx, y), style, Baseline::Top).draw(&mut target);
    }
}

fn text(band: &mut TileBand, font: &MonoFont, s: &[u8], x: i32, y: i32, col: Rgb565) {
    text_clipped(band, font, s, (x, y), col, (0, SCREEN, 0));
}

fn text_c(band: &mut TileBand, font: &MonoFont, s: &[u8], cx: i32, y: i32, col: Rgb565) {
    text(band, font, s, cx - text_width(font, s) / 2, y, col);
}

/// Marquee scroll speed and the pause at the start of each pass.
const MARQUEE_PX_PER_S: u32 = 30;
const MARQUEE_HOLD_MS: u32 = 1_500;
/// Space between the end of the text and its next copy.
const MARQUEE_GAP: i32 = 48;
/// Columns over which scrolling text fades out at each edge.
const MARQUEE_FADE: i32 = 14;

/// One line of text in columns `x0..x1`: centred when it fits; otherwise it holds, then scrolls
/// left at a steady pace and wraps around, all derived from `elapsed_ms`.
fn marquee(
    band: &mut TileBand,
    font: &MonoFont,
    s: &[u8],
    (x0, x1): (i32, i32),
    y: i32,
    col: Rgb565,
    elapsed_ms: u32,
) {
    let w = text_width(font, s);
    if w <= x1 - x0 {
        text(band, font, s, (x0 + x1 - w) / 2, y, col);
        return;
    }
    let period = (w + MARQUEE_GAP) as u32;
    let scroll_ms = period * 1000 / MARQUEE_PX_PER_S;
    let t = elapsed_ms % (MARQUEE_HOLD_MS + scroll_ms);
    let off = t.saturating_sub(MARQUEE_HOLD_MS) * MARQUEE_PX_PER_S / 1000;
    let x = x0 - off as i32;
    text_clipped(band, font, s, (x, y), col, (x0, x1, MARQUEE_FADE));
    text_clipped(
        band,
        font,
        s,
        (x + period as i32, y),
        col,
        (x0, x1, MARQUEE_FADE),
    );
}

// ---------------------------------------------------------------------------------------------
// Numbers: rounded monoline seven-segment digits
// ---------------------------------------------------------------------------------------------

struct Digits {
    w: i32,
    h: i32,
    /// Stroke radius.
    r: i32,
    gap: i32,
}

/// The clock.
const BIG: Digits = Digits {
    w: 32,
    h: 62,
    r: 4,
    gap: 10,
};
/// The volume value.
const MID: Digits = Digits {
    w: 28,
    h: 50,
    r: 4,
    gap: 8,
};
/// Values inside small gauges.
const SMALL: Digits = Digits {
    w: 14,
    h: 24,
    r: 2,
    gap: 5,
};

/// Segments a b c d e f g as bits 0..=6, for digits 0..=9.
const SEGMENTS: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];

/// One digit with its top-left at `(x, y)`; `None` draws only the middle bar (an explicit
/// "unknown").
fn digit(band: &mut TileBand, x: i32, y: i32, d: Option<u8>, s: &Digits, col: Rgb565) {
    let (l, r, t, b) = (x + s.r, x + s.w - 1 - s.r, y + s.r, y + s.h - 1 - s.r);
    let m = (t + b) / 2;
    if d == Some(1) {
        // A centred stem with a flag reads better than a seven-segment "1" pushed right.
        let cx = x + s.w / 2;
        line(band, (cx, t), (cx, b), s.r, col);
        line(band, (cx - s.w / 4, t + s.w / 4), (cx, t), s.r, col);
        return;
    }
    let mask = d.map_or(0x40, |d| SEGMENTS[usize::from(d.min(9))]);
    let segs = [
        ((l, t), (r, t)), // a
        ((r, t), (r, m)), // b
        ((r, m), (r, b)), // c
        ((l, b), (r, b)), // d
        ((l, m), (l, b)), // e
        ((l, t), (l, m)), // f
        ((l, m), (r, m)), // g
    ];
    for (i, (p, q)) in segs.into_iter().enumerate() {
        if mask & (1 << i) != 0 {
            line(band, p, q, s.r, col);
        }
    }
}

fn digits_width(n: usize, s: &Digits) -> i32 {
    n as i32 * s.w + (n as i32 - 1).max(0) * s.gap
}

/// Digits left to right from `x`; returns the right edge.
fn digits(
    band: &mut TileBand,
    mut x: i32,
    y: i32,
    ds: &[Option<u8>],
    s: &Digits,
    col: Rgb565,
) -> i32 {
    for d in ds {
        digit(band, x, y, *d, s, col);
        x += s.w + s.gap;
    }
    x - s.gap
}

/// The decimal digits of `v` (clamped to 100).
fn decimal(v: u8) -> ([Option<u8>; 3], usize) {
    let v = v.min(100);
    if v >= 100 {
        ([Some(1), Some(0), Some(0)], 3)
    } else if v >= 10 {
        ([Some(v / 10), Some(v % 10), None], 2)
    } else {
        ([Some(v), None, None], 1)
    }
}

/// A percentage centred on `cx`: digits with a `%` half their height on their baseline. `None`
/// is `--`.
fn percent(band: &mut TileBand, v: Option<u8>, cx: i32, y: i32, s: &Digits, col: Rgb565) {
    let (ds, n) = match v {
        Some(v) => decimal(v),
        None => ([None, None, None], 2),
    };
    let vector = s.h >= 40;
    let sign_w = if vector {
        s.w * 3 / 5
    } else {
        advance(F_LABEL)
    };
    let sign = if v.is_some() {
        sign_w + s.gap / 2 + 2
    } else {
        0
    };
    let x0 = cx - (digits_width(n, s) + sign) / 2;
    let x1 = digits(band, x0, y, &ds[..n], s, col) + s.gap / 2 + 2;
    if v.is_none() {
        return;
    }
    if vector {
        let (w, h) = (sign_w, s.h / 2);
        let (x, y) = (x1, y + s.h - h);
        let ro = w / 4 + 1;
        ring(band, x + ro, y + ro, ro, ro - 2, col);
        ring(band, x + w - 1 - ro, y + h - 1 - ro, ro, ro - 2, col);
        line(band, (x + w - 3, y + 1), (x + 2, y + h - 2), 1, col);
    } else {
        text(band, F_LABEL, b"%", x1, y + s.h - 13, col);
    }
}

/// `"NN%"` into `buf`.
fn percent_text(v: u8, buf: &mut [u8; 4]) -> &[u8] {
    let v = v.min(100);
    let n = if v >= 100 {
        buf[..3].copy_from_slice(b"100");
        3
    } else if v >= 10 {
        buf[0] = b'0' + v / 10;
        buf[1] = b'0' + v % 10;
        2
    } else {
        buf[0] = b'0' + v;
        1
    };
    buf[n] = b'%';
    &buf[..=n]
}

// ---------------------------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------------------------

/// A speaker with a slash through it, 24x24 at `(x, y)`; `halo` is what the slash cuts with.
fn speaker_muted(band: &mut TileBand, x: i32, y: i32, col: Rgb565, halo: Rgb565) {
    rrect(band, x + 2, y + 8, 6, 8, 1, col);
    tri(band, (x + 7, y + 8), (x + 16, y + 2), (x + 16, y + 22), col);
    rect(band, x + 7, y + 8, 3, 8, col);
    line(band, (x + 3, y + 3), (x + 21, y + 21), 3, halo);
    line(band, (x + 3, y + 3), (x + 21, y + 21), 1, col);
}

/// An eighth note, 24x24 at `(x, y)`.
fn note(band: &mut TileBand, x: i32, y: i32, col: Rgb565) {
    disc(band, x + 8, y + 18, 4, col);
    rect(band, x + 11, y + 3, 2, 16, col);
    line(band, (x + 12, y + 3), (x + 19, y + 9), 1, col);
}

// ---------------------------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------------------------

const RECOVERY_RING: Arc = Arc {
    cx: CENTER,
    cy: 100,
    ro: 62,
    ri: 52,
    start: 0,
    sweep: TURN,
};

/// The recovery-hold takeover, owning the whole screen. `percent` is time-based hold progress,
/// 0..=100, shown as an amber ring around a restart glyph. Draws every pixel of the band.
pub fn render_recovery(band: &mut TileBand, percent: u8) {
    band.fill(BG);
    let percent = percent.min(100);
    let a = &RECOVERY_RING;
    // The track is always drawn, so 0% still reads as "a ring that will fill".
    arc(band, a, 0, a.sweep, 0, AMBER_DIM);
    let end = a.sweep * i32::from(percent) / 100;
    if end > 0 {
        arc(band, a, 0, end, 0, AMBER);
        if end < a.sweep {
            cap(band, a, 0, 0, AMBER);
            cap(band, a, end, 0, AMBER);
        }
    }
    restart_glyph(band, a.cx, a.cy, TEXT);
    if percent >= 100 {
        text_c(band, F_TITLE, b"Restarting", CENTER, 182, TEXT);
    } else {
        text_c(band, F_TITLE, b"Keep holding", CENTER, 176, TEXT);
        text_c(band, F_LABEL, b"to restart Kivori", CENTER, 202, MUTED);
    }
}

/// A clockwise circular arrow centred on `(cx, cy)`.
fn restart_glyph(band: &mut TileBand, cx: i32, cy: i32, col: Rgb565) {
    let a = Arc {
        cx,
        cy,
        ro: 24,
        ri: 18,
        start: TURN / 12,
        sweep: TURN * 3 / 4,
    };
    arc(band, &a, 0, a.sweep, 0, col);
    cap(band, &a, 0, 0, col);
    // Arrowhead at the end, pointing along the clockwise tangent.
    let t = a.start + a.sweep;
    let (px, py) = a.at(a.sweep);
    let (s, c) = (sin_q14(t), sin_q14(t + TURN / 4));
    let along = |d: i32| ((c * d) >> 14, (s * d) >> 14);
    let out = |d: i32| ((s * d) >> 14, -(c * d) >> 14);
    let (tx, ty) = along(10);
    let (nx, ny) = out(10);
    tri(
        band,
        (px + tx, py + ty),
        (px + nx, py + ny),
        (px - nx, py - ny),
        col,
    );
}

// ---------------------------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------------------------

/// The full-screen view for the current [`DisplayMode`] (see [`render_view`]).
pub fn render_mode(band: &mut TileBand, view: &DeskView, overlay: Option<ValueDisplay>) {
    render_view(band, view.status.mode, view, overlay);
}

/// The full-screen view for `mode`, drawn from `view`'s data. Draws every pixel of the band.
/// `overlay` is the live volume value, which the Volume view prefers over
/// `view.status.volume_percent` while it is in force. `Buddy` belongs to the mascot; here it is
/// the bare background.
pub fn render_view(
    band: &mut TileBand,
    mode: DisplayMode,
    view: &DeskView,
    overlay: Option<ValueDisplay>,
) {
    band.fill(BG);
    let status = &view.status;
    match mode {
        DisplayMode::Buddy => {}
        DisplayMode::Clock => clock_view(band, status),
        DisplayMode::Volume => volume_view(band, status, overlay),
        DisplayMode::Media => media_view(band, status.media, view.media_info, view.elapsed_ms),
        DisplayMode::System => system_view(band, status, &view.cpu_history),
    }
}

/// While a view switch animates: the outgoing mode and how many rows it has scrolled up
/// (`0..=240`, ease-out cubic over [`VIEW_TRANSITION_MS`]).
#[must_use]
pub fn view_switch(view: &DeskView) -> Option<(DisplayMode, i32)> {
    let previous = view.previous_mode.filter(|p| *p != view.status.mode)?;
    if view.mode_age_ms >= VIEW_TRANSITION_MS {
        return None;
    }
    let t = i64::from(view.mode_age_ms) * 4096 / i64::from(VIEW_TRANSITION_MS);
    let u = 4096 - t;
    let eased = 4096 - u * u / 4096 * u / 4096;
    Some((previous, (i64::from(SCREEN) * eased / 4096) as i32))
}

/// The view layer of a frame: the current mode, or during a switch the outgoing view sliding up
/// and out with the new one pushing in from below. `draw` paints one mode full-screen in
/// frame-absolute coordinates; during a switch it gets row slices of the band that are scrolled
/// (see [`TileBand::scrolled`]), so it needs no knowledge of the animation and the mascot can be
/// one of the two views.
///
/// # Errors
/// Whatever `draw` returns.
pub fn render_views<E>(
    band: &mut TileBand,
    view: &DeskView,
    mut draw: impl FnMut(&mut TileBand, DisplayMode) -> Result<(), E>,
) -> Result<(), E> {
    let Some((previous, shift)) = view_switch(view) else {
        return draw(band, view.status.mode);
    };
    // Rows above the seam still show the outgoing view.
    let seam = SCREEN - shift;
    let (mut above, mut below) = band.split_at_row(seam as u16);
    if above.rect().h > 0 {
        if let Some(mut b) = above.scrolled(shift) {
            draw(&mut b, previous)?;
        }
    }
    if below.rect().h > 0 {
        if let Some(mut b) = below.scrolled(-seam) {
            draw(&mut b, view.status.mode)?;
        }
    }
    Ok(())
}

// --- Clock -----------------------------------------------------------------------------------

fn clock_view(band: &mut TileBand, status: &DeskStatus) {
    let clock = status.clock;
    // Seconds ring: 60 ticks, the elapsed ones lit, the current one bright.
    let second = clock.map(|t| i32::from(t.second.min(59)));
    for i in 0..60 {
        let t = i * TURN / 60;
        let major = i % 5 == 0;
        let now = second == Some(i);
        let r0 = if major || now { 101 } else { 105 };
        let col = match second {
            _ if now => TEXT,
            Some(s) if i < s => ACCENT,
            _ if major => LINE,
            _ => TRACK,
        };
        line(
            band,
            polar(CENTER, CENTER, r0, t),
            polar(CENTER, CENTER, 112, t),
            if now { 2 } else { 1 },
            col,
        );
    }

    // HH:MM: two digit pairs around a colon cell.
    const COLON: i32 = 22;
    let w = 2 * digits_width(2, &BIG) + COLON;
    let (x0, y0) = (CENTER - w / 2, 80);
    let (ds, col) = match clock {
        Some(t) => {
            let (h, m) = (t.hour.min(23), t.minute.min(59));
            (
                [Some(h / 10), Some(h % 10), Some(m / 10), Some(m % 10)],
                TEXT,
            )
        }
        None => ([None; 4], MUTED),
    };
    let x1 = digits(band, x0, y0, &ds[..2], &BIG, col);
    digits(band, x1 + COLON, y0, &ds[2..], &BIG, col);
    let cx = x1 + COLON / 2;
    disc(band, cx, y0 + 19, 4, col);
    disc(band, cx, y0 + BIG.h - 20, 4, col);

    // Secondary info row: only what is known.
    let mut vol = [0u8; 4];
    let mut cpu = [0u8; 4];
    let mut chips: [Option<(&[u8], &[u8])>; 2] = [None, None];
    if status.muted == Some(true) {
        chips[0] = Some((b"VOL", b"MUTED"));
    } else if let Some(v) = status.volume_percent {
        chips[0] = Some((b"VOL", percent_text(v, &mut vol)));
    }
    if let Some(c) = status.cpu_percent {
        chips[1] = Some((b"CPU", percent_text(c, &mut cpu)));
    }
    chip_row(band, &chips, 164);
}

const CHIP_H: i32 = 22;

fn chip_width(label: &[u8], value: &[u8]) -> i32 {
    text_width(F_LABEL, label) + 6 + text_width(F_LABEL, value) + 20
}

/// Rounded chips ("LABEL value"), centred as a row at `y`.
fn chip_row(band: &mut TileBand, chips: &[Option<(&[u8], &[u8])>], y: i32) {
    const GAP: i32 = 8;
    let (mut w, mut n) = (0, 0);
    for (label, value) in chips.iter().flatten() {
        w += chip_width(label, value);
        n += 1;
    }
    if n == 0 {
        return;
    }
    let mut x = CENTER - (w + (n - 1) * GAP) / 2;
    for (label, value) in chips.iter().flatten() {
        let cw = chip_width(label, value);
        rrect(band, x, y, cw, CHIP_H, CHIP_H / 2, CARD);
        text(band, F_LABEL, label, x + 10, y + 5, MUTED);
        text(
            band,
            F_LABEL,
            value,
            x + 10 + text_width(F_LABEL, label) + 6,
            y + 5,
            TEXT,
        );
        x += cw + GAP;
    }
}

// --- Volume ----------------------------------------------------------------------------------

const VOLUME_GAUGE: Arc = Arc::gauge(CENTER, 128, 96, 82);

fn volume_view(band: &mut TileBand, status: &DeskStatus, overlay: Option<ValueDisplay>) {
    let (pct, confidence, at_boundary) = match overlay {
        Some(o) => (
            Some(o.current_percent.min(100)),
            o.confidence,
            o.at_boundary,
        ),
        None => (
            status.volume_percent.map(|p| p.min(100)),
            ValueConfidence::Confirmed,
            matches!(status.volume_percent, Some(0 | 100)),
        ),
    };
    let muted = status.muted == Some(true);
    let a = &VOLUME_GAUGE;
    let num_y = 94;
    match pct {
        Some(p) => {
            // The limit gets one subtle reaction: a brighter track.
            let track = if at_boundary { LINE } else { TRACK };
            let fill = if muted { FAINT } else { ACCENT };
            gauge(band, a, p, confidence, fill, track);
            let col = if muted {
                FAINT
            } else if confidence == ValueConfidence::Unverified {
                MUTED
            } else {
                TEXT
            };
            percent(band, Some(p), CENTER, num_y, &MID, col);
        }
        None => {
            unknown_gauge(band, a);
            percent(band, None, CENTER, num_y, &MID, MUTED);
        }
    }
    // Scale ends.
    let (sx, sy) = a.at(0);
    let (ex, ey) = a.at(a.sweep);
    text_c(band, F_SMALL, b"0", sx, sy + 12, FAINT);
    text_c(band, F_SMALL, b"100", ex, ey + 12, FAINT);

    // Unknown mute shows the plain caption: never claim unmuted.
    if muted {
        let w = 18 + 6 + text_width(F_LABEL, b"MUTED") + 20;
        let x = CENTER - w / 2;
        rrect(band, x, 154, w, CHIP_H, CHIP_H / 2, CARD);
        speaker_muted(band, x + 8, 154 + 2, TEXT, CARD);
        text(band, F_LABEL, b"MUTED", x + 10 + 18 + 6, 154 + 5, TEXT);
    } else {
        text_c(band, F_LABEL, b"VOLUME", CENTER, 158, MUTED);
    }
}

// --- Media -----------------------------------------------------------------------------------

const MEDIA_TEXT: (i32, i32) = (16, SCREEN - 16);

fn media_view(
    band: &mut TileBand,
    media: Option<MediaStatus>,
    info: Option<MediaInfo>,
    elapsed_ms: u32,
) {
    // Artwork tile with the transport state.
    let (cx, cy) = (CENTER, 74);
    rrect(band, cx - 34, cy - 34, 68, 68, 18, CARD);
    let label: Option<(&[u8], Rgb565)> = match media {
        Some(MediaStatus::Playing) => {
            tri(
                band,
                (cx - 9, cy - 15),
                (cx - 9, cy + 15),
                (cx + 16, cy),
                ACCENT,
            );
            Some((b"PLAYING", ACCENT))
        }
        Some(MediaStatus::Paused) => {
            rrect(band, cx - 12, cy - 14, 8, 28, 2, TEXT);
            rrect(band, cx + 4, cy - 14, 8, 28, 2, TEXT);
            Some((b"PAUSED", MUTED))
        }
        Some(MediaStatus::Stopped) => {
            rrect(band, cx - 12, cy - 12, 24, 24, 4, TEXT);
            Some((b"STOPPED", MUTED))
        }
        None => {
            for (p, q) in [
                ((cx - 9, cy - 9), (cx - 4, cy - 14)),
                ((cx - 4, cy - 14), (cx + 4, cy - 14)),
                ((cx + 4, cy - 14), (cx + 9, cy - 9)),
                ((cx + 9, cy - 9), (cx + 9, cy - 5)),
                ((cx + 9, cy - 5), (cx, cy + 2)),
                ((cx, cy + 2), (cx, cy + 5)),
            ] {
                line(band, p, q, 2, MUTED);
            }
            disc(band, cx, cy + 13, 3, MUTED);
            None
        }
    };
    if let Some((s, col)) = label {
        text_c(band, F_LABEL, s, CENTER, 116, col);
    }

    // Title and artist, as the OS reports them. No info is said, not left blank.
    match info {
        Some(info) => {
            let (title, title_col): (&[u8], _) = if info.title.is_empty() {
                (b"Unknown title", MUTED)
            } else {
                (info.title.as_latin1(), TEXT)
            };
            marquee(band, F_TITLE, title, MEDIA_TEXT, 140, title_col, elapsed_ms);
            if !info.artist.is_empty() {
                let artist = info.artist.as_latin1();
                marquee(band, F_BODY, artist, MEDIA_TEXT, 166, MUTED, elapsed_ms);
            }
        }
        None => text_c(band, F_TITLE, b"No media info", CENTER, 146, MUTED),
    }

    if media == Some(MediaStatus::Playing) {
        equalizer(band, elapsed_ms);
    }
}

/// Decorative level bars while something plays (not a measurement: the desktop sends none).
/// Each bar is a triangle wave of its own period, so the motion never visibly loops.
fn equalizer(band: &mut TileBand, elapsed_ms: u32) {
    const N: i32 = 9;
    const W: i32 = 6;
    const GAP: i32 = 5;
    const BASE: i32 = 222;
    let x0 = CENTER - (N * W + (N - 1) * GAP) / 2;
    for i in 0..N {
        let period = 520 + 70 * i as u32 + (i as u32 % 3) * 110;
        let phase = (elapsed_ms + 997 * i as u32) % period;
        let half = period / 2;
        let tri = if phase < half { phase } else { period - phase };
        let h = 5 + (tri * 21 / half) as i32;
        rrect(band, x0 + i * (W + GAP), BASE - h, W, h, W / 2, ACCENT);
    }
}

// --- System ----------------------------------------------------------------------------------

const CPU_GAUGE: Arc = Arc::gauge(64, 92, 44, 36);
const RAM_GAUGE: Arc = Arc::gauge(176, 92, 44, 36);

fn system_view(band: &mut TileBand, status: &DeskStatus, history: &CpuHistory) {
    let hot = status.high_load;
    let gauges: [(&Arc, Option<u8>, &[u8], bool); 2] = [
        (&CPU_GAUGE, status.cpu_percent, b"CPU", hot),
        (&RAM_GAUGE, status.ram_percent, b"RAM", false),
    ];
    for (a, value, name, hot) in gauges {
        let fill = if hot { AMBER } else { ACCENT };
        match value {
            Some(v) => {
                gauge(band, a, v, ValueConfidence::Confirmed, fill, TRACK);
                percent(band, Some(v), a.cx, a.cy - 13, &SMALL, TEXT);
            }
            None => {
                unknown_gauge(band, a);
                percent(band, None, a.cx, a.cy - 13, &SMALL, MUTED);
            }
        }
        text_c(
            band,
            F_LABEL,
            name,
            a.cx,
            a.cy + 26,
            if hot { AMBER } else { MUTED },
        );
    }
    sparkline(band, history.as_slice(), hot);
}

/// The CPU history card: the last minute as an area chart, newest at the right.
fn sparkline(band: &mut TileBand, samples: &[u8], hot: bool) {
    const X0: i32 = 16;
    const Y0: i32 = 146;
    const GX0: i32 = 28;
    const GX1: i32 = 212;
    const GY0: i32 = 170;
    /// The baseline row.
    const GY1: i32 = 213;
    rrect(band, X0, Y0, SCREEN - 2 * X0, 80, 14, CARD);
    text(band, F_SMALL, b"CPU", GX0, Y0 + 9, MUTED);
    text(
        band,
        F_SMALL,
        b"1 MIN",
        GX1 - text_width(F_SMALL, b"1 MIN"),
        Y0 + 9,
        FAINT,
    );
    // A dotted 50% gridline and the baseline.
    let mid = (GY0 + GY1) / 2;
    shape(band, (GX0, mid, GX1, mid + 1), LINE, |x, _| {
        if (x - GX0) % 4 == 0 {
            FULL
        } else {
            0
        }
    });
    rect(band, GX0, GY1, GX1 - GX0, 1, LINE);
    if samples.is_empty() {
        text_c(band, F_SMALL, b"No samples yet", CENTER, mid - 12, FAINT);
        return;
    }
    let (stroke, fill) = if hot {
        (AMBER, AMBER_DIM)
    } else {
        (ACCENT, ACCENT_DIM)
    };
    let slots = CpuHistory::CAPACITY as i32;
    let first = slots - samples.len() as i32;
    let span = GX1 - GX0 - 1;
    // The chart height of column `x` in 1/256 px above the baseline, or None before the data.
    let level = |x: i32| -> Option<i32> {
        let pos = (x - GX0) * (slots - 1) * 256 / span;
        let slot = pos / 256;
        if slot < first {
            return None;
        }
        let i = (slot - first) as usize;
        let a = i32::from(samples[i].min(100));
        let b = i32::from(samples.get(i + 1).copied().unwrap_or(samples[i]).min(100));
        let v = a * 256 + (b - a) * (pos % 256);
        Some(v * (GY1 - GY0) / 100)
    };
    paint(band, (GX0, GY0 - 2, GX1, GY1), |x, y| {
        let here = level(x)?;
        let top = GY1 - (here + 128) / 256;
        // Join to the previous column so steep changes stay one continuous stroke.
        let prev = level(x - 1).map_or(top, |p| GY1 - (p + 128) / 256);
        let (lo, hi) = (top.min(prev) - 1, top.max(prev) + 1);
        if (lo..=hi).contains(&y) {
            Some((stroke, FULL))
        } else if y > top {
            Some((fill, FULL))
        } else {
            None
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Chrome
// ---------------------------------------------------------------------------------------------

/// The status row, the high-load cue, the local press acknowledgement and the action feedback
/// badge, drawn over whatever view is below (and fixed while views slide). Touches only the
/// pixels it draws.
///
/// The status row is the badge on the left, the time in the centre (every view except Clock,
/// which is the time) and the indicators on the right: master mute, then media.
pub fn render_chrome(band: &mut TileBand, view: &DeskView) {
    let status = &view.status;

    if let (Some(t), true) = (status.clock, status.mode != DisplayMode::Clock) {
        let (h, m) = (t.hour.min(23), t.minute.min(59));
        let s = [
            b'0' + h / 10,
            b'0' + h % 10,
            b':',
            b'0' + m / 10,
            b'0' + m % 10,
        ];
        text_c(band, F_STATUS, &s, CENTER, 10, MUTED);
    }

    // Indicators, right-aligned; master mute has priority (rightmost).
    let mut slot_x = SCREEN - 8 - 24;
    if status.muted == Some(true) {
        speaker_muted(band, slot_x, 8, TEXT, BG);
        slot_x -= 24 + 6;
    }
    if status.media == Some(MediaStatus::Playing) {
        // Gentle bob: 0..2 px, triangle wave with a 1200 ms period.
        let phase = (view.elapsed_ms % 1200) as i32;
        let tri_wave = if phase < 600 { phase } else { 1200 - phase };
        let bob = (tri_wave * 2 + 300) / 600;
        note(band, slot_x, 8 + bob, TEXT);
    }

    // High-load cue: heat rising above the keycap. Buddy only; a cue, not a face.
    if status.mode == DisplayMode::Buddy && status.high_load {
        let flip = if (view.elapsed_ms / 400).is_multiple_of(2) {
            1
        } else {
            -1
        };
        for (i, x) in [104, 120, 136].into_iter().enumerate() {
            let s = if i % 2 == 0 { flip } else { -flip };
            let pts = [(x, 54), (x + 3 * s, 48), (x, 42), (x + 3 * s, 36)];
            for pair in pts.windows(2) {
                line(band, pair[0], pair[1], 1, AMBER);
            }
        }
    }

    if let (DisplayMode::Buddy, Some(labels)) = (status.mode, view.controls) {
        control_legend(band, &labels);
    }

    if let Some(fb) = view.feedback {
        badge(band, fb.kind, view.elapsed_ms);
    }

    if view.button_down {
        let n = SCREEN;
        rect(band, 0, 0, n, 3, TEXT);
        rect(band, 0, n - 3, n, 3, TEXT);
        rect(band, 0, 0, 3, n, TEXT);
        rect(band, n - 3, 0, 3, n, TEXT);
    }
}

/// What each control does, under the keycap of the Buddy view: three 80 px columns of a small
/// caption (the control) over its label (the bound action, as the desktop named it). A control
/// with an empty label is left out, never guessed. A label too wide for its column fades out at
/// the column edge.
fn control_legend(band: &mut TileBand, labels: &ControlLabels) {
    const COL_W: i32 = SCREEN / 3;
    const PAD: i32 = 4;
    const FADE: i32 = 10;
    let columns: [(&[u8], &MediaText); 3] = [
        (b"TURN", &labels.rotate),
        (b"PRESS", &labels.press),
        (b"HOLD", &labels.hold),
    ];
    for (i, (caption, label)) in columns.into_iter().enumerate() {
        let label = label.as_latin1();
        if label.is_empty() {
            continue;
        }
        let x0 = i as i32 * COL_W;
        let cx = x0 + COL_W / 2;
        text_c(band, F_SMALL, caption, cx, 210, FAINT);
        let (x1, x2) = (x0 + PAD, x0 + COL_W - PAD);
        let w = text_width(F_LABEL, label);
        let x = if w <= x2 - x1 { cx - w / 2 } else { x1 };
        // The left fade zone sits before the text, so only the cut (right) end fades.
        text_clipped(band, F_LABEL, label, (x, 222), TEXT, (x1 - FADE, x2, FADE));
    }
}

/// The action feedback badge: a 36 px circle at the top left, cut out of whatever is beneath by
/// a thin background halo. Every kind differs in shape and colour; `Unverified` is a bare amber
/// ring with a question mark and never a check or green.
fn badge(band: &mut TileBand, kind: FeedbackKind, elapsed_ms: u32) {
    let (cx, cy) = (30, 28);
    ring(band, cx, cy, 21, 18, BG);
    match kind {
        FeedbackKind::StateConfirmed => {
            disc(band, cx, cy, 18, GREEN);
            line(band, (cx - 8, cy + 1), (cx - 3, cy + 6), 2, TEXT);
            line(band, (cx - 3, cy + 6), (cx + 8, cy - 6), 2, TEXT);
        }
        FeedbackKind::ExecutionConfirmed => {
            disc(band, cx, cy, 18, BLUE);
            tri(
                band,
                (cx - 5, cy - 9),
                (cx - 5, cy + 9),
                (cx + 10, cy),
                TEXT,
            );
        }
        FeedbackKind::Unverified => {
            ring(band, cx, cy, 18, 15, AMBER);
            text_c(band, F_TITLE, b"?", cx + 1, cy - 10, AMBER);
        }
        FeedbackKind::Error => {
            disc(band, cx, cy, 18, RED);
            line(band, (cx - 7, cy - 7), (cx + 7, cy + 7), 2, TEXT);
            line(band, (cx + 7, cy - 7), (cx - 7, cy + 7), 2, TEXT);
        }
        FeedbackKind::Processing => {
            // Indeterminate: a faint ring with eight dots and a bright head circling it.
            let head = ((elapsed_ms / 100) % 8) as i32;
            ring(band, cx, cy, 17, 15, TRACK);
            for i in 0..8 {
                let col = match (head + 8 - i) % 8 {
                    0 => TEXT,
                    1 => rgb(0xc8, 0xce, 0xda),
                    2 => MUTED,
                    3 => rgb(0x5a, 0x62, 0x76),
                    _ => rgb(0x2c, 0x34, 0x46),
                };
                let (x, y) = polar(cx, cy, 16, i * TURN / 8);
                disc(band, x, y, 3, col);
            }
        }
    }
}
