//! The hold ring and the profile picker: what the device draws while a switch is held towards its
//! Hold, and while the knob switch is held and turned to pick a pinned profile.
//!
//! Both are the device's own immediate response, drawn from device-local state (`HoldRing`,
//! `PickerView`). The ring appears only for a Hold the desktop said does something, and the picker
//! shows a pick as confirmed only after the desktop reported running it (invariants 2 and 3).

use super::*;
use kivori_model::picker::{HoldRing, PickerPhase};

/// The mascot's colours, so the picker's small buddy reads as the same character.
const MINT: Rgb565 = rgb(0x7b, 0xf3, 0xc6);
const MINT_DEEP: Rgb565 = rgb(0x4a, 0xcf, 0x9c);
const EYE: Rgb565 = rgb(0x10, 0x10, 0x18);
/// A brighter accent for a hold that is ready.
const ACCENT_BRIGHT: Rgb565 = rgb(0xb4, 0xea, 0xff);

/// A rounded rectangle at `alpha` sixteenths opacity: a scrim that lets what is beneath show.
fn rrect_alpha(
    band: &mut TileBand,
    (x, y, w, h): (i32, i32, i32, i32),
    r: i32,
    col: Rgb565,
    alpha: i32,
) {
    let (ix0, iy0, ix1, iy1) = (x + r, y + r, x + w - 1 - r, y + h - 1 - r);
    shape(band, (x, y, x + w, y + h), col, |px, py| {
        let dx = (ix0 - px).max(px - ix1).max(0);
        let dy = (iy0 - py).max(py - iy1).max(0);
        round_cov(i64::from(dx * dx + dy * dy), r) * alpha / FULL
    });
}

// ---------------------------------------------------------------------------------------------
// Hold ring
// ---------------------------------------------------------------------------------------------

const HOLD_RING_ARC: Arc = Arc {
    cx: CENTER,
    cy: 84,
    ro: 36,
    ri: 29,
    start: 0,
    sweep: TURN,
};

/// The hold-progress ring: a scrimmed card over whatever is on screen with a ring that fills as
/// the switch is held, the action the Hold runs, and a caption. When the ring is full it brightens
/// and says to release, which is what runs the Hold. Never a check or green: nothing has run yet.
pub(super) fn hold_ring(band: &mut TileBand, hold: &HoldRing, elapsed_ms: u32) {
    let a = &HOLD_RING_ARC;
    // The card: a hairline edge round an almost opaque fill, so it stands off the buddy beneath.
    rrect_alpha(band, (34, 34, 172, 140), 24, LINE, 16);
    rrect_alpha(band, (35, 35, 170, 138), 23, CARD, 15);
    let ready = hold.ready();
    let percent = i32::from(hold.percent.min(100));
    // The track is always drawn, so the ring reads as "will fill" from its first frame.
    arc(
        band,
        a,
        0,
        a.sweep,
        0,
        if ready { ACCENT_DIM } else { TRACK },
    );
    if ready {
        // Full and lit, with a soft halo that breathes (triangle wave, 900 ms).
        let phase = (elapsed_ms % 900) as i32;
        let breath = if phase < 450 { phase } else { 900 - phase };
        let halo = a.inset(-3);
        arc(
            band,
            &halo,
            0,
            a.sweep,
            0,
            mix(BG, ACCENT, 4 + breath * 6 / 450),
        );
        arc(band, a, 0, a.sweep, 0, ACCENT_BRIGHT);
    } else {
        let end = a.sweep * percent / 100;
        if end > 0 {
            arc(band, a, 0, end, 0, ACCENT);
            cap(band, a, 0, 0, ACCENT);
            cap(band, a, end, 0, ACCENT);
        }
    }
    // The dot at the centre grows with the hold: a button being pressed in.
    let r = 3 + percent * 15 / 100;
    disc(band, a.cx, a.cy, r, if ready { TEXT } else { ACCENT });
    let label = hold.label.as_latin1();
    text_clipped(
        band,
        F_TITLE,
        label,
        (CENTER - text_width(F_TITLE, label).min(150) / 2, 128),
        TEXT,
        (CENTER - 76, CENTER + 76, 12),
    );
    if ready {
        text_c(band, F_SMALL, b"RELEASE", CENTER, 156, ACCENT_BRIGHT);
    } else {
        text_c(band, F_SMALL, b"KEEP HOLDING", CENTER, 156, MUTED);
    }
}

