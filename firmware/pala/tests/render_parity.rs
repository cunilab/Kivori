//! Render-parity test (T069; SC-005): the firmware's change-driven, tile-by-tile render path
//! produces exactly the same pixels as a single full-frame render of the shared renderer. This is
//! what lets the host golden frames stand in for the on-device image.
#![cfg(feature = "host-sim")]

use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::render::{TileRenderer, TILE_COUNT};
use kivori_firmware::sim::{CaptureDisplay, FRAME_PIXELS};
use kivori_framebuffer::{hash_rgb565, TileBand};
use kivori_model::{CompanionState, Rect, Rgb565};
use kivori_renderer::render_scene;

#[test]
fn buffered_frames_clear_old_pixels_and_retry_a_failed_transfer() {
    use kivori_firmware::ports::DisplaySink;
    use kivori_model::MascotAnimator;

    struct FailOnce {
        display: Box<CaptureDisplay>,
        remaining: Option<usize>,
    }
    impl FailOnce {
        /// A window either transfers whole or fails before any pixel moves.
        fn fail_or_count(&mut self) -> Result<(), ()> {
            if let Some(remaining) = &mut self.remaining {
                if *remaining == 0 {
                    self.remaining = None;
                    return Err(());
                }
                *remaining -= 1;
            }
            Ok(())
        }
    }
    impl DisplaySink for FailOnce {
        type Error = ();
        fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), ()> {
            self.fail_or_count()?;
            self.display.blit_tile(rect, pixels).unwrap();
            Ok(())
        }
        fn blit_tiles(&mut self, rect: Rect, tiles: &[Rgb565], cols: u16) -> Result<(), ()> {
            self.fail_or_count()?;
            self.display.blit_tiles(rect, tiles, cols).unwrap();
            Ok(())
        }
    }

    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let mut storage: Box<[Rgb565; FRAME_PIXELS]> = vec![Rgb565::from_raw(0); FRAME_PIXELS]
        .into_boxed_slice()
        .try_into()
        .unwrap();
    let mut renderer = TileRenderer::with_frame_buffer(&mut storage);
    let mut sink = FailOnce {
        display: Box::new(CaptureDisplay::new()),
        remaining: Some(1), // the first band (two tile rows) lands, the second fails
    };
    let mut animator = MascotAnimator::new(CompanionState::Happy, 0);
    let pose = animator.pose_at(200);
    assert!(renderer
        .render_animation(&blob, animator.target(), &pose, &mut sink)
        .is_err());
    renderer
        .render_animation(&blob, animator.target(), &pose, &mut sink)
        .unwrap();
    assert_eq!(
        sink.display.blits as usize, TILE_COUNT,
        "only successful transfers enter the cache"
    );
    for ms in [200, 400, 700, 1200, 1600, 3600] {
        if ms == 700 {
            animator.set_state(CompanionState::Idle, ms);
        }
        let pose = animator.pose_at(ms);
        let mut expected = vec![Rgb565::from_raw(0); FRAME_PIXELS];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut expected).unwrap();
        kivori_renderer::render_pose(
            &blob,
            blob.scene(animator.target()).unwrap(),
            &pose,
            &mut band,
        )
        .unwrap();
        renderer
            .render_animation(&blob, animator.target(), &pose, &mut sink)
            .unwrap();
        assert_eq!(sink.display.frame(), expected, "prepared pose at {ms} ms");
    }
    let before = sink.display.blits;
    renderer
        .render_animation(&blob, animator.target(), &animator.pose_at(3600), &mut sink)
        .unwrap();
    assert_eq!(
        sink.display.blits, before,
        "identical buffered frame sends nothing"
    );
    renderer.invalidate();
    renderer
        .render_animation(&blob, animator.target(), &animator.pose_at(3600), &mut sink)
        .unwrap();
    assert_eq!((sink.display.blits - before) as usize, TILE_COUNT);
}

#[test]
fn a_late_composition_error_cannot_partially_update_a_buffered_frame() {
    let mut bytes = compile_default_blob();
    let manifest_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    // Keep the valid manifest but remove its pixel pool: the first background tiles render,
    // then sampling the lower mascot discovers the missing bitmap data.
    bytes.truncate(16 + manifest_len);
    bytes[12..16].copy_from_slice(&0u32.to_le_bytes());
    let blob = AssetBlob::parse(&bytes).unwrap();
    let mut unbuffered = TileRenderer::new();
    let mut display = Box::new(CaptureDisplay::new());
    assert!(unbuffered
        .render(&blob, CompanionState::Idle, 0, display.as_mut())
        .is_err());
    assert!(
        display.blits > 0,
        "fixture must fail after an earlier tile was composed"
    );

    let mut storage: Box<[Rgb565; FRAME_PIXELS]> = vec![Rgb565::from_raw(0); FRAME_PIXELS]
        .into_boxed_slice()
        .try_into()
        .unwrap();
    let mut buffered = TileRenderer::with_frame_buffer(&mut storage);
    let before = display.blits;
    assert!(buffered
        .render(&blob, CompanionState::Idle, 0, display.as_mut())
        .is_err());
    assert_eq!(
        display.blits, before,
        "no display writes until the entire pose is composed"
    );
}

