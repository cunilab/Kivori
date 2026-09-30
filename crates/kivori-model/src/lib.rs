#![no_std]
#![warn(missing_docs)]
//! `kivori-model` — the shared canonical domain types for Kivori.
//!
//! `no_std`, no `alloc`, and no floating point in rendering-facing math. This crate holds the three
//! separate state axes (the six-state companion axis, the desktop-sendable subset, the
//! device-originated lifecycle subset, and the connection-lifecycle axis), the device profile, the
//! deterministic integer-millisecond timeline, protocol version/capability value types, and pixel
//! geometry/color primitives.
//!
//! Wire framing/codec (`kivori-protocol`), rendering (`kivori-renderer`), and assets
//! (`kivori-assets`) live in other crates and depend on these types.

pub mod capabilities;
pub mod color;
pub mod connection;
pub mod geometry;
pub mod input;
pub mod mascot;
pub mod presentation;
pub mod profile;
pub mod scene;
pub mod state;
pub mod timeline;
pub mod version;

pub use capabilities::Capabilities;
pub use color::Rgb565;
pub use connection::{ConnectionEvent, ConnectionState};
pub use geometry::{Point, Rect, Size};
pub use mascot::{
    MascotAction, MascotAnimator, MascotExpression, MascotPersonality, MascotPose, MASCOT_ANCHOR,
    MASCOT_SLEEP_TRANSITION_MS, MASCOT_TRANSITION_MS, SCALE_Q8_ONE,
};
pub use profile::{ColorFormat, DeviceProfile, PanelController, ProfileError, TileConfig};
pub use scene::{
    resolve_transform, AssetId, FontId, Keyframe, LayerKind, LayerRole, ResolvedTransform, StringId,
};
pub use state::{CompanionState, LifecycleState, NotSendable, SendableState};
pub use timeline::{frame_step_ms, scene_frame, ElapsedMs, FrameRate, StudioTimeline, PREVIEW_FPS};
pub use version::{ProtocolVersion, VersionError};
