//! Deterministic assembly of the compiled asset blob.

use crate::rasterize::svg_to_rgb565_alpha;
use heapless::Vec as HVec;
use kivori_assets::manifest::{BitmapEntry, LayerDef, Manifest, PoolRef, SceneDef};
use kivori_assets::{FORMAT_VERSION, MAGIC, MAX_BITMAPS, MAX_SCENES, MAX_STRINGS};
use kivori_model::{
    CompanionState, DeviceProfile, FrameRate, Keyframe, LayerKind, LayerRole, Point, Rgb565, Size,
};

const DIM: u16 = 240;
const PACKED_ALPHA_MAX: usize = 131_072;

/// Source crops `(x, y, w, h)` in the 240x240 mascot artwork, one per compiled layer.
///
/// The SVG's drawing contract gives the base (28,48 184x160), cap (40,52 160x120) and mouth
/// (96,134 48x24) roomier boxes; they are stored trimmed to their drawn pixels to fit the 128 KiB
/// budget. Trimming is lossless: anchored sampling depends only on absolute art coordinates.
/// The eye crop stays as specified because blinks squash about the crop's centre (the eye centre).
const BODY_CROP: (u32, u32, u32, u32) = (44, 88, 152, 118);
const CAP_CROP: (u32, u32, u32, u32) = (42, 56, 156, 112);
const EYE_CROP: (u32, u32, u32, u32) = (88, 96, 32, 40);
const MOUTH_CROP: (u32, u32, u32, u32) = (108, 139, 24, 11);
/// The right eye reuses the left-eye sheet, shifted by one eye crop width.
const RIGHT_EYE_ORIGIN: Point = Point::new(120, 96);

/// Assembles a compiled asset blob from rasterized sprites and scene definitions.
pub struct BlobBuilder {
    profile: DeviceProfile,
    pool: Vec<u8>,
    bitmaps: HVec<BitmapEntry, MAX_BITMAPS>,
    strings: HVec<PoolRef, MAX_STRINGS>,
    scenes: HVec<SceneDef, MAX_SCENES>,
}

impl BlobBuilder {
    #[must_use]
    pub fn new(profile: DeviceProfile) -> Self {
        Self {
            profile,
            pool: Vec::new(),
            bitmaps: HVec::new(),
            strings: HVec::new(),
            scenes: HVec::new(),
        }
    }

    /// Adds an opaque RGB565 bitmap, returning its deduplicated asset id.
    pub fn add_bitmap(&mut self, size: Size, frames: u16, pixels: &[u8]) -> u16 {
        self.add_bitmap_inner(size, frames, pixels, None)
    }

    /// Adds RGB565 bytes with one 8-bit alpha sample per pixel. Alpha is packed two samples per byte.
    pub fn add_masked_bitmap(
        &mut self,
        size: Size,
        frames: u16,
        pixels: &[u8],
        alpha: &[u8],
    ) -> u16 {
        assert_eq!(
            pixels.len(),
            usize::from(size.w) * usize::from(size.h) * usize::from(frames) * 2
        );
        assert_eq!(alpha.len(), pixels.len() / 2);
        let mut packed = Vec::with_capacity(alpha.len().div_ceil(2));
        for (i, &a) in alpha.iter().enumerate() {
            let nibble = a / 17;
            if i % 2 == 0 {
                packed.push(nibble);
            } else {
                *packed.last_mut().expect("alpha byte") |= nibble << 4;
            }
        }
        self.add_bitmap_inner(size, frames, pixels, Some(&packed))
    }

    fn add_bitmap_inner(
        &mut self,
        size: Size,
        frames: u16,
        pixels: &[u8],
        alpha: Option<&[u8]>,
    ) -> u16 {
        for (id, entry) in self.bitmaps.iter().enumerate() {
            if entry.size != size
                || entry.frames != frames
                || entry.alpha.is_some() != alpha.is_some()
            {
                continue;
            }
            let matches_pixels = self.pool_slice(entry.data) == pixels;
            let matches_alpha = match (entry.alpha, alpha) {
                (None, None) => true,
                (Some(r), Some(a)) => self.pool_slice(r) == a,
                _ => false,
            };
            if matches_pixels && matches_alpha {
                return id as u16;
            }
        }
        let data = self.push_pool(pixels);
        let alpha_ref = alpha.map(|a| self.push_pool(a));
        let id = self.bitmaps.len() as u16;
        self.bitmaps
            .push(BitmapEntry {
                size,
                frames,
                data,
                alpha: alpha_ref,
            })
            .expect("bitmap table capacity");
        id
    }

