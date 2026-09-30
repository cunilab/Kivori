//! Change-driven tile rendering (FR-013).
//!
//! Renders the current companion scene one tile at a time through the shared [`render_scene`], hashes
//! each tile, and flushes only tiles whose content changed since the last frame — the bandwidth
//! discipline the SPI panel needs. Physical mode supplies a static frame buffer: all tiles are
//! composed and hashed first, then changed tiles are transferred without composition gaps.
//! The small scratch buffer is used by the unbuffered simulation path.

use crate::ports::DisplaySink;
use kivori_assets::AssetBlob;
use kivori_framebuffer::{hash_rgb565, TileBand};
use kivori_model::presentation::ValueDisplay;
use kivori_model::{CompanionState, ElapsedMs, MascotAnimator, MascotPose, Rect, Rgb565};
use kivori_renderer::overlay::render_volume_overlay;
use kivori_renderer::render_scene;

/// Tile width.
pub const TILE_W: u16 = 40;
/// Tile height.
pub const TILE_H: u16 = 40;
/// Panel width in pixels.
pub const PANEL_W: u16 = 240;
/// Panel height in pixels.
pub const PANEL_H: u16 = 240;
/// Pixels per tile.
pub const TILE_PIXELS: usize = TILE_W as usize * TILE_H as usize;
/// Number of tile columns covering the panel.
pub const TILE_COLS: usize = (PANEL_W / TILE_W) as usize;
/// Number of tiles covering the panel.
pub const TILE_COUNT: usize = TILE_COLS * (PANEL_H / TILE_H) as usize;
/// Pixels in the optional single-frame staging buffer.
pub const FRAME_PIXELS: usize = PANEL_W as usize * PANEL_H as usize;

/// Why a render pass failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError<E> {
    /// The blob has no scene for the requested state.
    MissingScene,
    /// The tile buffer size did not match the tile rectangle (a build-time invariant).
    Band,
    /// The shared compositor rejected the scene.
    Compositor,
    /// The display sink failed.
    Sink(E),
}

/// A change-driven tile renderer holding the per-tile scratch buffer and last-flushed signatures.
pub struct TileRenderer<'a> {
    buf: [Rgb565; TILE_PIXELS],
    signatures: [Option<u64>; TILE_COUNT],
    frame_buffer: Option<&'a mut [Rgb565; FRAME_PIXELS]>,
    #[cfg(feature = "latency-probe")]
    latency: crate::latency_probe::Readout,
}

const _: () = assert!(core::mem::size_of::<TileRenderer>() <= 4_096);

