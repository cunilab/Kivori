//! M1 desk rendering: the non-Buddy views, the indicator/feedback chrome drawn over any view, and
//! the recovery-hold takeover.
//!
//! Same rules as the rest of the renderer: integer math only, a pure function of its inputs, and
//! frame-absolute coordinates clipped by [`TileBand::set`], so a frame stitched from tiles equals
//! the full-frame render on the host and on the device. Shapes are per-pixel integer predicates
//! over a clipped bounding box; text uses the `embedded-graphics` ASCII mono fonts.
//!
//! Unknown (`None`) values are always drawn as an explicit unknown (`--`, `--:--`, a dashed empty
//! bar, "No media info"), never as zero or a guess.

use embedded_graphics::mono_font::{ascii, MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::raw::RawU16;
use embedded_graphics::pixelcolor::Rgb565 as EgRgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use kivori_framebuffer::TileBand;
use kivori_model::desk::{ClockTime, DeskStatus, DeskView, DisplayMode, FeedbackKind, MediaStatus};
use kivori_model::presentation::{ValueConfidence, ValueDisplay};
use kivori_model::Rgb565;

use crate::overlay::render_volume_overlay;

const fn rgb(r: u8, g: u8, b: u8) -> Rgb565 {
    Rgb565::from_rgb888(r, g, b)
}

/// Background, matching the mascot scenes.
const BG: Rgb565 = rgb(0x0c, 0x10, 0x1c);
const WHITE: Rgb565 = Rgb565::WHITE;
const GRAY: Rgb565 = rgb(0x8a, 0x94, 0xa8);
/// A dimmed foreground, for a value that is known but muted.
const DIM: Rgb565 = rgb(0x5c, 0x66, 0x80);
/// Bar outline; the same gray as the volume overlay track.
const TRACK: Rgb565 = rgb(96, 96, 96);
const AMBER: Rgb565 = rgb(0xf5, 0xa5, 0x24);
const GREEN: Rgb565 = rgb(0x3c, 0xc4, 0x7c);
const BLUE: Rgb565 = rgb(0x4c, 0x8d, 0xf6);
const RED: Rgb565 = rgb(0xe5, 0x48, 0x4d);

const FONT: &MonoFont<'static> = &ascii::FONT_10X20;
const CHAR_W: i32 = 10;

const SCREEN: i32 = 240;

// ---------------------------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------------------------

/// Calls `paint` for every `(x, y)` in `[x0, x1) x [y0, y1)` that lies inside the band and for
/// which `inside` holds. The loop is clipped to the band first, so tiles skip shapes they miss.
fn shape(
    band: &mut TileBand,
    (x0, y0, x1, y1): (i32, i32, i32, i32),
    col: Rgb565,
    inside: impl Fn(i32, i32) -> bool,
) {
    let r = band.rect();
    let x0 = x0.max(i32::from(r.x));
    let y0 = y0.max(i32::from(r.y));
    let x1 = x1.min(i32::from(r.x) + i32::from(r.w));
    let y1 = y1.min(i32::from(r.y) + i32::from(r.h));
    for y in y0..y1 {
        for x in x0..x1 {
            if inside(x, y) {
                band.set(x as u16, y as u16, col);
            }
        }
    }
}

fn rect(band: &mut TileBand, x: i32, y: i32, w: i32, h: i32, col: Rgb565) {
    shape(band, (x, y, x + w, y + h), col, |_, _| true);
}

/// A disc (`inner == 0`) or ring of pixels whose distance from `(cx, cy)` is in `inner..=outer`.
fn ring(band: &mut TileBand, cx: i32, cy: i32, outer: i32, inner: i32, col: Rgb565) {
    shape(
        band,
        (cx - outer, cy - outer, cx + outer + 1, cy + outer + 1),
        col,
        |x, y| {
            let d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
            d2 <= outer * outer && d2 >= inner * inner
        },
    );
}

fn disc(band: &mut TileBand, cx: i32, cy: i32, r: i32, col: Rgb565) {
    ring(band, cx, cy, r, 0, col);
}

/// A line segment with round caps; every pixel within `r` of the segment.
fn line(band: &mut TileBand, (x0, y0): (i32, i32), (x1, y1): (i32, i32), r: i32, col: Rgb565) {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len2 = i64::from(dx * dx + dy * dy);
    let r2 = i64::from(r * r);
    shape(
        band,
        (
            x0.min(x1) - r,
            y0.min(y1) - r,
            x0.max(x1) + r + 1,
            y0.max(y1) + r + 1,
        ),
        col,
        |x, y| {
            let (px, py) = (x - x0, y - y0);
            let dot = i64::from(px * dx + py * dy);
            let d2 = if dot <= 0 {
                i64::from(px * px + py * py)
            } else if dot >= len2 {
                i64::from((x - x1) * (x - x1) + (y - y1) * (y - y1))
            } else {
                let cross = i64::from(px * dy - py * dx);
                cross * cross / len2
            };
            d2 <= r2
        },
    );
}

/// A filled triangle.
fn tri(band: &mut TileBand, a: (i32, i32), b: (i32, i32), c: (i32, i32), col: Rgb565) {
    let edge = |p: (i32, i32), q: (i32, i32), x: i32, y: i32| {
        (q.0 - p.0) * (y - p.1) - (q.1 - p.1) * (x - p.0)
    };
    shape(
        band,
        (
            a.0.min(b.0).min(c.0),
            a.1.min(b.1).min(c.1),
            a.0.max(b.0).max(c.0) + 1,
            a.1.max(b.1).max(c.1) + 1,
        ),
        col,
        |x, y| {
            let (e0, e1, e2) = (edge(a, b, x, y), edge(b, c, x, y), edge(c, a, x, y));
            (e0 >= 0 && e1 >= 0 && e2 >= 0) || (e0 <= 0 && e1 <= 0 && e2 <= 0)
        },
    );
}

fn eg_color(c: Rgb565) -> EgRgb565 {
    EgRgb565::from(RawU16::new(c.raw()))
}

fn text(band: &mut TileBand, s: &str, x: i32, y: i32, col: Rgb565) {
    let style = MonoTextStyle::new(FONT, eg_color(col));
    // `TileBand`'s DrawTarget is infallible.
    let _ = Text::with_baseline(s, Point::new(x, y), style, Baseline::Top).draw(band);
}

fn text_width(s: &str) -> i32 {
    s.len() as i32 * CHAR_W
}

fn text_centered(band: &mut TileBand, s: &str, cx: i32, y: i32, col: Rgb565) {
    text(band, s, cx - text_width(s) / 2, y, col);
}

/// Decimal `v` (0..=100) into `buf`, returned as text.
fn num(v: u8, buf: &mut [u8; 4]) -> &str {
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
    core::str::from_utf8(&buf[..n]).unwrap_or("")
}

/// "NN%" into `buf`.
fn percent_text(v: u8, buf: &mut [u8; 4]) -> usize {
    let n = num(v, buf).len();
    buf[n] = b'%';
    n + 1
}

// ---------------------------------------------------------------------------------------------
// Seven-segment digits
// ---------------------------------------------------------------------------------------------

const DIGIT_W: i32 = 36;
const DIGIT_H: i32 = 64;
const SEG_T: i32 = 8;
/// Segments a b c d e f g as bits 0..=6, for digits 0..=9.
const SEGMENTS: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];

