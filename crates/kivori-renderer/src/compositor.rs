//! Tile compositor for generic scenes and the layered, alpha4 mascot contract.

use embedded_graphics::mono_font::{ascii, MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::raw::RawU16;
use embedded_graphics::pixelcolor::Rgb565 as EgRgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use kivori_assets::{AssetBlob, LayerDef, SceneDef};
use kivori_framebuffer::TileBand;
use kivori_model::{
    resolve_transform, ElapsedMs, FontId, LayerKind, LayerRole, MascotAnimator, MascotPose, Rgb565,
    MASCOT_ANCHOR, SCALE_Q8_ONE,
};

/// Error compositing a scene: a layer referenced an asset/string/frame that was missing or malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    /// A sprite layer referenced a bitmap not present in the blob.
    MissingBitmap,
    /// A sprite frame's pixel bytes were shorter than its dimensions require.
    ShortBitmapData,
    /// A packed alpha4 payload was shorter than its dimensions require.
    ShortAlphaData,
    /// A text layer referenced a string not present in the blob.
    MissingString,
    /// Corresponding facial layers between state scenes do not have compatible sprite geometry.
    IncompatibleFaceLayer,
}

#[derive(Clone, Copy)]
struct DrawTransform {
    offset_q8: (i32, i32),
    scale_x_q8: u16,
    scale_y_q8: u16,
    opacity: u8,
    anchored: bool,
}

impl DrawTransform {
    const IDENTITY: Self = Self {
        offset_q8: (0, 0),
        scale_x_q8: SCALE_Q8_ONE,
        scale_y_q8: SCALE_Q8_ONE,
        opacity: u8::MAX,
        anchored: false,
    };
}

fn eg_color(c: Rgb565) -> EgRgb565 {
    EgRgb565::from(RawU16::new(c.raw()))
}

fn mono_font(font: FontId) -> &'static MonoFont<'static> {
    match font {
        1 => &ascii::FONT_9X15,
        _ => &ascii::FONT_6X10,
    }
}

fn loop_time(scene: &SceneDef, elapsed_ms: ElapsedMs) -> ElapsedMs {
    if scene.frame_count > 1 && scene.fps.num > 0 {
        let loop_ms = (u64::from(scene.frame_count) * 1000 * u64::from(scene.fps.den)
            / u64::from(scene.fps.num)) as u32;
        if loop_ms > 0 {
            return elapsed_ms % loop_ms;
        }
    }
    elapsed_ms
}

/// Renders a scene at `elapsed_ms`. Mascot-role scenes use the shared deterministic pose controller;
/// generic scenes retain their legacy step-held timeline behavior.
pub fn render_scene(
    blob: &AssetBlob,
    scene: &SceneDef,
    elapsed_ms: ElapsedMs,
    band: &mut TileBand,
) -> Result<(), RenderError> {
    if scene
        .layers
        .iter()
        .any(|layer| layer.role != LayerRole::Static)
    {
        let pose = MascotAnimator::new(scene.id, 0).pose_at(elapsed_ms);
        return render_pose(blob, scene, &pose, band);
    }
    render_legacy_scene(blob, scene, elapsed_ms, band)
}

/// Renders a resolved mascot pose into `band` without allocating a second framebuffer.
///
/// Facial layers are mixed per output pixel from every state scene before composition, avoiding
/// the opacity loss caused by source-over drawing multiple partially weighted faces.
pub fn render_pose(
    blob: &AssetBlob,
    scene: &SceneDef,
    pose: &MascotPose,
    band: &mut TileBand,
) -> Result<(), RenderError> {
    band.fill(scene.background);
    for (ordinal, layer) in scene.layers.iter().enumerate() {
        match layer.role {
            LayerRole::Static => render_layer(blob, layer, 0, DrawTransform::IDENTITY, band)?,
            LayerRole::Body => render_layer(blob, layer, 0, body_transform(pose), band)?,
            LayerRole::Cap => render_layer(blob, layer, 0, pressed_transform(pose), band)?,
            LayerRole::Eyes | LayerRole::Mouth => {
                render_face_layer(blob, scene, ordinal, layer.role, pose, band)?;
            }
        }
    }
    Ok(())
}

fn render_legacy_scene(
    blob: &AssetBlob,
    scene: &SceneDef,
    elapsed_ms: ElapsedMs,
    band: &mut TileBand,
) -> Result<(), RenderError> {
    band.fill(scene.background);
    let time = loop_time(scene, elapsed_ms);
    for layer in &scene.layers {
        render_layer(blob, layer, time, DrawTransform::IDENTITY, band)?;
    }
    Ok(())
}

