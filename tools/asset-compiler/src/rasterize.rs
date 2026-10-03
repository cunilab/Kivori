//! Deterministic SVG → RGB565 rasterization (research R-9). Renders an SVG to `width x height` with
//! `resvg`/`tiny-skia`, then converts each pixel to canonical RGB565 (little-endian bytes).

use kivori_model::Rgb565;

/// Error rasterizing an SVG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RasterError {
    /// The SVG failed to parse.
    Parse,
    /// The target pixmap could not be allocated (zero or too-large dimensions).
    Alloc,
}

/// Rasterizes an SVG and returns RGB565 bytes plus one 8-bit alpha value per pixel.
pub fn svg_to_rgb565_alpha(
    svg: &[u8],
    width: u32,
    height: u32,
) -> Result<(Vec<u8>, Vec<u8>), RasterError> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(svg, &opt).map_err(|_| RasterError::Parse)?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or(RasterError::Alloc)?;

    // Scale the SVG's intrinsic size to fill the target exactly.
    let size = tree.size();
    let transform = resvg::tiny_skia::Transform::from_scale(
        width as f32 / size.width(),
        height as f32 / size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let mut out = Vec::with_capacity((width as usize) * (height as usize) * 2);
    let mut alpha = Vec::with_capacity((width as usize) * (height as usize));
    for px in pixmap.pixels() {
        let c = px.demultiply();
        let rgb = Rgb565::from_rgb888(c.red(), c.green(), c.blue());
        out.extend_from_slice(&rgb.raw().to_le_bytes());
        alpha.push(px.alpha());
    }
    Ok((out, alpha))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg_to_rgb565(svg: &[u8], width: u32, height: u32) -> Result<Vec<u8>, RasterError> {
        svg_to_rgb565_alpha(svg, width, height).map(|(pixels, _)| pixels)
    }

    #[test]
    fn solid_red_svg_becomes_red_rgb565() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="red"/></svg>"#;
        let pixels = svg_to_rgb565(svg, 4, 4).unwrap();
        assert_eq!(pixels.len(), 4 * 4 * 2);
        // red = RGB(255,0,0) -> RGB565 0xF800 -> little-endian [0x00, 0xF8]
        let red = Rgb565::from_rgb888(255, 0, 0).raw().to_le_bytes();
        for chunk in pixels.as_chunks::<2>().0 {
            assert_eq!(chunk, &red);
        }
    }

    #[test]
    fn rasterization_is_reproducible() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="6" fill="blue"/></svg>"#;
        assert_eq!(svg_to_rgb565(svg, 16, 16), svg_to_rgb565(svg, 16, 16));
    }

    #[test]
    fn invalid_svg_errors() {
        assert_eq!(svg_to_rgb565(b"not an svg", 4, 4), Err(RasterError::Parse));
    }
}
