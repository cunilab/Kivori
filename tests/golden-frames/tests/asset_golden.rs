//! T049 — asset-backed golden frames: rasterize + compile the 6 layered mascot scenes, render each from
//! the compiled blob, and assert committed frame-hashes (SC-005) + that scenes are distinct.
//!
//! Regenerate the committed hashes deliberately if the scene art or renderer changes (Principle III).

use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_framebuffer::TileBand;
use kivori_model::{CompanionState, Rect, Rgb565};
use kivori_renderer::{frame_hash, render_scene};

const DIM: u16 = 240;

// Committed golden frame-hashes (FNV-1a over RGB565 LE bytes); see manifest.toml.
const H_BOOTING: u64 = 0x7586_05BD_FD48_D067;
const H_IDLE: u64 = 0xA0A8_C44E_2033_06C5;
const H_HAPPY: u64 = 0xF27A_470A_3F4D_6B85;
const H_BUSY: u64 = 0xCBA3_80BE_F9D4_5608;
const H_SLEEPING: u64 = 0x68E0_9265_B830_94EE;
const H_OFFLINE: u64 = 0xE477_FC3D_27A8_F331;

fn render_state_hash(state: CompanionState) -> u64 {
    let blob = compile_default_blob();
    let asset = AssetBlob::parse(&blob).unwrap();
    let scene = asset.scene(state).unwrap();
    let mut buf = vec![Rgb565::from_raw(0); DIM as usize * DIM as usize];
    let mut band = TileBand::new(Rect::new(0, 0, DIM, DIM), &mut buf).unwrap();
    render_scene(&asset, scene, 0, &mut band).unwrap();
    frame_hash(&buf)
}

#[test]
fn asset_scenes_render_to_committed_hashes() {
    let cases = [
        (CompanionState::Booting, H_BOOTING),
        (CompanionState::Idle, H_IDLE),
        (CompanionState::Happy, H_HAPPY),
        (CompanionState::Busy, H_BUSY),
        (CompanionState::Sleeping, H_SLEEPING),
        (CompanionState::Offline, H_OFFLINE),
    ];
    for (state, expected) in cases {
        assert_eq!(render_state_hash(state), expected, "{state:?}");
    }
}

#[test]
fn distinct_scenes_have_distinct_hashes() {
    let all: [u64; 6] = CompanionState::ALL.map(render_state_hash);
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j], "scenes {i} and {j} must render differently");
        }
    }
}

#[test]
fn motion_and_interrupted_transitions_match_reviewed_pixels() {
    use kivori_model::MascotAnimator;
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.set_state(CompanionState::Happy, 100);
    animator.set_state(CompanionState::Sleeping, 250);
    for (ms, expected) in [
        (250, 0xDCE9F882CA4EBCC1),
        (400, 0x26273749268AFA21),
        (600, 0xCC35D89C26FA9E52),
        (849, 0xC6AC5A8F22161766),
        (850, 0x68E09265B83094EE),
    ] {
        let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
        kivori_renderer::render_pose(
            &blob,
            blob.scene(animator.target()).unwrap(),
            &animator.pose_at(ms),
            &mut band,
        )
        .unwrap();
        assert_eq!(
            frame_hash(&pixels),
            expected,
            "interrupted transition at {ms}"
        );
    }
    for (state, ms, expected) in [
        (CompanionState::Idle, 600, 0xA0A8C44E203306C5),
        (CompanionState::Idle, 3600, 0xA0A8C44E203306C5),
        (CompanionState::Idle, 21227, 0x6580E82CF5F68E95),
        (CompanionState::Idle, 21377, 0x3EF05F2AD8CC4C61),
        (CompanionState::Idle, 21457, 0x7826FE1AEBE5AE29),
        (CompanionState::Idle, 22277, 0x493D01D3130E3E8D),
        (CompanionState::Happy, 300, 0xAA41555CA2E59DC6),
        (CompanionState::Sleeping, 1200, 0x68E09265B83094EE),
    ] {
        let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
        render_scene(&blob, blob.scene(state).unwrap(), ms, &mut band).unwrap();
        assert_eq!(frame_hash(&pixels), expected, "{state:?} at {ms}");
    }
}

#[test]
fn social_reactions_render_distinct_faces_from_the_same_semantic_state() {
    use kivori_model::{MascotExpression, MascotPose};

    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let scene = blob.scene(CompanionState::Idle).unwrap();
    let render = |pose| {
        let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
        let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
        kivori_renderer::render_pose(&blob, scene, &pose, &mut band).unwrap();
        frame_hash(&pixels)
    };

    let idle_pose = MascotPose::for_state(CompanionState::Idle);
    let idle = render(idle_pose);
    let mut affectionate_pose = idle_pose;
    affectionate_pose.expression = MascotExpression::Affectionate;
    let affectionate = render(affectionate_pose);
    let mut surprised_pose = idle_pose;
    surprised_pose.expression = MascotExpression::Surprised;
    let surprised = render(surprised_pose);

    assert_ne!(affectionate, idle);
    assert_ne!(surprised, idle);
    assert_ne!(surprised, affectionate);
}
