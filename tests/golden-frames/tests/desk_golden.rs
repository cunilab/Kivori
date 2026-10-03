//! Pins the M1 desk views (`kivori_renderer::desk`) to committed frame hashes. Goldens are
//! committed deliberately: a rendering change fails here until the new hashes are reviewed (see
//! the `desk_review` example) and pinned in this table and `manifest.toml` `[desk_frames]`.

use kivori_golden_frames::{desk_frames, render_desk};
use kivori_renderer::frame_hash;

const GOLDEN: &[(&str, u64)] = &[
    ("clock_0941_30s", 0xEE60_4E74_D8D2_2915),
    ("clock_0941_next_second", 0xE5BE_CC16_D926_BEB5),
    ("clock_unknown", 0xB1DF_015D_D00F_4075),
    ("volume_42", 0x4154_2E76_821B_B5B1),
    ("volume_100_muted", 0x6022_A63F_6411_4323),
    ("volume_unknown", 0x99D8_445C_CF24_A04F),
    ("volume_overlay_unverified_75", 0xE743_95C8_53F7_D133),
    ("media_playing", 0x3812_6DAE_DAA1_054D),
    ("media_paused", 0xCCCB_9F14_5A50_3C1F),
    ("media_stopped", 0x4135_75B7_2938_C66F),
    ("media_unknown", 0x343A_65F7_98CE_926E),
    ("system_normal", 0xED65_8664_5CA7_9BB2),
    ("system_high_load", 0x3EB8_080B_AC3A_E9CC),
    ("system_unknown", 0x51D6_F670_AF00_E37B),
    ("recovery_0", 0xAC78_4C7B_BB53_5B67),
    ("recovery_50", 0x8A54_5D62_C035_4989),
    ("recovery_100", 0x2363_7D91_4098_57A2),
    ("buddy_load_pressed", 0xF50D_D6AE_8FCE_55A5),
    ("buddy_indicators", 0x43A7_0108_2EFC_22A2),
    ("feedback_state_confirmed", 0x71CE_2E66_B414_E9C7),
    ("feedback_execution_confirmed", 0x2C09_0A40_0615_8DD8),
    ("feedback_unverified", 0xCD9B_AAF8_3E18_C40D),
    ("feedback_error", 0xEFB1_E622_43EA_3884),
    ("feedback_processing_0", 0x7A92_20F1_06C5_A57C),
    ("feedback_processing_300", 0x2BD5_F77B_74EB_9578),
    ("clock_chips", 0x6104_1B3E_A07E_619E),
    ("volume_preview_58", 0x58E7_C667_DA29_697D),
    ("media_playing_info", 0xE309_D5A5_1945_9634),
    ("media_long_title_0", 0xFB7D_5CC3_384C_0128),
    ("media_long_title_4000", 0xF671_883D_0F38_0BCD),
    ("media_paused_info", 0x5033_143E_AF6F_11DB),
    ("system_history", 0x066A_EC2D_F6A4_F72F),
    ("system_history_high_load", 0x8201_D9BB_A209_7EB1),
    ("buddy_status_row", 0x8985_0E35_B26D_BB34),
    ("switch_clock_to_volume_mid", 0x08E8_E3E7_7B01_E035),
    ("switch_media_to_system_mid", 0x321B_AE64_BA1A_3629),
    ("switch_buddy_to_clock_mid", 0x1F5F_A92B_C989_5930),
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
