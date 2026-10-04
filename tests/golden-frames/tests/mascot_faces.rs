//! Issue #13: the buddy's observed-state faces. One Idle-scene frame per new face (plus the
//! celebrate frame: Happy face with the deep press), pinned here and in `manifest.toml`
//! `[mascot_faces]`. Export reviewable PNG sources with `--example mascot_review`.

use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_framebuffer::TileBand;
use kivori_model::mascot::HAPPY_PRESS_Q8;
use kivori_model::{CompanionState, MascotExpression, MascotPose, Rect, Rgb565};
use kivori_renderer::{frame_hash, render_pose, render_scene};

fn render(blob: &AssetBlob, state: CompanionState, pose: &MascotPose) -> Vec<Rgb565> {
    let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
    let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
    render_pose(blob, blob.scene(state).unwrap(), pose, &mut band).unwrap();
    pixels
}

fn idle_with(expression: MascotExpression, press_q8: i32) -> MascotPose {
    let mut pose = MascotPose::for_state(CompanionState::Idle);
    pose.expression = expression;
    pose.press_q8 = press_q8;
    pose
}

#[test]
fn observed_state_faces_render_to_committed_hashes() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let cases = [
        ("error", MascotExpression::Error, 0, 0x1437_D156_6FB3_8AB2),
        (
            "strained",
            MascotExpression::Strained,
            0,
            0x1FAF_9685_20A5_64AD,
        ),
        ("muted", MascotExpression::Muted, 0, 0xF002_D0A8_222C_3523),
        (
            "listening",
            MascotExpression::Listening,
            0,
            0xDC32_707D_6C37_C51D,
        ),
        (
            "attentive",
            MascotExpression::Attentive,
            0,
            0x3CEA_C170_7B97_941D,
        ),
        (
            "celebrate",
            MascotExpression::Happy,
            HAPPY_PRESS_Q8,
            0xF27A_470A_3F4D_6B85,
        ),
    ];
    let mut seen = Vec::new();
    for (name, expression, press, expected) in cases {
        let hash = frame_hash(&render(
            &blob,
            CompanionState::Idle,
            &idle_with(expression, press),
        ));
        assert_eq!(hash, expected, "{name}");
        assert!(!seen.contains(&hash), "{name} repeats another face");
        seen.push(hash);
    }
}

/// Connection truth (invariant 21): a disconnected buddy must never be mistaken for a sleeping
/// one, so the two must differ in the eye region of their rendered scenes.
#[test]
fn offline_and_sleeping_differ_in_the_eye_region() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let scene = |state| {
        let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
        render_scene(&blob, blob.scene(state).unwrap(), 0, &mut band).unwrap();
        pixels
    };
    let (offline, sleeping) = (
        scene(CompanionState::Offline),
        scene(CompanionState::Sleeping),
    );
    // Both eyes' crops: x88..152, y96..136 (before the body's own pose offset).
    let differing = (96..136)
        .flat_map(|y| (88..152).map(move |x| y * 240 + x))
        .filter(|&i| offline[i] != sleeping[i])
        .count();
    assert!(differing > 100, "only {differing} eye pixels differ");
}