/// One digit at `(x, y)`; `None` draws only the middle bar (an explicit "unknown").
fn digit(band: &mut TileBand, x: i32, y: i32, d: Option<u8>, col: Rgb565) {
    let mask = d.map_or(0x40, |d| SEGMENTS[usize::from(d.min(9))]);
    let (w, h, t) = (DIGIT_W, DIGIT_H, SEG_T);
    let mid = (h - t) / 2;
    // Segments overlap at the corners, so a digit reads as one solid blocky glyph.
    let segs = [
        (x, y, w, t),                     // a
        (x + w - t, y, t, mid + t),       // b
        (x + w - t, y + mid, t, h - mid), // c
        (x, y + h - t, w, t),             // d
        (x, y + mid, t, h - mid),         // e
        (x, y, t, mid + t),               // f
        (x, y + mid, w, t),               // g
    ];
    for (i, (sx, sy, sw, sh)) in segs.into_iter().enumerate() {
        if mask & (1 << i) != 0 {
            rect(band, sx, sy, sw, sh, col);
        }
    }
}

/// Draws `digits` left to right with a 10 px gap, starting at `x`.
fn digits(band: &mut TileBand, mut x: i32, y: i32, ds: &[Option<u8>], col: Rgb565) {
    for d in ds {
        digit(band, x, y, *d, col);
        x += DIGIT_W + 10;
    }
}

