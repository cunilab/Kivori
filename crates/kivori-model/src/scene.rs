//! Canonical scene value types (ADR-0004, data-model §6): the leaf records of the layered scene
//! model and the pure, integer, step-held keyframe resolver.
//!
//! The aggregate scene/layer *views* (which borrow the compiled blob) live in `kivori-assets`; the
//! layer compositor lives in `kivori-renderer`. This module owns only the small `Copy` value types
//! shared by both (and serialized into the compiled blob via `postcard`).

use crate::{Point, Rgb565, Size};
use serde::{Deserialize, Serialize};

/// Index of a compiled sprite bitmap in the asset blob's bitmap table.
pub type AssetId = u16;
/// Index of a compiled bitmap font in the asset blob's font table.
pub type FontId = u16;
/// Index of a compiled string in the asset blob's string table.
pub type StringId = u16;

/// Semantic role of an asset layer in the shared mascot composition contract.
///
/// `Static` preserves generic scene behavior. The remaining roles are composed using a
/// [`crate::MascotPose`]: every mascot layer shares a bottom-centre transform; the cap and the face
/// printed on it additionally sink by the pose's press, eyes receive the blink squash, and
/// mouths/eyes are selected by expression.
///
/// Variants are postcard-encoded by index: append new roles at the end only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerRole {
    /// A generic layer unaffected by mascot pose composition.
    Static,
    /// The keycap's fixed base (side walls and front lip); it stays on the desk during a press.
    Body,
    /// The interchangeable pair of eyes.
    Eyes,
    /// The interchangeable mouth expression.
    Mouth,
    /// The keycap's cap (top face, upper wall wedges, legend); it sinks into the base on a press.
    Cap,
}

/// One keyframe of a layer's timeline. At `at_ms` (and until the next keyframe), the layer uses this
/// transform, sprite frame, and visibility. Animation is **step-held** — no interpolation — so
/// rendering stays integer-only and byte-identical across host and device (ADR-0004 decision 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyframe {
    /// Canonical time (ms) at which this keyframe takes effect.
    pub at_ms: u32,
    /// Integer translation applied to the layer's origin.
    pub offset: Point,
    /// Which frame of a sprite-sheet to draw (ignored by non-sprite layers).
    pub sprite_frame: u16,
    /// Whether the layer is drawn.
    pub visible: bool,
}

/// What a layer draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    /// A compiled sprite bitmap (optionally a sprite-sheet of `frame_size` frames).
    Sprite {
        /// Bitmap table index.
        asset: AssetId,
        /// Size of one sprite frame.
        frame_size: Size,
    },
    /// A solid-color rectangle.
    SolidRect {
        /// Rectangle size.
        size: Size,
        /// Fill color.
        color: Rgb565,
    },
    /// Bitmap text: a compiled string drawn with a compiled bitmap font.
    Text {
        /// Font table index.
        font: FontId,
        /// String table index.
        string: StringId,
        /// Text color.
        color: Rgb565,
    },
}

/// A layer's resolved animation state at a point in time (step-held; no interpolation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTransform {
    /// Integer translation to apply to the layer origin.
    pub offset: Point,
    /// Active sprite frame.
    pub sprite_frame: u16,
    /// Whether the layer is visible.
    pub visible: bool,
}

impl ResolvedTransform {
    /// The identity transform: no offset, frame 0, visible.
    pub const IDENTITY: ResolvedTransform = ResolvedTransform {
        offset: Point::ORIGIN,
        sprite_frame: 0,
        visible: true,
    };
}

/// Resolves a layer's keyframe timeline at `elapsed_ms` by **holding** the last keyframe whose
/// `at_ms <= elapsed_ms`.
///
/// `keyframes` MUST be sorted ascending by `at_ms`. If it is empty, the identity transform is
/// returned; if `elapsed_ms` precedes the first keyframe, the first keyframe is used. Pure and
/// integer-only (deterministic).
#[must_use]
pub fn resolve_transform(keyframes: &[Keyframe], elapsed_ms: u32) -> ResolvedTransform {
    let mut chosen: Option<&Keyframe> = None;
    let mut i = 0;
    while i < keyframes.len() {
        if keyframes[i].at_ms <= elapsed_ms {
            chosen = Some(&keyframes[i]);
        } else {
            break;
        }
        i += 1;
    }
    match chosen.or_else(|| keyframes.first()) {
        Some(kf) => ResolvedTransform {
            offset: kf.offset,
            sprite_frame: kf.sprite_frame,
            visible: kf.visible,
        },
        None => ResolvedTransform::IDENTITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kf(at_ms: u32, x: i16, frame: u16, visible: bool) -> Keyframe {
        Keyframe {
            at_ms,
            offset: Point::new(x, 0),
            sprite_frame: frame,
            visible,
        }
    }

    #[test]
    fn empty_timeline_is_identity() {
        assert_eq!(resolve_transform(&[], 1234), ResolvedTransform::IDENTITY);
    }

    #[test]
    fn holds_last_keyframe_at_or_before_time() {
        let kfs = [
            kf(0, 0, 0, true),
            kf(100, 10, 1, true),
            kf(200, 20, 2, false),
        ];
        assert_eq!(resolve_transform(&kfs, 0).offset, Point::new(0, 0));
        assert_eq!(resolve_transform(&kfs, 99).offset, Point::new(0, 0));
        assert_eq!(resolve_transform(&kfs, 100).sprite_frame, 1);
        assert_eq!(resolve_transform(&kfs, 199).offset, Point::new(10, 0));
        assert_eq!(resolve_transform(&kfs, 250).offset, Point::new(20, 0));
        assert!(!resolve_transform(&kfs, 250).visible);
    }

    #[test]
    fn time_before_first_keyframe_uses_first() {
        let kfs = [kf(50, 5, 3, true)];
        assert_eq!(resolve_transform(&kfs, 0).sprite_frame, 3);
    }
}
