//! A [`TileBand`]: a mutable RGB565 view over a rectangular region backed by caller-provided storage.

use kivori_model::{Rect, Rgb565};

/// A mutable RGB565 pixel band covering `rect`, backed by a caller-provided slice of exactly
/// `rect.w * rect.h` pixels (row-major). Coordinates passed to accessors are absolute (display)
/// coordinates; out-of-band coordinates are ignored on write and return `None` on read.
pub struct TileBand<'a> {
    rect: Rect,
    pixels: &'a mut [Rgb565],
}

impl<'a> TileBand<'a> {
    /// Creates a band over `pixels`, or `None` if `pixels.len() != rect.w * rect.h`.
    #[must_use]
    pub fn new(rect: Rect, pixels: &'a mut [Rgb565]) -> Option<Self> {
        if pixels.len() == rect.area() as usize {
            Some(Self { rect, pixels })
        } else {
            None
        }
    }

    /// The band's region in absolute display coordinates.
    #[must_use]
    pub const fn rect(&self) -> Rect {
        self.rect
    }

    /// The band's pixels (row-major, `rect.w * rect.h`).
    #[must_use]
    pub fn pixels(&self) -> &[Rgb565] {
        self.pixels
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x < self.rect.x || y < self.rect.y {
            return None;
        }
        let lx = x - self.rect.x;
        let ly = y - self.rect.y;
        if lx >= self.rect.w || ly >= self.rect.h {
            return None;
        }
        Some(ly as usize * self.rect.w as usize + lx as usize)
    }

    /// Sets the pixel at absolute `(x, y)`. Ignored if `(x, y)` is outside the band.
    pub fn set(&mut self, x: u16, y: u16, color: Rgb565) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = color;
        }
    }

    /// Gets the pixel at absolute `(x, y)`, or `None` if outside the band.
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> Option<Rgb565> {
        self.index(x, y).map(|i| self.pixels[i])
    }

    /// Splits the band at absolute row `y` (clamped to the band) into the rows above it and the
    /// rows from it down. Both halves write into this band's storage.
    pub fn split_at_row(&mut self, y: u16) -> (TileBand<'_>, TileBand<'_>) {
        let r = self.rect;
        let rows = y.clamp(r.y, r.y.saturating_add(r.h)) - r.y;
        let (top, bottom) = self
            .pixels
            .split_at_mut(usize::from(rows) * usize::from(r.w));
        (
            TileBand {
                rect: Rect::new(r.x, r.y, r.w, rows),
                pixels: top,
            },
            TileBand {
                rect: Rect::new(r.x, r.y + rows, r.w, r.h - rows),
                pixels: bottom,
            },
        )
    }

    /// The same storage seen `dy` rows further down the frame, so whatever is drawn at absolute
    /// `(x, y + dy)` lands on this band's `(x, y)`: content scrolled up by `dy` (down when
    /// negative). `None` if the moved band would leave `u16` coordinates.
    pub fn scrolled(&mut self, dy: i32) -> Option<TileBand<'_>> {
        let r = self.rect;
        let y = u16::try_from(i32::from(r.y) + dy).ok()?;
        y.checked_add(r.h)?;
        Some(TileBand {
            rect: Rect::new(r.x, y, r.w, r.h),
            pixels: self.pixels,
        })
    }

    /// Fills the entire band with a single color.
    pub fn fill(&mut self, color: Rgb565) {
        for p in self.pixels.iter_mut() {
            *p = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_scroll_write_the_right_rows() {
        let mut px = [Rgb565::from_raw(0); 2 * 8];
        let mut band = TileBand::new(Rect::new(8, 40, 2, 8), &mut px[..]).unwrap();
        let (mut above, mut below) = band.split_at_row(43);
        assert_eq!(above.rect(), Rect::new(8, 40, 2, 3));
        assert_eq!(below.rect(), Rect::new(8, 43, 2, 5));
        // Content row 100 lands on the band's first row once scrolled by 60.
        above.scrolled(60).unwrap().set(9, 100, Rgb565::WHITE);
        below.scrolled(-43).unwrap().set(8, 0, Rgb565::WHITE);
        assert!(below.scrolled(-44).is_none());
        assert_eq!(band.get(9, 40), Some(Rgb565::WHITE));
        assert_eq!(band.get(8, 43), Some(Rgb565::WHITE));
        // Clamped at the band's edges.
        assert_eq!(band.split_at_row(0).1.rect().h, 8);
        assert_eq!(band.split_at_row(999).0.rect().h, 8);
    }
}
