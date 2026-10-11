//! ADR-0012: the buddy's idle moments, one Idle-scene frame each, rendered from the poses the
//! shared `MascotAnimator` resolves (the same ones the firmware and Device Studio draw), pinned
//! here and in `manifest.toml` `[idle_poses]`. They reuse the existing eye frames: only the eye
//! layer moves or squashes.

use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_framebuffer::TileBand;
use kivori_model::mascot::DEFAULT_IDLE_SEED;
use kivori_model::{
    CompanionState, LookTarget, MascotAnimator, MascotExpression, MascotPose, Rect, Rgb565,
    SCALE_Q8_ONE,
};
use kivori_renderer::{frame_hash, render_pose};

fn render(blob: &AssetBlob, pose: &MascotPose) -> Vec<Rgb565> {
    let mut pixels = vec![Rgb565::from_raw(0); 240 * 240];
    let mut band = TileBand::new(Rect::new(0, 0, 240, 240), &mut pixels).unwrap();
    render_pose(
        blob,
        blob.scene(CompanionState::Idle).unwrap(),
        pose,
        &mut band,
    )
    .unwrap();
    pixels
}

fn idle() -> MascotAnimator {
    MascotAnimator::new_seeded(CompanionState::Idle, 0, DEFAULT_IDLE_SEED)
}

/// A moment with no blink, gaze drift or expression of its own, so a frame shows only the new
/// idle pose: the animator's own pose there is the plain Idle face.
const CALM_MS: u32 = 12_000;

#[test]
fn the_calm_moment_is_a_plain_idle_face() {
    let pose = idle().pose_at(CALM_MS + 700);
    assert_eq!(pose.expression, MascotExpression::Idle);
    assert_eq!(pose.eyes_scale_y_q8, SCALE_Q8_ONE);
    assert_eq!(pose.eye_offset_q8, (0, 0));
}

fn evening_blink_ms() -> u32 {
    let (normal, evening) = (idle(), {
        let mut a = idle();
        a.set_evening(true);
        a
    });
    (0..6_000)
        .find(|&ms| {
            let (n, e) = (normal.pose_at(ms), evening.pose_at(ms));
            n.eyes_scale_y_q8 == SCALE_Q8_ONE
                && n.expression == MascotExpression::Idle
                && (100..200).contains(&e.eyes_scale_y_q8)
        })
        .expect("an evening blink that is still closing where the normal one is already open")
}

#[test]
fn idle_moments_render_to_committed_hashes() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let rest = frame_hash(&render(&blob, &idle().pose_at(CALM_MS + 700)));

    let at = CALM_MS + 700;
    let mut glance_left = idle();
    assert!(glance_left.glance(-768, CALM_MS));
    let mut glance_right = idle();
    assert!(glance_right.glance(768, CALM_MS));
    let look = |target: LookTarget| {
        let mut animator = idle();
        animator.look_at(target, CALM_MS + 400);
        animator.pose_at(CALM_MS + 700)
    };
    let mut evening = idle();
    evening.set_evening(true);

    let cases = [
        (
            "glance_left",
            glance_left.pose_at(at),
            0x7B31_E27F_6130_3295,
        ),
        (
            "glance_right",
            glance_right.pose_at(at),
            0x6BAA_40DC_3193_5555,
        ),
        ("look_knob", look(LookTarget::Knob), 0x2CF4_E609_D405_C295),
        (
            "look_button_left",
            look(LookTarget::Button(0)),
            0x2148_1518_30AD_1295,
        ),
        (
            "look_button_middle",
            look(LookTarget::Button(1)),
            0xCC3C_0F77_CC9A_7EC5,
        ),
        (
            "look_button_right",
            look(LookTarget::Button(2)),
            0xABAE_D790_E5D6_3D55,
        ),
        (
            "evening_blink",
            evening.pose_at(evening_blink_ms()),
            0x7D94_F9AB_66AB_462D,
        ),
    ];
    let mut seen = vec![rest];
    for (name, pose, expected) in cases {
        // Idle life moves or squashes the eye layer only: the face is still the Idle face.
        assert_eq!(pose.expression, MascotExpression::Idle, "{name}");
        assert!(pose.eyes_scale_y_q8 >= 48, "{name} closed the eyes");
        let hash = frame_hash(&render(&blob, &pose));
        assert_eq!(hash, expected, "{name}: 0x{hash:016X}");
        assert!(!seen.contains(&hash), "{name} repeats another idle frame");
        seen.push(hash);
    }
}