// ---------------------------------------------------------------------------------------------
// Bars
// ---------------------------------------------------------------------------------------------

const BAR_X: i32 = 24;
const BAR_W: i32 = 192;
const BAR_H: i32 = 16;
const BAR_BORDER: i32 = 2;

/// A solid track with `pct`% filled in `fill`.
fn meter(band: &mut TileBand, y: i32, pct: u8, fill: Rgb565) {
    let pct = i32::from(pct.min(100));
    rect(band, BAR_X, y, BAR_W, BAR_BORDER, TRACK);
    rect(
        band,
        BAR_X,
        y + BAR_H - BAR_BORDER,
        BAR_W,
        BAR_BORDER,
        TRACK,
    );
    rect(band, BAR_X, y, BAR_BORDER, BAR_H, TRACK);
    rect(
        band,
        BAR_X + BAR_W - BAR_BORDER,
        y,
        BAR_BORDER,
        BAR_H,
        TRACK,
    );
    let inner_w = BAR_W - 2 * BAR_BORDER;
    rect(
        band,
        BAR_X + BAR_BORDER,
        y + BAR_BORDER,
        inner_w * pct / 100,
        BAR_H - 2 * BAR_BORDER,
        fill,
    );
}

/// An empty bar with a dashed outline: "the value is not known". A known 0% has a solid outline.
fn unknown_bar(band: &mut TileBand, y: i32) {
    shape(band, (BAR_X, y, BAR_X + BAR_W, y + BAR_H), GRAY, |x, yy| {
        let (lx, ly) = (x - BAR_X, yy - y);
        let horizontal_edge = !(BAR_BORDER..BAR_H - BAR_BORDER).contains(&ly);
        let vertical_edge = !(BAR_BORDER..BAR_W - BAR_BORDER).contains(&lx);
        (horizontal_edge && (lx / 6) % 2 == 0) || (vertical_edge && (ly / 4) % 2 == 0)
    });
}

// ---------------------------------------------------------------------------------------------
// Icons (24 px)
// ---------------------------------------------------------------------------------------------

/// A speaker with a slash through it, 24x24 at `(x, y)`.
fn speaker_muted(band: &mut TileBand, x: i32, y: i32, col: Rgb565) {
    rect(band, x + 2, y + 8, 6, 8, col);
    tri(band, (x + 8, y + 8), (x + 16, y + 2), (x + 16, y + 22), col);
    tri(band, (x + 8, y + 8), (x + 16, y + 22), (x + 8, y + 16), col);
    // Background halo so the slash reads where it crosses the speaker.
    line(band, (x + 2, y + 2), (x + 22, y + 22), 3, BG);
    line(band, (x + 2, y + 2), (x + 22, y + 22), 1, col);
}

/// An eighth note, 24x24 at `(x, y)`.
fn note(band: &mut TileBand, x: i32, y: i32, col: Rgb565) {
    disc(band, x + 8, y + 18, 4, col);
    rect(band, x + 11, y + 3, 3, 16, col);
    line(band, (x + 13, y + 3), (x + 20, y + 10), 1, col);
}