// ---------------------------------------------------------------------------------------------
// Profile picker
// ---------------------------------------------------------------------------------------------

/// Scaled text: each font pixel becomes a `k` x `k` block, anchored at `(ox, oy)`.
struct Scaled<'a, 'b> {
    band: &'a mut TileBand<'b>,
    ox: i32,
    oy: i32,
    k: i32,
}

impl Dimensions for Scaled<'_, '_> {
    fn bounding_box(&self) -> Rectangle {
        self.band.bounding_box()
    }
}

impl DrawTarget for Scaled<'_, '_> {
    type Color = EgRgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            let col = Rgb565::from_raw(c.into_storage());
            for dy in 0..self.k {
                for dx in 0..self.k {
                    let (x, y) = (self.ox + p.x * self.k + dx, self.oy + p.y * self.k + dy);
                    if x >= 0 && y >= 0 {
                        self.band.set(x as u16, y as u16, col);
                    }
                }
            }
        }
        Ok(())
    }
}

/// `s` in `font` blown up `k` times, centred on `cx`, top at `y`.
fn text_scaled(
    band: &mut TileBand,
    font: &MonoFont,
    s: &[u8],
    (cx, y): (i32, i32),
    k: i32,
    col: Rgb565,
) {
    let r = band.rect();
    let h = font.character_size.height as i32 * k;
    if y >= i32::from(r.y) + i32::from(r.h) || y + h <= i32::from(r.y) {
        return;
    }
    let x0 = cx - text_width(font, s) * k / 2;
    let style = MonoTextStyle::new(font, eg_color(col));
    let step = advance(font);
    let mut target = Scaled {
        band,
        ox: x0,
        oy: y,
        k,
    };
    for (i, &b) in s.iter().enumerate() {
        let mut buf = [0u8; 4];
        let glyph = char::from(b).encode_utf8(&mut buf);
        let _ = Text::with_baseline(glyph, Point::new(i as i32 * step, 0), style, Baseline::Top)
            .draw(&mut target);
    }
}

/// The picker's little buddy: the mascot's keycap and eyes, glancing the way the knob turns and
/// blinking now and then. `badge` is the outcome mark of a finished pick.
fn mini_buddy(band: &mut TileBand, glance: i32, elapsed_ms: u32, press: i32) {
    let (x, y) = (CENTER - 40, 22 + press);
    rrect(band, x, 50, 80, 40, 16, MINT_DEEP);
    rrect(band, x, y, 80, 50, 17, MINT);
    // Eyes: a blink every ~3.4 s lasting ~130 ms squashes them flat.
    let phase = (elapsed_ms + 1_500) % 3400;
    let open = if phase < 130 { 3 } else { 16 };
    for ex in [CENTER - 17, CENTER + 17] {
        let cx = ex + glance;
        rrect(band, cx - 4, y + 25 - open / 2, 8, open, 4, EYE);
    }
}

/// The dot row's x positions: Auto, then each pinned profile, evenly spread.
fn dot_x(index: i32, count: i32) -> i32 {
    if count <= 1 {
        return CENTER;
    }
    let gap = (192 / (count - 1)).min(32);
    CENTER - gap * (count - 1) / 2 + gap * index
}