impl<'a> TileRenderer<'a> {
    /// Creates a renderer with an empty (all-black) scratch buffer and no cached signatures.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: [Rgb565::from_raw(0); TILE_PIXELS],
            signatures: [None; TILE_COUNT],
            frame_buffer: None,
            #[cfg(feature = "latency-probe")]
            latency: crate::latency_probe::Readout {
                last: None,
                max: None,
            },
        }
    }

    /// Uses caller-owned storage to finish composition before the first display write.
    /// On hardware this storage must be static, rather than a large stack temporary.
    pub fn with_frame_buffer(frame_buffer: &'a mut [Rgb565; FRAME_PIXELS]) -> Self {
        Self {
            frame_buffer: Some(frame_buffer),
            ..Self::new()
        }
    }

    /// Sets the latency readout composited as the last layer of every following frame.
    #[cfg(feature = "latency-probe")]
    pub fn set_latency_readout(&mut self, readout: crate::latency_probe::Readout) {
        self.latency = readout;
    }

    /// Forces every tile to be re-flushed on the next [`Self::render`] (e.g. after a display re-init).
    pub fn invalidate(&mut self) {
        self.signatures = [None; TILE_COUNT];
    }

    /// Renders `state` at `elapsed_ms` from `blob`, flushing only changed tiles to `sink`.
    ///
    /// # Errors
    /// [`RenderError`] if the scene is missing, a tile can't be built, the compositor fails, or the
    /// sink errors.
    pub fn render<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        elapsed_ms: ElapsedMs,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        self.render_with_overlay(blob, state, elapsed_ms, None, sink)
    }

    /// Renders `state` at `elapsed_ms` from `blob`, compositing `overlay` (if any) as the final
    /// pass so it is included in the hash that decides which tiles are flushed, then flushes only
    /// changed tiles to `sink`. While an overlay shows, the mascot uses
    /// [`MascotPose::with_overlay_room`].
    ///
    /// # Errors
    /// [`RenderError`] if the scene is missing, a tile can't be built, the compositor fails, or the
    /// sink errors.
    pub fn render_with_overlay<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        elapsed_ms: ElapsedMs,
        overlay: Option<ValueDisplay>,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        self.render_inner(blob, state, elapsed_ms, None, overlay, sink)
    }

    /// Renders a resolved shared pose, preserving transitions across state changes.
    pub fn render_animation<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        pose: &MascotPose,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        self.render_animation_with_overlay(blob, state, pose, None, sink)
    }

    /// Renders a resolved shared pose, then composites `overlay` (if any) on top of the pose
    /// before hashing, so no second frame buffer is needed. While an overlay shows, the pose is
    /// drawn with [`MascotPose::with_overlay_room`] so the keycap clears the bar.
    ///
    /// # Errors
    /// [`RenderError`] if the scene is missing, a tile can't be built, the compositor fails, or the
    /// sink errors.
    pub fn render_animation_with_overlay<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        pose: &MascotPose,
        overlay: Option<ValueDisplay>,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        self.render_inner(blob, state, 0, Some(pose), overlay, sink)
    }

    fn render_inner<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        elapsed_ms: ElapsedMs,
        pose: Option<&MascotPose>,
        overlay: Option<ValueDisplay>,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        let scene = blob.scene(state).ok_or(RenderError::MissingScene)?;
        // The mascot makes room for the overlay (shrunk and lifted clear of the bar).
        // ponytail: instant switch in and out; ease `with_overlay_room` like state transitions
        // if the jump reads as harsh on the panel.
        let pose = match overlay {
            Some(_) => Some(
                pose.copied()
                    .unwrap_or_else(|| MascotAnimator::new(state, 0).pose_at(elapsed_ms))
                    .with_overlay_room(),
            ),
            None => pose.copied(),
        };
        let buffered = self.frame_buffer.is_some();
        let mut prepared_signatures = [0; TILE_COUNT];
        for tile in 0..TILE_COUNT {
            let x = (tile % TILE_COLS) as u16 * TILE_W;
            let y = (tile / TILE_COLS) as u16 * TILE_H;
            let rect = Rect::new(x, y, TILE_W, TILE_H);
            let pixels = match self.frame_buffer.as_deref_mut() {
                Some(frame) => &mut frame[tile * TILE_PIXELS..(tile + 1) * TILE_PIXELS],
                None => &mut self.buf[..],
            };
            let mut band = TileBand::new(rect, pixels).ok_or(RenderError::Band)?;
            match &pose {
                Some(pose) => kivori_renderer::render_pose(blob, scene, pose, &mut band),
                None => render_scene(blob, scene, elapsed_ms, &mut band),
            }
            .map_err(|_| RenderError::Compositor)?;
            // The overlay is the final layer: after the pose, before the tile hash.
            if let Some(value) = overlay {
                render_volume_overlay(
                    &mut band,
                    value.current_percent,
                    value.confidence,
                    value.at_boundary,
                );
            }
            #[cfg(feature = "latency-probe")]
            crate::latency_probe::draw(&mut band, self.latency);
            let signature = hash_rgb565(band.pixels());
            prepared_signatures[tile] = signature;
            if !buffered && self.signatures[tile] != Some(signature) {
                sink.blit_tile(rect, band.pixels())
                    .map_err(RenderError::Sink)?;
                self.signatures[tile] = Some(signature);
            }
        }
        // No composition or hashing between display writes in the buffered path. The old
        // panel image stays visible while every tile of the next pose is being prepared.
        if let Some(frame) = self.frame_buffer.as_deref() {
            for (tile, signature) in prepared_signatures.into_iter().enumerate() {
                if self.signatures[tile] == Some(signature) {
                    continue;
                }
                let rect = Rect::new(
                    (tile % TILE_COLS) as u16 * TILE_W,
                    (tile / TILE_COLS) as u16 * TILE_H,
                    TILE_W,
                    TILE_H,
                );
                sink.blit_tile(rect, &frame[tile * TILE_PIXELS..(tile + 1) * TILE_PIXELS])
                    .map_err(RenderError::Sink)?;
                self.signatures[tile] = Some(signature);
            }
        }
        Ok(())
    }
}

impl Default for TileRenderer<'_> {
    fn default() -> Self {
        Self::new()
    }
}