#[test]
fn interrupted_animation_matches_full_frame_at_every_tile_boundary() {
    use kivori_model::MascotAnimator;
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.set_state(CompanionState::Happy, 100);
    animator.set_state(CompanionState::Sleeping, 250);
    let mut renderer = TileRenderer::new();
    let mut display = Box::new(CaptureDisplay::new());
    for ms in [250, 251, 400, 600, 849, 850, 3600, 4001] {
        let pose = animator.pose_at(ms);
        let mut full = vec![Rgb565::from_raw(0); FRAME_PIXELS];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut full).unwrap();
        kivori_renderer::render_pose(
            &blob,
            blob.scene(animator.target()).unwrap(),
            &pose,
            &mut band,
        )
        .unwrap();
        renderer
            .render_animation(&blob, animator.target(), &pose, display.as_mut())
            .unwrap();
        assert_eq!(display.frame(), &full[..], "transition at {ms}ms");
    }
}

/// Renders `state` at `elapsed_ms` into a single full-frame band and returns its content hash.
fn full_frame_hash(blob: &AssetBlob, state: CompanionState, elapsed_ms: u32) -> u64 {
    let mut buf = vec![Rgb565::from_raw(0); FRAME_PIXELS];
    let scene = blob.scene(state).expect("scene present");
    let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut buf).expect("full-frame band");
    render_scene(blob, scene, elapsed_ms, &mut band).expect("render");
    hash_rgb565(&buf)
}

#[test]
fn stitched_tiles_equal_the_full_frame() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).expect("valid blob");

    for state in CompanionState::ALL {
        let expected = full_frame_hash(&blob, state, 0);

        let mut renderer = TileRenderer::new();
        let mut display = Box::new(CaptureDisplay::new());
        renderer
            .render(&blob, state, 0, display.as_mut())
            .expect("tile render");

        assert_eq!(
            hash_rgb565(display.frame()),
            expected,
            "tiling parity for {state:?}"
        );
        assert_eq!(
            display.blits as usize, TILE_COUNT,
            "all tiles flushed on first render for {state:?}"
        );
    }
}

#[test]
fn unchanged_frame_flushes_nothing_on_rerender() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).expect("valid blob");

    let mut renderer = TileRenderer::new();
    let mut display = Box::new(CaptureDisplay::new());
    renderer
        .render(&blob, CompanionState::Idle, 0, display.as_mut())
        .expect("first render");
    let after_first = display.blits;
    renderer
        .render(&blob, CompanionState::Idle, 0, display.as_mut())
        .expect("second render");

    assert_eq!(after_first as usize, TILE_COUNT);
    assert_eq!(
        display.blits, after_first,
        "an identical frame flushes no tiles (FR-013)"
    );
}

fn boxed_frame_buffer() -> Box<[Rgb565; FRAME_PIXELS]> {
    vec![Rgb565::from_raw(0); FRAME_PIXELS]
        .into_boxed_slice()
        .try_into()
        .unwrap()
}

/// Records each window's rectangle while still capturing pixels.
struct WindowLog {
    display: Box<CaptureDisplay>,
    windows: Vec<Rect>,
}

impl kivori_firmware::ports::DisplaySink for WindowLog {
    type Error = core::convert::Infallible;
    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        self.windows.push(rect);
        self.display.blit_tile(rect, pixels)
    }
    fn blit_tiles(&mut self, rect: Rect, tiles: &[Rgb565], cols: u16) -> Result<(), Self::Error> {
        self.windows.push(rect);
        self.display.blit_tiles(rect, tiles, cols)
    }
}