fn body_transform(pose: &MascotPose) -> DrawTransform {
    DrawTransform {
        offset_q8: pose.body_offset_q8,
        scale_x_q8: pose.body_scale_q8,
        scale_y_q8: pose.body_scale_q8,
        opacity: pose.opacity,
        anchored: true,
    }
}

/// The body transform plus the press: the cap and the face printed on it sink into the base.
fn pressed_transform(pose: &MascotPose) -> DrawTransform {
    let mut transform = body_transform(pose);
    transform.offset_q8.1 += pose.press_q8;
    transform
}

fn face_transform(layer: &LayerDef, role: LayerRole, pose: &MascotPose) -> DrawTransform {
    let mut transform = pressed_transform(pose);
    if role == LayerRole::Eyes {
        let individual_scale = if layer.origin.x < MASCOT_ANCHOR.x {
            pose.left_eye_scale_y_q8
        } else {
            pose.right_eye_scale_y_q8
        };
        transform.scale_y_q8 = ((u32::from(transform.scale_y_q8) * u32::from(pose.eyes_scale_y_q8)
            / u32::from(SCALE_Q8_ONE))
            * u32::from(individual_scale)
            / u32::from(SCALE_Q8_ONE)) as u16;
        transform.offset_q8.0 += pose.eye_offset_q8.0;
        transform.offset_q8.1 += pose.eye_offset_q8.1;
        if let LayerKind::Sprite { frame_size, .. } = layer.kind {
            let crop_centre_y = i32::from(layer.origin.y) + i32::from(frame_size.h) / 2;
            transform.offset_q8.1 += (crop_centre_y - i32::from(MASCOT_ANCHOR.y))
                * (i32::from(pose.body_scale_q8) - i32::from(transform.scale_y_q8));
        }
    }
    transform
}

fn render_layer(
    blob: &AssetBlob,
    layer: &LayerDef,
    elapsed_ms: ElapsedMs,
    transform: DrawTransform,
    band: &mut TileBand,
) -> Result<(), RenderError> {
    let tf = resolve_transform(&layer.keyframes, elapsed_ms);
    if !tf.visible {
        return Ok(());
    }
    match layer.kind {
        LayerKind::SolidRect { size, color } => {
            let x = i32::from(layer.origin.x.saturating_add(tf.offset.x));
            let y = i32::from(layer.origin.y.saturating_add(tf.offset.y));
            Rectangle::new(
                Point::new(x, y),
                Size::new(u32::from(size.w), u32::from(size.h)),
            )
            .into_styled(PrimitiveStyle::with_fill(eg_color(color)))
            .draw(band)
            .unwrap();
            Ok(())
        }
        LayerKind::Sprite { asset, frame_size } => draw_sprite(
            blob,
            asset,
            frame_size,
            tf.sprite_frame,
            (
                layer.origin.x.saturating_add(tf.offset.x),
                layer.origin.y.saturating_add(tf.offset.y),
            ),
            transform,
            band,
        ),
        LayerKind::Text {
            font,
            string,
            color,
        } => {
            let text = blob.string(string).ok_or(RenderError::MissingString)?;
            let x = i32::from(layer.origin.x.saturating_add(tf.offset.x));
            let y = i32::from(layer.origin.y.saturating_add(tf.offset.y));
            Text::new(
                text,
                Point::new(x, y),
                MonoTextStyle::new(mono_font(font), eg_color(color)),
            )
            .draw(band)
            .unwrap();
            Ok(())
        }
    }
}

fn render_face_layer(
    blob: &AssetBlob,
    selected: &SceneDef,
    ordinal: usize,
    role: LayerRole,
    pose: &MascotPose,
    band: &mut TileBand,
) -> Result<(), RenderError> {
    let selected_layer = selected
        .layers
        .get(ordinal)
        .ok_or(RenderError::IncompatibleFaceLayer)?;
    let LayerKind::Sprite { frame_size, .. } = selected_layer.kind else {
        return Err(RenderError::IncompatibleFaceLayer);
    };
    if selected_layer.role != role {
        return Err(RenderError::IncompatibleFaceLayer);
    }
    let LayerKind::Sprite { asset, .. } = selected_layer.kind else {
        return Err(RenderError::IncompatibleFaceLayer);
    };
    let frame = match role {
        LayerRole::Eyes => pose.expression.eye_frame(),
        LayerRole::Mouth => pose.expression.mouth_frame(),
        LayerRole::Static | LayerRole::Body | LayerRole::Cap => {
            return Err(RenderError::IncompatibleFaceLayer)
        }
    };
    draw_sprite(
        blob,
        asset,
        frame_size,
        frame,
        (selected_layer.origin.x, selected_layer.origin.y),
        face_transform(selected_layer, role, pose),
        band,
    )
}