// ---------------------------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------------------------

/// The recovery-hold takeover, owning the whole screen. `percent` is time-based hold progress,
/// 0..=100. Draws every pixel of the band.
pub fn render_recovery(band: &mut TileBand, percent: u8) {
    band.fill(BG);
    let percent = percent.min(100);
    if percent >= 100 {
        text_centered(band, "Restarting", SCREEN / 2, 90, WHITE);
    } else {
        text_centered(band, "Keep holding", SCREEN / 2, 78, WHITE);
        text_centered(band, "to restart", SCREEN / 2, 102, WHITE);
    }
    // Amber progress bar, x 24..216, y 150..166; the outline is always drawn.
    let (x, y, w, h) = (24, 150, 192, 16);
    rect(band, x, y, w, 2, AMBER);
    rect(band, x, y + h - 2, w, 2, AMBER);
    rect(band, x, y, 2, h, AMBER);
    rect(band, x + w - 2, y, 2, h, AMBER);
    rect(
        band,
        x + 2,
        y + 2,
        (w - 4) * i32::from(percent) / 100,
        h - 4,
        AMBER,
    );
}

// ---------------------------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------------------------

/// The full-screen view for a non-Buddy [`DisplayMode`]. Draws every pixel of the band.
/// `overlay` is the live volume value, which the Volume view prefers over
/// `view.status.volume_percent` while it is in force.
pub fn render_mode(band: &mut TileBand, view: &DeskView, overlay: Option<ValueDisplay>) {
    band.fill(BG);
    let status = &view.status;
    match status.mode {
        // Buddy is the mascot's; if it is ever passed here the background is all there is.
        DisplayMode::Buddy => {}
        DisplayMode::Clock => clock_view(band, status.clock),
        DisplayMode::Volume => volume_view(band, status, overlay),
        DisplayMode::Media => media_view(band, status.media),
        DisplayMode::System => system_view(band, status),
    }
}

fn clock_view(band: &mut TileBand, clock: Option<ClockTime>) {
    // Two digit pairs (36 + 10 + 36 each) around a 24 px colon cell.
    const TOTAL: i32 = 82 + 24 + 82;
    let x0 = (SCREEN - TOTAL) / 2;
    let y0 = 88;
    let (ds, colon, col) = match clock {
        Some(t) => {
            let (h, m) = (t.hour.min(23), t.minute.min(59));
            (
                [Some(h / 10), Some(h % 10), Some(m / 10), Some(m % 10)],
                // Deterministic blink: visible on even seconds only.
                t.second % 2 == 0,
                WHITE,
            )
        }
        None => ([None; 4], true, GRAY),
    };
    digits(band, x0, y0, &ds[..2], col);
    let cx = x0 + 82 + 12;
    digits(band, x0 + 82 + 24, y0, &ds[2..], col);
    if colon {
        rect(band, cx - 4, y0 + 18, 8, 8, col);
        rect(band, cx - 4, y0 + 38, 8, 8, col);
    }
}

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
    let col = if muted {
        DIM
    } else if confidence == ValueConfidence::Confirmed {
        WHITE
    } else {
        GRAY
    };

    let y0 = 66;
    match pct {
        Some(p) => {
            let mut buf = [0u8; 4];
            let s = num(p, &mut buf);
            let mut ds = [None; 3];
            for (slot, ch) in ds.iter_mut().zip(s.bytes()) {
                *slot = Some(ch - b'0');
            }
            let n = s.len() as i32;
            // Digits, a 12 px gap, then the 28 px percent sign.
            let w = n * DIGIT_W + (n - 1) * 10 + 12 + 28;
            let x0 = (SCREEN - w) / 2;
            digits(band, x0, y0, &ds[..n as usize], col);
            percent_sign(band, x0 + w - 28, y0 + DIGIT_H - 40, col);
            render_volume_overlay(band, p, confidence, at_boundary);
        }
        None => {
            let w = 2 * DIGIT_W + 10;
            digits(band, (SCREEN - w) / 2, y0, &[None, None], GRAY);
            unknown_bar(band, VOLUME_BAR_Y);
        }
    }

    // Unknown mute shows nothing: never claim unmuted.
    if muted {
        let w = 24 + 10 + text_width("MUTED");
        let x = (SCREEN - w) / 2;
        speaker_muted(band, x, 146, WHITE);
        text(band, "MUTED", x + 24 + 10, 148, WHITE);
    }
}

