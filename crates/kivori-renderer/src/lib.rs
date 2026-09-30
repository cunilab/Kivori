#![no_std]
#![warn(missing_docs)]
//! `kivori-renderer` — the deterministic RGB565 renderer shared by the Device Studio preview and the
//! firmware (constitution Principle II, FR-016/017/019).
//!
//! `no_std`, integer-only (no floating point), and a pure function of its inputs. The layer
//! [`compositor::render_scene`] draws a compiled scene (from `kivori-assets`) into a
//! [`kivori_framebuffer::TileBand`] at a canonical elapsed millisecond time (ADR-0004). The
//! provisional [`Scene`] trait remains for the code-defined golden-frame harness.

pub mod compositor;
pub mod frame_select;
pub mod hash;
pub mod overlay;
pub mod render;

pub use compositor::{render_pose, render_scene, RenderError};
pub use frame_select::select_frame;
pub use hash::frame_hash;
pub use render::{render_tile, Scene};