fn draw_sprite(
    blob: &AssetBlob,
    asset: u16,
    frame_size: kivori_model::Size,
    frame: u16,
    origin: (i16, i16),
    transform: DrawTransform,
    band: &mut TileBand,
) -> Result<(), RenderError> {
    let (origin_x, origin_y) = origin;
    if transform.scale_x_q8 == 0 || transform.scale_y_q8 == 0 {
        return Ok(());
    }
    let entry = blob.bitmap(asset).ok_or(RenderError::MissingBitmap)?;
    let frame = frame % entry.frames.max(1);
    let bounds = clip_bounds(
        sprite_bounds(origin_x, origin_y, frame_size, transform),
        band,
    );
    if bounds.0 >= bounds.2 || bounds.1 >= bounds.3 {
        return Ok(());
    }
    // Everything that does not depend on the pixel is worked out once: the sprite origin, the
    // bitmap data, and the source column of each x (the same `div_euclid` as per pixel, but once
    // per column and per row). The device has no 64-bit divide, so per pixel this dominated a frame.
    let (left, top) = transformed_origin(origin_x, origin_y, transform);
    let data = SpriteData {
        pixels: blob.bitmap_pixels(entry),
        alpha: entry.alpha.map(|_| blob.bitmap_alpha(entry)),
        area: usize::from(frame_size.w) * usize::from(frame_size.h),
        width: usize::from(frame_size.w),
        frame: usize::from(frame),
    };
    let alpha_of = opacity_table(transform.opacity);
    let width = (bounds.2 - bounds.0) as usize;
    let mut columns = [None; MAX_COLUMNS];
    let mut columns = columns.get_mut(..width);
    if let Some(columns) = columns.as_deref_mut() {
        for (i, column) in columns.iter_mut().enumerate() {
            *column = source_index(
                bounds.0 + i as i32,
                left,
                transform.scale_x_q8,
                frame_size.w,
            );
        }
    }
    let mut y = bounds.1;
    while y < bounds.3 {
        if let Some(sy) = source_index(y, top, transform.scale_y_q8, frame_size.h) {
            let mut x = bounds.0;
            while x < bounds.2 {
                let sx = match columns.as_deref() {
                    Some(columns) => columns[(x - bounds.0) as usize],
                    None => source_index(x, left, transform.scale_x_q8, frame_size.w),
                };
                if let Some(sx) = sx {
                    let (color, alpha4) = data.sample(sx, sy)?;
                    composite_pixel(band, x, y, color, alpha_of[usize::from(alpha4 & 0x0F)]);
                }
                x += 1;
            }
        }
        y += 1;
    }
    Ok(())
}

/// Widest clipped sprite row whose source columns are precomputed on the stack (a 240-px panel
/// fits); a wider band computes them per pixel instead.
const MAX_COLUMNS: usize = 256;

/// The source row or column of destination `coord` (`floor((coord*256 - origin) / scale)`), or
/// `None` outside the sprite.
fn source_index(coord: i32, origin_q8: i64, scale_q8: u16, size: u16) -> Option<u16> {
    let source = (i64::from(coord) * 256 - origin_q8).div_euclid(i64::from(scale_q8));
    (0..i64::from(size))
        .contains(&source)
        .then_some(source as u16)
}

/// One sprite frame's pixel and alpha data, looked up once per draw. A missing slice is still
/// reported only when a pixel actually needs it, as before.
struct SpriteData<'a> {
    pixels: Option<&'a [u8]>,
    /// `None`: opaque bitmap; `Some(None)`: alpha declared but missing from the blob.
    alpha: Option<Option<&'a [u8]>>,
    area: usize,
    width: usize,
    frame: usize,
}

impl SpriteData<'_> {
    fn sample(&self, sx: u16, sy: u16) -> Result<(Rgb565, u8), RenderError> {
        let pixels = self.pixels.ok_or(RenderError::MissingBitmap)?;
        let index = self.frame * self.area + usize::from(sy) * self.width + usize::from(sx);
        let byte = index.checked_mul(2).ok_or(RenderError::ShortBitmapData)?;
        if byte + 2 > pixels.len() {
            return Err(RenderError::ShortBitmapData);
        }
        let alpha = match self.alpha {
            None => 15,
            Some(packed) => {
                let packed = packed.ok_or(RenderError::ShortAlphaData)?;
                let value = *packed.get(index / 2).ok_or(RenderError::ShortAlphaData)?;
                if index & 1 == 0 {
                    value & 0x0F
                } else {
                    value >> 4
                }
            }
        };
        // A fully transparent pixel changes nothing: skip reading its colour from flash.
        if alpha == 0 {
            return Ok((Rgb565::from_raw(0), 0));
        }
        Ok((
            Rgb565::from_raw(u16::from_le_bytes([pixels[byte], pixels[byte + 1]])),
            alpha,
        ))
    }
}

