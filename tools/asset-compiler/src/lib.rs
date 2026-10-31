//! `kivori-asset-compiler` (host, std): compiles layered SVG art into the deterministic RGB565 asset
//! blob consumed by `kivori-assets` (Principle XI, ADR-0004).

pub mod blob;
pub mod build_id;
pub mod rasterize;

pub use blob::{compile_default_blob, BlobBuilder};