/// The known-volume bar is the shared overlay's own (24..216 x 180..196), so the view and the
/// transient overlay look identical for the same confidence.
const VOLUME_BAR_Y: i32 = 180;

/// A 28x40 percent sign at `(x, y)`.
fn percent_sign(band: &mut TileBand, x: i32, y: i32, col: Rgb565) {
    ring(band, x + 7, y + 8, 7, 4, col);
    ring(band, x + 21, y + 32, 7, 4, col);
    line(band, (x + 24, y + 3), (x + 4, y + 37), 2, col);
}

fn media_view(band: &mut TileBand, media: Option<MediaStatus>) {
    let (cx, cy) = (120, 96);
    let label = match media {
        Some(MediaStatus::Playing) => {
            tri(
                band,
                (cx - 24, cy - 40),
                (cx - 24, cy + 40),
                (cx + 40, cy),
                WHITE,
            );
            "Playing"
        }
        Some(MediaStatus::Paused) => {
            rect(band, cx - 30, cy - 40, 22, 80, WHITE);
            rect(band, cx + 8, cy - 40, 22, 80, WHITE);
            "Paused"
        }
        Some(MediaStatus::Stopped) => {
            rect(band, cx - 36, cy - 36, 72, 72, WHITE);
            "Stopped"
        }
        None => {
            // A question mark: unknown, not stopped.
            for (a, b) in [
                ((cx - 24, cy - 24), (cx - 12, cy - 36)),
                ((cx - 12, cy - 36), (cx + 12, cy - 36)),
                ((cx + 12, cy - 36), (cx + 24, cy - 24)),
                ((cx + 24, cy - 24), (cx + 24, cy - 12)),
                ((cx + 24, cy - 12), (cx, cy + 8)),
                ((cx, cy + 8), (cx, cy + 20)),
            ] {
                line(band, a, b, 5, GRAY);
            }
            rect(band, cx - 8, cy + 32, 16, 14, GRAY);
            text_centered(band, "No media info", SCREEN / 2, 164, GRAY);
            return;
        }
    };
    text_centered(band, label, SCREEN / 2, 164, WHITE);
}