fn sprite_bounds(
    origin_x: i16,
    origin_y: i16,
    size: kivori_model::Size,
    transform: DrawTransform,
) -> (i32, i32, i32, i32) {
    let (left, top) = transformed_origin(origin_x, origin_y, transform);
    let right = left + i64::from(size.w) * i64::from(transform.scale_x_q8);
    let bottom = top + i64::from(size.h) * i64::from(transform.scale_y_q8);
    (
        floor_q8(left),
        floor_q8(top),
        ceil_q8(right),
        ceil_q8(bottom),
    )
}

fn clip_bounds(bounds: (i32, i32, i32, i32), band: &TileBand) -> (i32, i32, i32, i32) {
    let rect = band.rect();
    (
        bounds.0.max(i32::from(rect.x)),
        bounds.1.max(i32::from(rect.y)),
        bounds.2.min(rect.right() as i32),
        bounds.3.min(rect.bottom() as i32),
    )
}

fn transformed_origin(origin_x: i16, origin_y: i16, transform: DrawTransform) -> (i64, i64) {
    if transform.anchored {
        (
            i64::from(MASCOT_ANCHOR.x) * 256
                + i64::from(transform.offset_q8.0)
                + i64::from(i32::from(origin_x) - i32::from(MASCOT_ANCHOR.x))
                    * i64::from(transform.scale_x_q8),
            i64::from(MASCOT_ANCHOR.y) * 256
                + i64::from(transform.offset_q8.1)
                + i64::from(i32::from(origin_y) - i32::from(MASCOT_ANCHOR.y))
                    * i64::from(transform.scale_y_q8),
        )
    } else {
        (
            i64::from(origin_x) * 256 + i64::from(transform.offset_q8.0),
            i64::from(origin_y) * 256 + i64::from(transform.offset_q8.1),
        )
    }
}

fn floor_q8(value: i64) -> i32 {
    value.div_euclid(256) as i32
}
fn ceil_q8(value: i64) -> i32 {
    (-(-value).div_euclid(256)) as i32
}

/// The 4-bit alpha after `opacity`, for every source alpha: `(alpha4 * opacity + 127) / 255`,
/// worked out once per sprite instead of dividing for every pixel.
fn opacity_table(opacity: u8) -> [u8; 16] {
    let mut table = [0u8; 16];
    for (alpha4, slot) in table.iter_mut().enumerate() {
        *slot = ((alpha4 as u16 * u16::from(opacity) + 127) / 255) as u8;
    }
    table
}

/// `x / 15` for every `x` a blend produces (at most `255 * 15 + 7`), as a multiply and shift:
/// the device's divide is slow. Exhaustively tested.
const fn div15(x: u32) -> u32 {
    (x * 0x889) >> 15
}

