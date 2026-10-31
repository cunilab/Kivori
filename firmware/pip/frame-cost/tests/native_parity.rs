//! The simulator executes the real renderer, so the frame it composes must equal, bit for bit, the
//! frame a native (host) build of the same code composes. The bench reports a checksum of its frame
//! buffer after every frame; this renders the same scenes natively and compares.
//!
//! Needs the built bench ELF: `KIVORI_FRAME_COST_ELF=path cargo test --test native_parity` (the
//! `run.sh` recipes set it). Without it the test is skipped, loudly.
use kivori_assets::AssetBlob;
use kivori_firmware::ports::DisplaySink;
use kivori_firmware::render::{TileRenderer, FRAME_PIXELS, TILE_ROWS};
use kivori_model::{Rect, Rgb565};
use kivori_pip_frame_cost::model::CycleModel;
use kivori_pip_frame_cost::report;

#[path = "../bench/src/scenes.rs"]
mod scenes;

struct NullSink;
impl DisplaySink for NullSink {
    type Error = ();
    fn blit_tile(&mut self, _: Rect, _: &[Rgb565]) -> Result<(), ()> {
        Ok(())
    }
}

#[test]
fn simulated_frames_match_a_native_render_bit_for_bit() {
    let Ok(path) = std::env::var("KIVORI_FRAME_COST_ELF") else {
        eprintln!("SKIPPED: set KIVORI_FRAME_COST_ELF to the built bench ELF");
        return;
    };
    let elf = std::fs::read(&path).unwrap();
    let simulated = report::run_elf(&elf, CycleModel::v1(), 20_000_000_000).unwrap();

    let blob_bytes = kivori_asset_compiler::compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).unwrap();
    let mut storage: Box<[Rgb565; FRAME_PIXELS]> = vec![Rgb565::from_raw(0); FRAME_PIXELS]
        .into_boxed_slice()
        .try_into()
        .unwrap();
    let raw: *mut [Rgb565; FRAME_PIXELS] = &mut *storage;
    // SAFETY: the renderer is idle whenever the frame is read.
    let mut renderer = TileRenderer::with_frame_buffer(unsafe { &mut *raw });
    let mut sink = NullSink;

    assert_eq!(simulated.len(), scenes::SCENARIOS.len());
    for (run, scenario) in simulated.iter().zip(scenes::SCENARIOS.iter()) {
        assert_eq!(run.name, scenario.name);
        assert_eq!(run.hashes.len(), scenario.frames as usize);
        for i in 0..scenario.frames {
            let f = (scenario.build)(i);
            for row in 0..TILE_ROWS {
                renderer
                    .prepare_frame_row(&blob, f.state, &f.pose, f.overlay, &f.view, row, &mut sink)
                    .unwrap();
            }
            // SAFETY: as above.
            let native = scenes::frame_hash(unsafe {
                std::slice::from_raw_parts(raw.cast::<Rgb565>(), FRAME_PIXELS)
            });
            assert_eq!(
                run.hashes[i as usize], native,
                "{} frame {i}: simulator {:#010x}, native {native:#010x}",
                scenario.name, run.hashes[i as usize]
            );
        }
    }
}
