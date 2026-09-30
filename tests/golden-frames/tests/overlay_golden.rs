use kivori_framebuffer::TileBand;
use kivori_model::presentation::ValueConfidence;
use kivori_model::{Rect, Rgb565};
use kivori_renderer::hash::frame_hash;
use kivori_renderer::overlay::render_volume_overlay;

const W: u16 = 240;
const H: u16 = 240;

fn draw(percent: u8, confidence: ValueConfidence, at_boundary: bool) -> Vec<Rgb565> {
    let mut pixels = vec![Rgb565::BLACK; (W as usize) * (H as usize)];
    // Rect::new takes (x, y, w, h) in absolute display coordinates.
    let rect = Rect::new(0, 0, W, H);
    let mut band = TileBand::new(rect, &mut pixels).expect("full-frame band");
    render_volume_overlay(&mut band, percent, confidence, at_boundary);
    pixels
}

#[test]
fn overlay_is_deterministic_across_repeated_renders() {
    let a = draw(50, ValueConfidence::Confirmed, false);
    let b = draw(50, ValueConfidence::Confirmed, false);
    assert_eq!(frame_hash(&a), frame_hash(&b));
}

/// The whole point of ValueConfidence: a preview must not LOOK like confirmed truth.
#[test]
fn the_three_confidences_are_visually_distinguishable() {
    let preview = frame_hash(&draw(50, ValueConfidence::Preview, false));
    let confirmed = frame_hash(&draw(50, ValueConfidence::Confirmed, false));
    let unverified = frame_hash(&draw(50, ValueConfidence::Unverified, false));

    assert_ne!(preview, confirmed, "Preview must not render as Confirmed");
    assert_ne!(preview, unverified, "Preview must not render as Unverified");
    assert_ne!(
        confirmed, unverified,
        "Confirmed must not render as Unverified"
    );
}

#[test]
fn different_percentages_render_differently() {
    assert_ne!(
        frame_hash(&draw(0, ValueConfidence::Confirmed, false)),
        frame_hash(&draw(100, ValueConfidence::Confirmed, false))
    );
    assert_ne!(
        frame_hash(&draw(49, ValueConfidence::Confirmed, false)),
        frame_hash(&draw(51, ValueConfidence::Confirmed, false))
    );
}

#[test]
fn the_boundary_flag_changes_the_rendering() {
    assert_ne!(
        frame_hash(&draw(100, ValueConfidence::Confirmed, false)),
        frame_hash(&draw(100, ValueConfidence::Confirmed, true))
    );
}

#[test]
fn bounds_render_without_panicking_and_are_distinct() {
    let zero = frame_hash(&draw(0, ValueConfidence::Confirmed, false));
    let full = frame_hash(&draw(100, ValueConfidence::Confirmed, false));
    assert_ne!(zero, full);
}

/// Out-of-range input is clamped, never allowed to index outside the band.
#[test]
fn percent_above_one_hundred_is_clamped_to_full() {
    assert_eq!(
        frame_hash(&draw(200, ValueConfidence::Confirmed, false)),
        frame_hash(&draw(100, ValueConfidence::Confirmed, false))
    );
}

// Committed golden frame-hashes (FNV-1a over RGB565 LE bytes); see manifest.toml `[overlay.frame]`.
const H_0_CONFIRMED: u64 = 0xBC2E_D144_C581_85E5;
const H_49_CONFIRMED: u64 = 0xFA46_442C_70DB_3145;
const H_50_CONFIRMED: u64 = 0xD454_B404_1BAE_D395;
const H_51_CONFIRMED: u64 = 0xE6C3_040C_1631_2CBD;
const H_100_CONFIRMED: u64 = 0x416F_A57F_DB1E_6145;
const H_100_CONFIRMED_BOUNDARY: u64 = 0xA705_9821_BF58_F325;
const H_50_PREVIEW: u64 = 0xEC9B_2BEE_3F4F_44AD;
const H_50_UNVERIFIED: u64 = 0x0682_064E_865B_B565;

/// Each recorded manifest hash must match a fresh render: a regression that moves the overlay's
/// pixels (bar position, colour, off-by-one fill) must fail here even though the tests above only
/// compare live renders against each other.
#[test]
fn overlay_frame_hashes_match_manifest() {
    assert_eq!(
        frame_hash(&draw(0, ValueConfidence::Confirmed, false)),
        H_0_CONFIRMED,
        "percent=0, confirmed, at_boundary=false"
    );
    assert_eq!(
        frame_hash(&draw(49, ValueConfidence::Confirmed, false)),
        H_49_CONFIRMED,
        "percent=49, confirmed, at_boundary=false"
    );
    assert_eq!(
        frame_hash(&draw(50, ValueConfidence::Confirmed, false)),
        H_50_CONFIRMED,
        "percent=50, confirmed, at_boundary=false"
    );
    assert_eq!(
        frame_hash(&draw(51, ValueConfidence::Confirmed, false)),
        H_51_CONFIRMED,
        "percent=51, confirmed, at_boundary=false"
    );
    assert_eq!(
        frame_hash(&draw(100, ValueConfidence::Confirmed, false)),
        H_100_CONFIRMED,
        "percent=100, confirmed, at_boundary=false"
    );
    assert_eq!(
        frame_hash(&draw(100, ValueConfidence::Confirmed, true)),
        H_100_CONFIRMED_BOUNDARY,
        "percent=100, confirmed, at_boundary=true"
    );
    assert_eq!(
        frame_hash(&draw(50, ValueConfidence::Preview, false)),
        H_50_PREVIEW,
        "percent=50, preview, at_boundary=false"
    );
    assert_eq!(
        frame_hash(&draw(50, ValueConfidence::Unverified, false)),
        H_50_UNVERIFIED,
        "percent=50, unverified, at_boundary=false"
    );
}