    fn pool_slice(&self, r: PoolRef) -> &[u8] {
        &self.pool[r.offset as usize..r.offset as usize + r.len as usize]
    }

    fn push_pool(&mut self, bytes: &[u8]) -> PoolRef {
        let offset = self.pool.len() as u32;
        self.pool.extend_from_slice(bytes);
        PoolRef {
            offset,
            len: bytes.len() as u32,
        }
    }

    /// Adds a static scene consisting of a single full-frame opaque sprite layer.
    pub fn add_full_sprite_scene(&mut self, id: CompanionState, background: Rgb565, asset: u16) {
        let mut keyframes = HVec::new();
        keyframes
            .push(Keyframe {
                at_ms: 0,
                offset: Point::ORIGIN,
                sprite_frame: 0,
                visible: true,
            })
            .expect("keyframe capacity");
        let mut layers = HVec::new();
        layers
            .push(LayerDef {
                role: LayerRole::Static,
                kind: LayerKind::Sprite {
                    asset,
                    frame_size: Size::new(DIM, DIM),
                },
                origin: Point::ORIGIN,
                keyframes,
            })
            .expect("layer capacity");
        self.scenes
            .push(SceneDef {
                id,
                background,
                fps: FrameRate::fps(1),
                frame_count: 1,
                layers,
            })
            .expect("scene capacity");
    }

    fn add_mascot_scene(&mut self, state: CompanionState, [body, cap, eyes, mouth]: [u16; 4]) {
        let crop =
            |c @ (x, y, _, _): (u32, u32, u32, u32)| (crop_size(c), Point::new(x as i16, y as i16));
        let (body_size, body_origin) = crop(BODY_CROP);
        let (cap_size, cap_origin) = crop(CAP_CROP);
        let (eye_size, left_eye) = crop(EYE_CROP);
        let (mouth_size, mouth_origin) = crop(MOUTH_CROP);
        let specs = [
            (LayerRole::Body, body, body_size, body_origin),
            (LayerRole::Cap, cap, cap_size, cap_origin),
            (LayerRole::Eyes, eyes, eye_size, left_eye),
            (LayerRole::Eyes, eyes, eye_size, RIGHT_EYE_ORIGIN),
            (LayerRole::Mouth, mouth, mouth_size, mouth_origin),
        ];
        let mut layers = HVec::new();
        for (role, asset, size, origin) in specs {
            let mut keyframes = HVec::new();
            keyframes
                .push(Keyframe {
                    at_ms: 0,
                    offset: Point::ORIGIN,
                    sprite_frame: 0,
                    visible: true,
                })
                .expect("keyframe capacity");
            layers
                .push(LayerDef {
                    role,
                    kind: LayerKind::Sprite {
                        asset,
                        frame_size: size,
                    },
                    origin,
                    keyframes,
                })
                .expect("layer capacity");
        }
        self.scenes
            .push(SceneDef {
                id: state,
                background: Rgb565::from_rgb888(10, 14, 24),
                fps: FrameRate::fps(30),
                frame_count: 120,
                layers,
            })
            .expect("scene capacity");
    }

    #[must_use]
    /// Finalizes the binary asset blob (`header || manifest || pool`).
    pub fn finish(self) -> Vec<u8> {
        let manifest = Manifest {
            profile: self.profile,
            bitmaps: self.bitmaps,
            strings: self.strings,
            scenes: self.scenes,
        };
        let mbytes = postcard::to_stdvec(&manifest).expect("manifest serializes");
        let mut blob = Vec::with_capacity(16 + mbytes.len() + self.pool.len());
        blob.extend_from_slice(&MAGIC.to_le_bytes());
        blob.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        blob.extend_from_slice(&0u16.to_le_bytes());
        blob.extend_from_slice(&(mbytes.len() as u32).to_le_bytes());
        blob.extend_from_slice(&(self.pool.len() as u32).to_le_bytes());
        blob.extend_from_slice(&mbytes);
        blob.extend_from_slice(&self.pool);
        blob
    }
}

fn crop_size((_, _, w, h): (u32, u32, u32, u32)) -> Size {
    Size::new(w as u16, h as u16)
}

fn layer_svg(source: &str, id: &str) -> Vec<u8> {
    let (_, rest) = source.split_once("<defs>").expect("mascot SVG has defs");
    let (defs, _) = rest.split_once("</defs>").expect("mascot SVG closes defs");
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240"><defs>{defs}</defs><use href="#{id}"/></svg>"##).into_bytes()
}