fn composite_pixel(band: &mut TileBand, x: i32, y: i32, source: Rgb565, alpha: u8) {
    if alpha == 0 || x < 0 || y < 0 || x > i32::from(u16::MAX) || y > i32::from(u16::MAX) {
        return;
    }
    let (x, y) = (x as u16, y as u16);
    let Some(destination) = band.get(x, y) else {
        return;
    };
    if alpha >= 15 {
        band.set(x, y, source);
        return;
    }
    let (sr, sg, sb) = source.to_rgb888();
    let (dr, dg, db) = destination.to_rgb888();
    let a = u32::from(alpha);
    let blend = |source: u8, destination: u8| -> u8 {
        div15(u32::from(source) * a + u32::from(destination) * (15 - a) + 7) as u8
    };
    band.set(
        x,
        y,
        Rgb565::from_rgb888(blend(sr, dr), blend(sg, dg), blend(sb, db)),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn div15_matches_division_for_every_blend_value() {
        for x in 0..=(255 * 15 + 7) {
            assert_eq!(super::div15(x), x / 15, "x = {x}");
        }
    }

    #[test]
    fn opacity_table_matches_the_per_pixel_formula() {
        for opacity in 0..=255u8 {
            let table = super::opacity_table(opacity);
            for alpha4 in 0..16u16 {
                assert_eq!(
                    u16::from(table[alpha4 as usize]),
                    (alpha4 * u16::from(opacity) + 127) / 255
                );
            }
        }
    }

    use super::*;
    use heapless::Vec;
    use kivori_model::{CompanionState, Keyframe, Point, Size};

    fn eyes_layer() -> LayerDef {
        LayerDef {
            kind: LayerKind::Sprite {
                asset: 0,
                frame_size: Size::new(32, 40),
            },
            role: LayerRole::Eyes,
            origin: Point::new(88, 96),
            keyframes: Vec::<Keyframe, { kivori_assets::MAX_KEYFRAMES }>::new(),
        }
    }

    fn mouth_layer() -> LayerDef {
        LayerDef {
            kind: LayerKind::Sprite {
                asset: 0,
                frame_size: Size::new(48, 24),
            },
            role: LayerRole::Mouth,
            origin: Point::new(96, 134),
            keyframes: Vec::<Keyframe, { kivori_assets::MAX_KEYFRAMES }>::new(),
        }
    }

    /// Output y (Q8) of canonical art row `y` under `transform`.
    fn art_y_q8(y: i16, transform: DrawTransform) -> i64 {
        transformed_origin(0, y, transform).1
    }

    #[test]
    fn press_sinks_cap_and_face_but_leaves_the_base_on_the_desk() {
        let rest = MascotPose::for_state(CompanionState::Idle);
        let mut pressed = rest;
        pressed.press_q8 = 16 * 256;

        assert_eq!(
            body_transform(&pressed).offset_q8,
            body_transform(&rest).offset_q8
        );
        let sunk = |a: DrawTransform, b: DrawTransform| art_y_q8(100, a) - art_y_q8(100, b);
        assert_eq!(
            sunk(pressed_transform(&pressed), pressed_transform(&rest)),
            16 * 256
        );
        for (layer, role) in [
            (eyes_layer(), LayerRole::Eyes),
            (mouth_layer(), LayerRole::Mouth),
        ] {
            assert_eq!(
                sunk(
                    face_transform(&layer, role, &pressed),
                    face_transform(&layer, role, &rest)
                ),
                16 * 256,
                "{role:?}"
            );
        }
    }

    #[test]
    fn overlay_room_shrinks_and_lifts_the_keycap_clear_of_the_volume_bar() {
        const BAR_TOP: i64 = 180 * 256;
        let rest = MascotPose::for_state(CompanionState::Idle).with_overlay_room();
        assert_eq!(rest.body_scale_q8, 184);
        // The base's bottom edge (art row 206) lands at ~y172, above the bar.
        let base_bottom = art_y_q8(206, body_transform(&rest));
        assert!(
            (172 * 256..173 * 256).contains(&base_bottom),
            "{base_bottom}"
        );
        // Even a full press keeps the cap (art rows ..172) above the bar and on its base.
        let mut pressed = MascotPose::for_state(CompanionState::Happy);
        pressed.press_q8 = 16 * 256;
        let pressed = pressed.with_overlay_room();
        let cap_bottom = art_y_q8(172, pressed_transform(&pressed));
        assert!(cap_bottom < BAR_TOP && cap_bottom < base_bottom);
        // Horizontal centre stays on the anchor.
        let (left, _) = transformed_origin(28, 48, body_transform(&rest));
        let right = left + 184 * i64::from(rest.body_scale_q8);
        assert_eq!(left + right, 2 * i64::from(MASCOT_ANCHOR.x) * 256);
    }

    #[test]
    fn eye_blink_keeps_the_local_crop_centre_stationary() {
        let normal = MascotPose::for_state(CompanionState::Idle);
        let mut blink = normal;
        blink.eyes_scale_y_q8 = 128;
        let layer = eyes_layer();
        let normal_transform = face_transform(&layer, LayerRole::Eyes, &normal);
        let blink_transform = face_transform(&layer, LayerRole::Eyes, &blink);
        let (_, normal_top) = transformed_origin(layer.origin.x, layer.origin.y, normal_transform);
        let (_, blink_top) = transformed_origin(layer.origin.x, layer.origin.y, blink_transform);
        let height = 40_i64;

        assert_eq!(
            normal_top + height * i64::from(normal_transform.scale_y_q8) / 2,
            blink_top + height * i64::from(blink_transform.scale_y_q8) / 2
        );
    }

    #[test]
    fn clipping_limits_sprite_work_to_the_current_tile() {
        let mut pixels = [Rgb565::BLACK; 4];
        let band = TileBand::new(kivori_model::Rect::new(5, 7, 2, 2), &mut pixels).unwrap();
        assert_eq!(clip_bounds((0, 0, 240, 240), &band), (5, 7, 7, 9));
    }
}
