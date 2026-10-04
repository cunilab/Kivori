//! Export reviewable canonical device pixels, not a second renderer.
//! Run: cargo run -p kivori-golden-frames --example mascot_review
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_framebuffer::TileBand;
use kivori_model::{CompanionState, MascotAnimator, MascotExpression, MascotPose, Rect, Rgb565};
use kivori_renderer::{frame_hash, render_pose, render_scene};
use std::{fs, io::Write, path::Path};

fn ppm(path: &Path, pixels: &[Rgb565]) {
    let mut out = fs::File::create(path).unwrap();
    writeln!(out, "P6\n240 240\n255").unwrap();
    for pixel in pixels {
        let (r, g, b) = pixel.to_rgb888();
        out.write_all(&[r, g, b]).unwrap();
    }
}

fn main() {
    let directory = Path::new("assets/compiled/review");
    fs::create_dir_all(directory).unwrap();
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    println!(
        "blob_bytes={} animator_bytes={} pose_bytes={}",
        bytes.len(),
        std::mem::size_of::<MascotAnimator>(),
        std::mem::size_of::<kivori_model::MascotPose>()
    );
    for state in CompanionState::ALL {
        for ms in [0, 300, 600, 1200, 2400, 3600] {
            let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
            let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
            render_scene(&blob, blob.scene(state).unwrap(), ms, &mut band).unwrap();
            let name = format!("{state:?}-{ms}").to_lowercase();
            println!("{name}=0x{:016X}", frame_hash(&pixels));
            ppm(&directory.join(format!("{name}.ppm")), &pixels);
        }
    }
    for ms in [20_877, 21_227, 21_377, 21_457, 22_277] {
        let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
        render_scene(
            &blob,
            blob.scene(CompanionState::Idle).unwrap(),
            ms,
            &mut band,
        )
        .unwrap();
        println!("idle-life-{ms}=0x{:016X}", frame_hash(&pixels));
        ppm(&directory.join(format!("idle-life-{ms}.ppm")), &pixels);
    }
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.set_state(CompanionState::Happy, 100);
    animator.set_state(CompanionState::Sleeping, 250);
    for ms in [250, 400, 600, 849, 850] {
        let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
        render_pose(
            &blob,
            blob.scene(animator.target()).unwrap(),
            &animator.pose_at(ms),
            &mut band,
        )
        .unwrap();
        println!("transition-{ms}=0x{:016X}", frame_hash(&pixels));
        ppm(&directory.join(format!("transition-{ms}.ppm")), &pixels);
    }
    // One Idle-scene frame per observed-state face (issue #13), pinned in tests/mascot_faces.rs.
    for (name, expression) in [
        ("error", MascotExpression::Error),
        ("strained", MascotExpression::Strained),
        ("muted", MascotExpression::Muted),
        ("listening", MascotExpression::Listening),
        ("attentive", MascotExpression::Attentive),
    ] {
        let mut pose = MascotPose::for_state(CompanionState::Idle);
        pose.expression = expression;
        let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
        render_pose(
            &blob,
            blob.scene(CompanionState::Idle).unwrap(),
            &pose,
            &mut band,
        )
        .unwrap();
        println!("face-{name}=0x{:016X}", frame_hash(&pixels));
        ppm(&directory.join(format!("face-{name}.ppm")), &pixels);
    }
}