/// The profile picker, owning the whole screen. Draws every pixel of the band.
pub fn render_picker(band: &mut TileBand, view: &DeskView) {
    band.fill(BG);
    let Some(p) = view.picker else {
        return;
    };
    let elapsed = view.elapsed_ms;
    let (count, index, from) = (i32::from(p.count), i32::from(p.index), i32::from(p.from));
    let auto = p.index == 0;
    // The buddy looks the way the highlight is moving, until it has settled.
    let moving = p.moved_ms < 260 && p.index != p.from;
    let glance = if !moving {
        0
    } else if p.index > p.from {
        3
    } else {
        -3
    };
    let sunk = i32::from(p.phase == PickerPhase::Choosing && p.moved_ms < 120) * 2;
    mini_buddy(band, glance, elapsed, sunk);

    // The outcome mark, top right of the buddy.
    match p.phase {
        PickerPhase::Confirmed => {
            disc(band, CENTER + 46, 30, 12, GREEN);
            line(band, (CENTER + 41, 30), (CENTER + 45, 34), 1, TEXT);
            line(band, (CENTER + 45, 34), (CENTER + 52, 26), 1, TEXT);
        }
        PickerPhase::Unconfirmed => {
            ring(band, CENTER + 46, 30, 12, 9, AMBER);
            text_c(band, F_LABEL, b"?", CENTER + 47, 23, AMBER);
        }
        PickerPhase::Choosing | PickerPhase::Waiting => {}
    }

    // The name, large. Auto has none of its own.
    let name: &[u8] = if auto { b"Auto" } else { p.name.as_latin1() };
    let wide = text_width(F_TITLE, name);
    if wide * 2 <= SCREEN - 12 {
        text_scaled(band, F_TITLE, name, (CENTER, 108), 2, TEXT);
    } else {
        let x = CENTER - wide.min(SCREEN - 20) / 2;
        text_clipped(band, F_TITLE, name, (x, 118), TEXT, (10, SCREEN - 10, 12));
    }

    // One honest line about what this is, and what it is doing.
    let (line_text, tone): (&[u8], Rgb565) = match (p.phase, auto) {
        (PickerPhase::Choosing, true) => (b"Follows the app in front", MUTED),
        (PickerPhase::Choosing, false) => (b"Stays on this profile", MUTED),
        (PickerPhase::Waiting, _) => (b"Switching...", MUTED),
        (PickerPhase::Confirmed, true) => (b"Following your apps", GREEN),
        (PickerPhase::Confirmed, false) => (b"Pinned", GREEN),
        (PickerPhase::Unconfirmed, _) => (b"Not confirmed", AMBER),
    };
    text_c(band, F_LABEL, line_text, CENTER, 156, tone);

    // The row: Auto, then the pinned profiles. The highlight slides between them.
    let y = 190;
    if count > 1 {
        let (x0, x1) = (dot_x(0, count), dot_x(count - 1, count));
        rrect(band, x0, y - 1, x1 - x0, 2, 1, TRACK);
    }
    for i in 0..count {
        let x = dot_x(i, count);
        if i == 0 {
            // Auto is a hollow mark, the pinned profiles are dots.
            ring(band, x, y, 5, 3, FAINT);
        } else {
            disc(band, x, y, 3, FAINT);
        }
    }
    let (fx, tx) = (dot_x(from, count), dot_x(index, count));
    let x = fx + ((tx - fx) * p.slide_q12() + 2048) / 4096;
    let (halo, fill) = match p.phase {
        PickerPhase::Confirmed => (rgb(0x14, 0x3b, 0x2a), GREEN),
        PickerPhase::Unconfirmed => (AMBER_DIM, AMBER),
        PickerPhase::Choosing | PickerPhase::Waiting => (ACCENT_DIM, ACCENT),
    };
    // While waiting the halo breathes: the pick is out, nothing is confirmed yet.
    let breathe = if p.phase == PickerPhase::Waiting {
        let t = (p.phase_ms % 700) as i32;
        (if t < 350 { t } else { 700 - t }) * 4 / 350
    } else {
        0
    };
    disc(band, x, y, 12 + breathe, halo);
    disc(band, x, y, 7, fill);
    text_c(
        band,
        F_SMALL,
        b"AUTO",
        dot_x(0, count),
        y + 14,
        if auto { MUTED } else { FAINT },
    );
    if p.phase == PickerPhase::Choosing {
        text_c(band, F_SMALL, b"RELEASE TO SELECT", CENTER, 226, FAINT);
    }
}
