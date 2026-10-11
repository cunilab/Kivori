//! Change-driven tile rendering (FR-013).
//!
//! Renders the current companion scene one tile at a time through the shared [`render_scene`], hashes
//! each tile, and flushes only tiles whose content changed since the last frame — the bandwidth
//! discipline the SPI panel needs. Physical mode supplies a static frame buffer: all tiles are
//! composed and hashed first, then changed tiles are transferred without composition gaps.
//! The small scratch buffer is used by the unbuffered simulation path.
//!
//! Buffered transfers merge dirty tiles: a run of horizontally adjacent tiles goes out as one
//! window, and fully dirty tile rows go out as a band of up to [`MAX_BAND_ROWS`] rows. Pixels are
//! identical to per-tile writes; only the per-window command overhead shrinks. The unbuffered path
//! composes and writes one tile at a time, so it stays per-tile.

use crate::ports::DisplaySink;
use kivori_assets::AssetBlob;
use kivori_framebuffer::{hash_rgb565, TileBand};
use kivori_model::desk::{DeskView, DisplayMode};
use kivori_model::presentation::ValueDisplay;
use kivori_model::{CompanionState, ElapsedMs, MascotAnimator, MascotPose, Rect, Rgb565};
use kivori_renderer::desk::{
    render_chrome, render_recovery, render_updating, render_view, render_views,
};
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

/// Most tile rows merged into one window. A window is one uninterrupted SPI transfer, so this
/// bounds how long the loop is blind to input between two [`TileRenderer::flush_next_window`]
/// calls (2 rows = 240x80 px = 38.4 KB, about 15 ms at 20 MHz).
pub const MAX_BAND_ROWS: usize = 2;

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
    /// Buffered path: tiles composed but not yet transferred (bit = tile index). Their new
    /// signatures are already in `signatures`; a failed pass resets them to `None`.
    pending: u64,
    #[cfg(feature = "latency-probe")]
    latency: crate::latency_probe::Readout,
    #[cfg(feature = "latency-probe")]
    probe_stats: crate::latency_probe::Stats,
}

const _: () = assert!(core::mem::size_of::<TileRenderer>() <= 4_096);
const _: () = assert!(TILE_COUNT <= 64, "`pending` is a u64 bitmask");

impl<'a> TileRenderer<'a> {
    /// Creates a renderer with an empty (all-black) scratch buffer and no cached signatures.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: [Rgb565::from_raw(0); TILE_PIXELS],
            signatures: [None; TILE_COUNT],
            frame_buffer: None,
            pending: 0,
            #[cfg(feature = "latency-probe")]
            latency: crate::latency_probe::Readout {
                last: None,
                max: None,
            },
            #[cfg(feature = "latency-probe")]
            probe_stats: crate::latency_probe::Stats {
                frame_ms: 0,
                tiles: 0,
                dropped_edges: 0,
                invalid_transitions: 0,
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

    /// Sets the frame and input counters shown beside the latency readout.
    #[cfg(feature = "latency-probe")]
    pub fn set_probe_stats(&mut self, stats: crate::latency_probe::Stats) {
        self.probe_stats = stats;
    }

    /// Forces every tile to be re-flushed on the next [`Self::render`] (e.g. after a display re-init).
    pub fn invalidate(&mut self) {
        self.signatures = [None; TILE_COUNT];
        self.pending = 0;
    }

    /// Forgets tiles that were composed but never transferred, so the next pass sends them again.
    fn discard_pending(&mut self) {
        for tile in 0..TILE_COUNT {
            if self.pending >> tile & 1 == 1 {
                self.signatures[tile] = None;
            }
        }
        self.pending = 0;
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
        self.render_inner(
            blob,
            state,
            elapsed_ms,
            None,
            overlay,
            &DeskView::default(),
            sink,
        )
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
        self.render_frame(blob, state, pose, overlay, &DeskView::default(), sink)
    }

    /// Renders one full device frame (M1): the recovery takeover when it owns the screen;
    /// otherwise the Buddy pose (plus volume overlay) or the selected desk view, then the
    /// indicator / feedback chrome over it. Only changed tiles are flushed.
    ///
    /// # Errors
    /// [`RenderError`] if the scene is missing, a tile can't be built, the compositor fails, or the
    /// sink errors.
    pub fn render_frame<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        pose: &MascotPose,
        overlay: Option<ValueDisplay>,
        view: &DeskView,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        self.render_inner(blob, state, 0, Some(pose), overlay, view, sink)
    }