/// Issue #19: a full-screen change is a handful of band windows, not one window per tile, and the
/// panel ends up with exactly the pixels the per-tile path produces.
#[test]
fn a_full_screen_change_is_three_band_windows_with_identical_pixels() {
    use kivori_firmware::render::MAX_BAND_ROWS;

    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let pose = kivori_model::MascotAnimator::new(CompanionState::Happy, 0).pose_at(200);

    let mut per_tile = CaptureDisplay::new();
    TileRenderer::new()
        .render_animation(&blob, CompanionState::Happy, &pose, &mut per_tile)
        .unwrap();

    let mut storage = boxed_frame_buffer();
    let mut banded = WindowLog {
        display: Box::new(CaptureDisplay::new()),
        windows: Vec::new(),
    };
    TileRenderer::with_frame_buffer(&mut storage)
        .render_animation(&blob, CompanionState::Happy, &pose, &mut banded)
        .unwrap();

    assert_eq!(per_tile.blits as usize, TILE_COUNT);
    assert_eq!(per_tile.windows as usize, TILE_COUNT, "before: 36 windows");
    assert_eq!(banded.display.blits as usize, TILE_COUNT, "same tiles");
    assert_eq!(
        banded.display.windows as usize,
        TILE_COUNT / 6 / MAX_BAND_ROWS,
        "after: 3 windows of two tile rows"
    );
    assert_eq!(banded.display.frame(), per_tile.frame(), "same pixels");
    assert!(banded
        .windows
        .iter()
        .all(|w| w.w == 240 && w.h == 40 * MAX_BAND_ROWS as u16));
}

/// Every window is a run of adjacent changed tiles (or a band of fully changed rows): together they
/// cover exactly the tiles the per-tile path flushes, once each, and the panel image matches at
/// every step of an animation.
#[test]
fn partial_changes_merge_runs_without_touching_clean_tiles() {
    use kivori_model::MascotAnimator;

    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    let mut storage = boxed_frame_buffer();
    let mut banded = TileRenderer::with_frame_buffer(&mut storage);
    let mut per_tile = TileRenderer::new();
    let mut banded_sink = WindowLog {
        display: Box::new(CaptureDisplay::new()),
        windows: Vec::new(),
    };
    let mut tile_sink = WindowLog {
        display: Box::new(CaptureDisplay::new()),
        windows: Vec::new(),
    };
    for ms in [0, 40, 700, 2_911, 2_950, 3_100, 5_000] {
        if ms == 700 {
            animator.set_state(CompanionState::Happy, ms);
        }
        let pose = animator.pose_at(ms);
        banded_sink.windows.clear();
        tile_sink.windows.clear();
        banded
            .render_animation(&blob, animator.target(), &pose, &mut banded_sink)
            .unwrap();
        per_tile
            .render_animation(&blob, animator.target(), &pose, &mut tile_sink)
            .unwrap();

        let covered = |windows: &[Rect]| {
            let mut tiles = Vec::new();
            for w in windows {
                for ty in (w.y / 40)..((w.y + w.h) / 40) {
                    for tx in (w.x / 40)..((w.x + w.w) / 40) {
                        tiles.push((ty, tx));
                    }
                }
            }
            tiles.sort_unstable();
            tiles
        };
        // The unbuffered path writes the same changed tiles one by one.
        assert_eq!(
            covered(&banded_sink.windows),
            covered(&tile_sink.windows),
            "same tiles flushed at {ms} ms"
        );
        assert!(banded_sink.windows.len() <= tile_sink.windows.len());
        assert_eq!(
            banded_sink.display.frame(),
            tile_sink.display.frame(),
            "same pixels at {ms} ms"
        );
    }
}

#[test]
fn a_failed_band_is_resent_whole_on_the_next_frame() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let pose = kivori_model::MascotAnimator::new(CompanionState::Happy, 0).pose_at(200);
    let mut storage = boxed_frame_buffer();
    let mut renderer = TileRenderer::with_frame_buffer(&mut storage);

    struct FailSecond {
        display: Box<CaptureDisplay>,
        calls: u32,
    }
    impl kivori_firmware::ports::DisplaySink for FailSecond {
        type Error = ();
        fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), ()> {
            self.display.blit_tile(rect, pixels).map_err(|_| ())
        }
        fn blit_tiles(&mut self, rect: Rect, tiles: &[Rgb565], cols: u16) -> Result<(), ()> {
            self.calls += 1;
            if self.calls == 2 {
                return Err(());
            }
            self.display.blit_tiles(rect, tiles, cols).map_err(|_| ())
        }
    }
    let mut sink = FailSecond {
        display: Box::new(CaptureDisplay::new()),
        calls: 0,
    };
    assert!(renderer
        .render_animation(&blob, CompanionState::Happy, &pose, &mut sink)
        .is_err());
    renderer
        .render_animation(&blob, CompanionState::Happy, &pose, &mut sink)
        .unwrap();

    let mut reference = CaptureDisplay::new();
    TileRenderer::new()
        .render_animation(&blob, CompanionState::Happy, &pose, &mut reference)
        .unwrap();
    assert_eq!(sink.display.frame(), reference.frame());
}