fn system_view(band: &mut TileBand, status: &DeskStatus) {
    let rows = [
        ("CPU", status.cpu_percent, 66, status.high_load),
        ("RAM", status.ram_percent, 130, false),
    ];
    for (name, value, y, hot) in rows {
        text(band, name, BAR_X, y, WHITE);
        match value {
            Some(v) => {
                let v = v.min(100);
                let mut buf = [0u8; 4];
                let n = percent_text(v, &mut buf);
                let s = core::str::from_utf8(&buf[..n]).unwrap_or("");
                text(band, s, BAR_X + BAR_W - text_width(s), y, WHITE);
                meter(
                    band,
                    y + 28,
                    v,
                    if hot { AMBER } else { rgb(0xd0, 0xd6, 0xe2) },
                );
            }
            None => {
                text(band, "--", BAR_X + BAR_W - text_width("--"), y, GRAY);
                unknown_bar(band, y + 28);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Chrome
// ---------------------------------------------------------------------------------------------

/// Indicators, the high-load cue, the local press acknowledgement and the action feedback badge,
/// drawn over whatever view is below. Touches only the pixels it draws.
pub fn render_chrome(band: &mut TileBand, view: &DeskView) {
    let status = &view.status;

    // Indicator strip, right-aligned; master mute has priority (rightmost).
    let mut slot_x = SCREEN - 8 - 24;
    if status.muted == Some(true) {
        speaker_muted(band, slot_x, 8, WHITE);
        slot_x -= 24 + 8;
    }
    if status.media == Some(MediaStatus::Playing) {
        // Gentle bob: 0..2 px, triangle wave with a 1200 ms period.
        let phase = (view.elapsed_ms % 1200) as i32;
        let tri_wave = if phase < 600 { phase } else { 1200 - phase };
        let bob = (tri_wave * 2 + 300) / 600;
        note(band, slot_x, 8 + bob, WHITE);
    }

    // High-load cue: heat waves above the keycap. Buddy only; a cue, not a face.
    if status.mode == DisplayMode::Buddy && status.high_load {
        for x in [104, 120, 136] {
            let pts = [(x, 52), (x + 4, 46), (x, 40), (x + 4, 34)];
            for pair in pts.windows(2) {
                line(band, pair[0], pair[1], 1, AMBER);
            }
        }
    }

    if let Some(fb) = view.feedback {
        badge(band, fb.kind, view.elapsed_ms);
    }

    if view.button_down {
        let n = SCREEN;
        rect(band, 0, 0, n, 3, WHITE);
        rect(band, 0, n - 3, n, 3, WHITE);
        rect(band, 0, 0, 3, n, WHITE);
        rect(band, n - 3, 0, 3, n, WHITE);
    }
}

/// The action feedback badge: a ~36 px circle at the top left. Every kind differs in shape and
/// colour; `Unverified` is a bare amber ring with a question mark and never a check or green.
fn badge(band: &mut TileBand, kind: FeedbackKind, elapsed_ms: u32) {
    let (cx, cy) = (30, 28);
    match kind {
        FeedbackKind::StateConfirmed => {
            disc(band, cx, cy, 18, GREEN);
            line(band, (cx - 9, cy + 1), (cx - 3, cy + 7), 2, WHITE);
            line(band, (cx - 3, cy + 7), (cx + 9, cy - 6), 2, WHITE);
        }
        FeedbackKind::ExecutionConfirmed => {
            disc(band, cx, cy, 18, BLUE);
            tri(
                band,
                (cx - 5, cy - 9),
                (cx - 5, cy + 9),
                (cx + 11, cy),
                WHITE,
            );
        }
        FeedbackKind::Unverified => {
            ring(band, cx, cy, 18, 14, AMBER);
            text(band, "?", cx - 5, cy - 10, AMBER);
        }
        FeedbackKind::Error => {
            disc(band, cx, cy, 18, RED);
            line(band, (cx - 8, cy - 8), (cx + 8, cy + 8), 2, WHITE);
            line(band, (cx + 8, cy - 8), (cx - 8, cy + 8), 2, WHITE);
        }
        FeedbackKind::Processing => {
            // Indeterminate: a faint ring with eight dots and a bright head circling it.
            const DOTS: [(i32, i32); 8] = [
                (0, -16),
                (11, -11),
                (16, 0),
                (11, 11),
                (0, 16),
                (-11, 11),
                (-16, 0),
                (-11, -11),
            ];
            let head = ((elapsed_ms / 100) % 8) as usize;
            ring(band, cx, cy, 17, 15, DIM);
            for (i, (dx, dy)) in DOTS.into_iter().enumerate() {
                let col = match (head + 8 - i) % 8 {
                    0 => WHITE,
                    1 => rgb(0xc8, 0xce, 0xda),
                    2 => GRAY,
                    3 => rgb(0x5a, 0x62, 0x76),
                    _ => rgb(0x2c, 0x34, 0x46),
                };
                disc(band, cx + dx, cy + dy, 3, col);
            }
        }
    }
}