    /// Composes one device frame like [`Self::render_frame`] but, on the buffered path, leaves the
    /// changed tiles pending: the caller drains them with [`Self::flush_next_window`], so it can
    /// do other work between windows. The unbuffered path writes as it composes, as before.
    ///
    /// # Errors
    /// [`RenderError`] if the scene is missing, a tile can't be built, the compositor fails, or the
    /// sink errors. Nothing stays pending after an error.
    pub fn prepare_frame<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        pose: &MascotPose,
        overlay: Option<ValueDisplay>,
        view: &DeskView,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        self.prepare_inner(blob, state, 0, Some(pose), overlay, view, sink)
    }

    /// Transfers the next pending window and reports whether more remain. A window is a run of
    /// horizontally adjacent changed tiles, or, when a tile row is changed end to end, that row
    /// merged with the fully changed rows directly below it (up to [`MAX_BAND_ROWS`]).
    ///
    /// # Errors
    /// [`RenderError::Sink`] if the transfer fails. Every window still pending is then forgotten,
    /// so the next pass sends it again.
    pub fn flush_next_window<S: DisplaySink>(
        &mut self,
        sink: &mut S,
    ) -> Result<bool, RenderError<S::Error>> {
        if self.pending == 0 {
            return Ok(false);
        }
        let Some(frame) = self.frame_buffer.as_deref() else {
            self.pending = 0;
            return Ok(false);
        };
        let first = self.pending.trailing_zeros() as usize;
        let (row, col) = (first / TILE_COLS, first % TILE_COLS);
        let row_mask = |row: usize| ((1u64 << TILE_COLS) - 1) << (row * TILE_COLS);
        let (cols, rows) = if self.pending & row_mask(row) == row_mask(row) {
            let mut rows = 1;
            while rows < MAX_BAND_ROWS
                && row + rows < TILE_COUNT / TILE_COLS
                && self.pending & row_mask(row + rows) == row_mask(row + rows)
            {
                rows += 1;
            }
            (TILE_COLS, rows)
        } else {
            let mut cols = 1;
            while col + cols < TILE_COLS && self.pending >> (first + cols) & 1 == 1 {
                cols += 1;
            }
            (cols, 1)
        };
        let rect = Rect::new(
            col as u16 * TILE_W,
            row as u16 * TILE_H,
            cols as u16 * TILE_W,
            rows as u16 * TILE_H,
        );
        // Tiles of a row run, and of a full-row band, are consecutive in the frame buffer.
        let tiles = &frame[first * TILE_PIXELS..(first + cols * rows) * TILE_PIXELS];
        if let Err(error) = sink.blit_tiles(rect, tiles, cols as u16) {
            self.discard_pending();
            return Err(RenderError::Sink(error));
        }
        for r in 0..rows {
            self.pending &= !(((1u64 << cols) - 1) << ((row + r) * TILE_COLS + col));
        }
        Ok(self.pending != 0)
    }

    /// Composes, then drains every pending window.
    #[allow(clippy::too_many_arguments)]
    fn render_inner<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        elapsed_ms: ElapsedMs,
        pose: Option<&MascotPose>,
        overlay: Option<ValueDisplay>,
        view: &DeskView,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        self.prepare_inner(blob, state, elapsed_ms, pose, overlay, view, sink)?;
        while self.flush_next_window(sink)? {}
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_inner<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        elapsed_ms: ElapsedMs,
        pose: Option<&MascotPose>,
        overlay: Option<ValueDisplay>,
        view: &DeskView,
        sink: &mut S,
    ) -> Result<(), RenderError<S::Error>> {
        self.discard_pending();
        let result = self.compose(blob, state, elapsed_ms, pose, overlay, view, sink);
        if result.is_err() {
            self.discard_pending();
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn compose<S: DisplaySink>(
        &mut self,
        blob: &AssetBlob,
        state: CompanionState,
        elapsed_ms: ElapsedMs,
        pose: Option<&MascotPose>,
        overlay: Option<ValueDisplay>,
        view: &DeskView,
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
        for tile in 0..TILE_COUNT {
            let x = (tile % TILE_COLS) as u16 * TILE_W;
            let y = (tile / TILE_COLS) as u16 * TILE_H;
            let rect = Rect::new(x, y, TILE_W, TILE_H);
            let pixels = match self.frame_buffer.as_deref_mut() {
                Some(frame) => &mut frame[tile * TILE_PIXELS..(tile + 1) * TILE_PIXELS],
                None => &mut self.buf[..],
            };
            let mut band = TileBand::new(rect, pixels).ok_or(RenderError::Band)?;
            if let Some(percent) = view.recovery_percent {
                // The recovery takeover owns the whole screen (highest layer).
                render_recovery(&mut band, percent);
            } else if view.updating {
                // Then the update takeover: the host is flashing this device.
                render_updating(&mut band);
            } else {
                let mode = view.status.mode;
                // The view layer, including the switch slide: `render_views` hands each mode a
                // (possibly scrolled) row slice of the tile, so the mascot slides like any view.
                render_views(&mut band, view, |band, m| {
                    if m == DisplayMode::Buddy {
                        match &pose {
                            Some(pose) => kivori_renderer::render_pose(blob, scene, pose, band),
                            None => render_scene(blob, scene, elapsed_ms, band),
                        }
                        .map_err(|_| RenderError::Compositor)
                    } else {
                        render_view(band, m, view, overlay);
                        Ok(())
                    }
                })?;
                // The volume bar sits over any view except Volume, which shows the value itself.
                if let (Some(value), true) = (overlay, mode != DisplayMode::Volume) {
                    render_volume_overlay(
                        &mut band,
                        value.current_percent,
                        value.confidence,
                        value.at_boundary,
                    );
                }
                render_chrome(&mut band, view);
            }
            #[cfg(feature = "latency-probe")]
            crate::latency_probe::draw(&mut band, self.latency, self.probe_stats);
            let signature = hash_rgb565(band.pixels());
            if self.signatures[tile] == Some(signature) {
                continue;
            }
            if buffered {
                // Transferred later by `flush_next_window`, once the whole frame is composed.
                self.pending |= 1 << tile;
                self.signatures[tile] = Some(signature);
            } else {
                sink.blit_tile(rect, band.pixels())
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