fn compile_layer(source: &str, id: &str, crop: (u32, u32, u32, u32)) -> (Vec<u8>, Vec<u8>) {
    let svg = layer_svg(source, id);
    let (pixels, alpha) = svg_to_rgb565_alpha(&svg, DIM as u32, DIM as u32).expect("mascot SVG");
    let (x, y, w, h) = crop;
    let mut cropped = Vec::with_capacity((w * h * 2) as usize);
    let mut cropped_alpha = Vec::with_capacity((w * h) as usize);
    for row in y..y + h {
        let start = ((row * DIM as u32 + x) * 2) as usize;
        cropped.extend_from_slice(&pixels[start..start + (w * 2) as usize]);
        let astart = (row * DIM as u32 + x) as usize;
        cropped_alpha.extend_from_slice(&alpha[astart..astart + w as usize]);
    }
    (cropped, cropped_alpha)
}

#[must_use]
pub fn compile_default_blob() -> Vec<u8> {
    let source = include_str!("../../../assets/mascot.svg");
    let mut builder = BlobBuilder::new(DeviceProfile::KIVORI_240);
    let (body_pixels, body_alpha) = compile_layer(source, "body", BODY_CROP);
    let body = builder.add_masked_bitmap(crop_size(BODY_CROP), 1, &body_pixels, &body_alpha);
    let (cap_pixels, cap_alpha) = compile_layer(source, "cap", CAP_CROP);
    let cap = builder.add_masked_bitmap(crop_size(CAP_CROP), 1, &cap_pixels, &cap_alpha);
    let eye_names = [
        "booting",
        "idle",
        "happy",
        "busy",
        "sleeping",
        "offline",
        "affectionate",
    ];
    let mouth_names = [
        "booting", "idle", "happy", "busy", "sleeping", "offline", "laughing",
    ];
    let mut eye_pixels = Vec::new();
    let mut eye_alpha = Vec::new();
    for name in eye_names {
        let (pixels, alpha) = compile_layer(source, &format!("eyes-{name}"), EYE_CROP);
        eye_pixels.extend_from_slice(&pixels);
        eye_alpha.extend_from_slice(&alpha);
    }
    let eyes = builder.add_masked_bitmap(
        crop_size(EYE_CROP),
        eye_names.len() as u16,
        &eye_pixels,
        &eye_alpha,
    );
    let mut mouth_pixels = Vec::new();
    let mut mouth_alpha = Vec::new();
    for name in mouth_names {
        let (pixels, alpha) = compile_layer(source, &format!("mouth-{name}"), MOUTH_CROP);
        mouth_pixels.extend_from_slice(&pixels);
        mouth_alpha.extend_from_slice(&alpha);
    }
    let mouth = builder.add_masked_bitmap(
        crop_size(MOUTH_CROP),
        mouth_names.len() as u16,
        &mouth_pixels,
        &mouth_alpha,
    );
    let states = [
        (CompanionState::Booting, "booting"),
        (CompanionState::Idle, "idle"),
        (CompanionState::Happy, "happy"),
        (CompanionState::Busy, "busy"),
        (CompanionState::Sleeping, "sleeping"),
        (CompanionState::Offline, "offline"),
    ];
    for (state, _name) in states {
        builder.add_mascot_scene(state, [body, cap, eyes, mouth]);
    }
    let blob = builder.finish();
    assert!(
        blob.len() <= PACKED_ALPHA_MAX,
        "compiled mascot pack exceeds 128 KiB: {}",
        blob.len()
    );
    blob
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layer_crop_contains_all_of_its_nontransparent_pixels() {
        let source = include_str!("../../../assets/mascot.svg");
        let faces = ["booting", "idle", "happy", "busy", "sleeping", "offline"];
        let mut layers = vec![("body".to_owned(), BODY_CROP), ("cap".to_owned(), CAP_CROP)];
        for name in faces.iter().chain(&["affectionate"]) {
            layers.push((format!("eyes-{name}"), EYE_CROP));
        }
        for name in faces.iter().chain(&["laughing"]) {
            layers.push((format!("mouth-{name}"), MOUTH_CROP));
        }
        for (id, (cx, cy, cw, ch)) in layers {
            let svg = layer_svg(source, &id);
            let (_, alpha) = svg_to_rgb565_alpha(&svg, DIM as u32, DIM as u32).unwrap();
            for y in 0..u32::from(DIM) {
                for x in 0..u32::from(DIM) {
                    if !(cx..cx + cw).contains(&x) || !(cy..cy + ch).contains(&y) {
                        let a = alpha[(y * u32::from(DIM) + x) as usize];
                        assert_eq!(a, 0, "{id} overflows its crop at ({x},{y})");
                    }
                }
            }
        }
    }
}
