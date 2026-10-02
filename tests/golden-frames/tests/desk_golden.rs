//! Pins the M1 desk views (`kivori_renderer::desk`) to committed frame hashes. Goldens are
//! committed deliberately: a rendering change fails here until the new hashes are reviewed (see
//! the `desk_review` example) and pinned in this table and `manifest.toml` `[desk_frames]`.

use kivori_golden_frames::{desk_frames, render_desk};
use kivori_renderer::frame_hash;

const GOLDEN: &[(&str, u64)] = &[
    ("clock_0941_colon", 0x19D7_C5A5_21F8_80A5),
    ("clock_0941_blink_off", 0xF6AD_2B8D_5BAB_53A5),
    ("clock_unknown", 0x09A6_C29C_2F0F_0F25),
    ("volume_42", 0xBB52_6C92_F248_A484),
    ("volume_100_muted", 0xA740_89E5_A476_1983),
    ("volume_unknown", 0xAD61_F7A0_4465_9DA5),
    ("volume_overlay_unverified_75", 0x2753_8547_7F80_994F),
    ("media_playing", 0x9EE8_78BC_DF03_CEF4),
    ("media_paused", 0xE026_A0DF_B355_D514),
    ("media_stopped", 0xAAAA_8D35_E71F_25F8),
    ("media_unknown", 0xA494_934B_6C58_98C7),
    ("system_normal", 0x0B5B_7A82_F2A7_B5B4),
    ("system_high_load", 0x5E82_FDE4_FEAB_548C),
    ("system_unknown", 0x03DE_286D_5F9B_0F39),
    ("recovery_0", 0xDA38_4266_0920_3FFD),
    ("recovery_50", 0xA712_FB47_037D_48DD),
    ("recovery_100", 0xE1D9_6220_86BD_B959),
    ("buddy_load_pressed", 0x3B44_5B3C_E2D1_2695),
    ("buddy_indicators", 0xCA7E_BEEE_EBFC_3254),
    ("feedback_state_confirmed", 0x1B2A_D7C7_DDFA_A891),
    ("feedback_execution_confirmed", 0xBF28_88C1_B032_60C5),
    ("feedback_unverified", 0xE9DD_E03C_CF2F_115C),
    ("feedback_error", 0x6852_FBD0_7E05_AB19),
    ("feedback_processing_0", 0xD6DE_190E_B4C4_BE3D),
    ("feedback_processing_300", 0x1DCE_7E9E_4EC2_ABF9),
];

#[test]
fn desk_frame_hashes_match_manifest() {
    for (name, frame) in desk_frames() {
        let expected = GOLDEN
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("{name} has no pinned hash"))
            .1;
        assert_eq!(frame_hash(&render_desk(&frame)), expected, "{name}");
    }
}

#[test]
fn every_pinned_hash_has_a_frame() {
    let frames = desk_frames();
    for (name, _) in GOLDEN {
        assert!(frames.iter().any(|(n, _)| n == name), "{name} is stale");
    }
}

#[test]
fn desk_frames_are_pairwise_distinct() {
    let hashes: Vec<_> = desk_frames()
        .iter()
        .map(|(n, f)| (*n, frame_hash(&render_desk(f))))
        .collect();
    for (i, (a, ha)) in hashes.iter().enumerate() {
        for (b, hb) in &hashes[i + 1..] {
            assert_ne!(ha, hb, "{a} and {b} render identically");
        }
    }
}
