//! M1 desk rendering: tile/full-frame equality, determinism, distinct feedback, explicit unknowns,
//! full coverage of the full-screen views and a no-op chrome when there is nothing to show.

use kivori_framebuffer::TileBand;
use kivori_model::desk::{
    ActionFeedback, ActionKind, ClockTime, CpuHistory, DeskStatus, DeskView, DisplayMode,
    FeedbackKind, MediaInfo, MediaStatus, MediaText, VIEW_TRANSITION_MS,
};
use kivori_model::presentation::{ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Rect, Rgb565};
use kivori_renderer::desk::{
    render_chrome, render_mode, render_recovery, render_updating, render_view, render_views,
    view_switch,
};

const DIM: u16 = 240;
const TILE: u16 = 40;
/// Not a colour the renderer uses, so any surviving sentinel pixel was never written.
const SENT: Rgb565 = Rgb565::from_raw(0x1234);

fn full(f: impl Fn(&mut TileBand)) -> Vec<Rgb565> {
    let mut buf = vec![SENT; usize::from(DIM) * usize::from(DIM)];
    let mut band = TileBand::new(Rect::new(0, 0, DIM, DIM), &mut buf).unwrap();
    f(&mut band);
    buf
}

fn tiled(f: impl Fn(&mut TileBand)) -> Vec<Rgb565> {
    let mut out = vec![SENT; usize::from(DIM) * usize::from(DIM)];
    for ty in 0..DIM / TILE {
        for tx in 0..DIM / TILE {
            let mut buf = vec![SENT; usize::from(TILE) * usize::from(TILE)];
            let rect = Rect::new(tx * TILE, ty * TILE, TILE, TILE);
            let mut band = TileBand::new(rect, &mut buf).unwrap();
            f(&mut band);
            for row in 0..TILE {
                let src = usize::from(row) * usize::from(TILE);
                let dst = usize::from(ty * TILE + row) * usize::from(DIM) + usize::from(tx * TILE);
                out[dst..dst + usize::from(TILE)]
                    .copy_from_slice(&buf[src..src + usize::from(TILE)]);
            }
        }
    }
    out
}

/// A whole frame as the device composes it: the mode view (the mascot's place stays sentinel in
/// Buddy mode), then the chrome.
fn scene(band: &mut TileBand, view: &DeskView, overlay: Option<ValueDisplay>) {
    if view.status.mode != DisplayMode::Buddy {
        render_mode(band, view, overlay);
    }
    render_chrome(band, view);
}

fn px(frame: &[Rgb565], x: u16, y: u16) -> Rgb565 {
    frame[usize::from(y) * usize::from(DIM) + usize::from(x)]
}

fn view(mode: DisplayMode, status: impl FnOnce(&mut DeskStatus)) -> DeskView {
    let mut v = DeskView::default();
    v.status.mode = mode;
    status(&mut v.status);
    v
}

fn feedback(kind: FeedbackKind) -> Option<ActionFeedback> {
    Some(ActionFeedback {
        action: ActionKind::Mute,
        kind,
    })
}

const FEEDBACKS: [FeedbackKind; 5] = [
    FeedbackKind::Processing,
    FeedbackKind::StateConfirmed,
    FeedbackKind::ExecutionConfirmed,
    FeedbackKind::Unverified,
    FeedbackKind::Error,
];

fn clock(hour: u8, minute: u8, second: u8) -> Option<ClockTime> {
    Some(ClockTime {
        hour,
        minute,
        second,
    })
}

/// Statuses covering known, unknown, boundary and flag combinations.
fn statuses(mode: DisplayMode) -> Vec<DeskStatus> {
    let mut known = DeskStatus::UNKNOWN;
    known.clock = clock(12, 34, 56);
    known.volume_percent = Some(63);
    known.muted = Some(true);
    known.media = Some(MediaStatus::Playing);
    known.cpu_percent = Some(80);
    known.ram_percent = Some(20);
    known.high_load = true;
    let mut edge = DeskStatus::UNKNOWN;
    edge.clock = clock(0, 0, 1);
    edge.volume_percent = Some(0);
    edge.muted = Some(false);
    edge.media = Some(MediaStatus::Paused);
    edge.cpu_percent = Some(0);
    edge.ram_percent = Some(100);
    let mut top = DeskStatus::UNKNOWN;
    top.clock = clock(23, 59, 58);
    top.volume_percent = Some(100);
    top.media = Some(MediaStatus::Stopped);
    top.cpu_percent = Some(100);
    let mut all = vec![DeskStatus::UNKNOWN, known, edge, top];
    for s in &mut all {
        s.mode = mode;
    }
    all
}

fn all_views() -> Vec<DeskView> {
    let mut out = Vec::new();
    for mode in DisplayMode::ALL {
        for status in statuses(mode) {
            for fb in std::iter::once(None).chain(FEEDBACKS.into_iter().map(Some)) {
                for button_down in [false, true] {
                    out.push(DeskView {
                        status,
                        feedback: fb.map(|kind| ActionFeedback {
                            action: ActionKind::Volume,
                            kind,
                        }),
                        button_down,
                        recovery_percent: None,
                        elapsed_ms: 1_250,
                        ..DeskView::default()
                    });
                }
            }
        }
    }
    out
}

fn overlay(confidence: ValueConfidence, percent: u8) -> ValueDisplay {
    ValueDisplay {
        kind: ValueKind::Volume,
        current_percent: percent,
        confidence,
        at_boundary: false,
    }
}

/// The view layer as the device composes it (Buddy paints a stand-in so its slice is visible),
/// then the chrome.
fn composed(band: &mut TileBand, view: &DeskView) {
    let _ = render_views(band, view, |b, mode| {
        render_view(b, mode, view, None);
        if mode == DisplayMode::Buddy {
            let r = b.rect();
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    if (x / 8 + y / 8) % 2 == 0 {
                        b.set(x, y, Rgb565::from_raw(0x07E0));
                    }
                }
            }
        }
        Ok::<(), ()>(())
    });
    render_chrome(band, view);
}

fn switching(from: DisplayMode, to: DisplayMode, age: u32) -> DeskView {
    let mut v = view(to, |s| {
        s.clock = clock(9, 41, 30);
        s.volume_percent = Some(42);
        s.media = Some(MediaStatus::Playing);
        s.cpu_percent = Some(37);
        s.ram_percent = Some(62);
    });
    v.previous_mode = Some(from);
    v.mode_age_ms = age;
    v
}

// 1. Tile-stitch equality ---------------------------------------------------------------------

#[test]
fn tiles_stitch_to_the_full_frame_for_every_view() {
    for v in all_views() {
        let f = |b: &mut TileBand| scene(b, &v, None);
        assert_eq!(tiled(f), full(f), "{v:?}");
    }
}

#[test]
fn tiles_stitch_to_the_full_frame_with_a_volume_overlay() {
    let v = view(DisplayMode::Volume, |s| s.volume_percent = Some(10));
    for confidence in [
        ValueConfidence::Confirmed,
        ValueConfidence::Preview,
        ValueConfidence::Unverified,
    ] {
        let f = |b: &mut TileBand| scene(b, &v, Some(overlay(confidence, 77)));
        assert_eq!(tiled(f), full(f), "{confidence:?}");
    }
}

#[test]
fn tiles_stitch_to_the_full_frame_for_recovery() {
    for percent in [0, 1, 50, 99, 100, 255] {
        let f = |b: &mut TileBand| render_recovery(b, percent);
        assert_eq!(tiled(f), full(f), "{percent}");
    }
}

#[test]
fn tiles_stitch_to_the_full_frame_for_updating() {
    assert_eq!(tiled(render_updating), full(render_updating));
}

#[test]
fn tiles_stitch_to_the_full_frame_mid_switch_between_every_pair_of_views() {
    for from in DisplayMode::ALL {
        for to in DisplayMode::ALL {
            for age in [1, 37, 160, 319] {
                let v = switching(from, to, age);
                let f = |b: &mut TileBand| composed(b, &v);
                assert_eq!(tiled(f), full(f), "{from:?} -> {to:?} at {age}");
            }
        }
    }
}

// 2. Determinism ------------------------------------------------------------------------------

#[test]
fn rendering_twice_is_identical() {
    for v in all_views() {
        let f = |b: &mut TileBand| scene(b, &v, None);
        assert_eq!(full(f), full(f));
    }
}

fn diff(a: &[Rgb565], b: &[Rgb565]) -> Vec<(u16, u16)> {
    (0..DIM * DIM)
        .filter(|i| a[usize::from(*i)] != b[usize::from(*i)])
        .map(|i| (i % DIM, i / DIM))
        .collect()
}

fn at(v: &DeskView, elapsed_ms: u32) -> Vec<Rgb565> {
    let v = DeskView { elapsed_ms, ..*v };
    full(|b| scene(b, &v, None))
}

#[test]
fn elapsed_time_moves_only_the_animated_elements() {
    // Nothing animated: any elapsed time gives the same frame, in every full-screen mode.
    for mode in DisplayMode::ALL {
        let mut v = view(mode, |s| {
            s.clock = clock(9, 41, 30);
            s.volume_percent = Some(40);
            s.muted = Some(true);
            s.media = Some(MediaStatus::Paused);
            s.cpu_percent = Some(10);
            s.ram_percent = Some(20);
        });
        v.feedback = feedback(FeedbackKind::StateConfirmed);
        assert_eq!(at(&v, 0), at(&v, 123_457), "{mode:?}");
    }

    // The media note bobs inside its 24 px slot and nowhere else.
    let note = view(DisplayMode::Buddy, |s| s.media = Some(MediaStatus::Playing));
    let moved = diff(&at(&note, 0), &at(&note, 600));
    assert!(!moved.is_empty());
    assert!(moved
        .iter()
        .all(|&(x, y)| (208..232).contains(&x) && (8..34).contains(&y)));
    // One full 1200 ms period later it is back where it started.
    assert_eq!(at(&note, 0), at(&note, 1_200));

    // The processing spinner turns inside the badge and nowhere else.
    let mut spin = view(DisplayMode::Volume, |s| s.volume_percent = Some(40));
    spin.feedback = feedback(FeedbackKind::Processing);
    let moved = diff(&at(&spin, 0), &at(&spin, 100));
    assert!(!moved.is_empty());
    assert!(moved
        .iter()
        .all(|&(x, y)| (10..50).contains(&x) && (8..48).contains(&y)));
}

#[test]
fn the_seconds_ring_follows_the_clock_not_the_device_timer() {
    let with = |second, elapsed_ms| {
        let mut v = view(DisplayMode::Clock, |s| s.clock = clock(9, 41, second));
        v.elapsed_ms = elapsed_ms;
        full(|b| scene(b, &v, None))
    };
    assert_eq!(with(7, 0), with(7, 54_321));
    assert_ne!(with(7, 0), with(8, 0));
    assert_ne!(with(0, 0), with(59, 0));
}

// Transitions -----------------------------------------------------------------------------------

#[test]
fn a_switch_slides_between_the_two_views_and_settles_on_the_plain_view() {
    for (from, to) in [
        (DisplayMode::Clock, DisplayMode::Volume),
        (DisplayMode::Buddy, DisplayMode::Media),
        (DisplayMode::System, DisplayMode::Buddy),
    ] {
        let render = |age| full(|b| composed(b, &switching(from, to, age)));
        let settled = {
            let mut v = switching(from, to, 0);
            v.previous_mode = None;
            full(|b| composed(b, &v))
        };
        let outgoing = {
            let mut v = switching(from, from, 0);
            v.previous_mode = None;
            full(|b| composed(b, &v))
        };
        let mid = render(VIEW_TRANSITION_MS / 3);
        assert_ne!(mid, settled, "{from:?} -> {to:?}");
        assert_ne!(mid, outgoing, "{from:?} -> {to:?}");
        // From the transition's end on, and with no previous view, the plain view is drawn.
        assert_eq!(render(VIEW_TRANSITION_MS), settled);
        assert_eq!(render(VIEW_TRANSITION_MS + 5_000), settled);
    }
}

#[test]
fn the_switch_offset_eases_out_monotonically_and_ignores_a_same_mode_switch() {
    let mut last = -1;
    for age in 0..VIEW_TRANSITION_MS {
        let (prev, shift) = view_switch(&switching(DisplayMode::Clock, DisplayMode::Media, age))
            .expect("animating");
        assert_eq!(prev, DisplayMode::Clock);
        assert!(
            (0..=240).contains(&shift) && shift >= last,
            "{age}: {shift}"
        );
        last = shift;
    }
    // Ease-out: more than half the travel is done in the first third.
    let (_, early) = view_switch(&switching(
        DisplayMode::Clock,
        DisplayMode::Media,
        VIEW_TRANSITION_MS / 3,
    ))
    .unwrap();
    assert!(early > 120, "{early}");
    assert_eq!(
        view_switch(&switching(DisplayMode::Clock, DisplayMode::Clock, 10)),
        None
    );
}

// Media text ------------------------------------------------------------------------------------

fn media_view(title: &str, artist: &str, elapsed_ms: u32) -> DeskView {
    let mut v = view(DisplayMode::Media, |s| s.media = Some(MediaStatus::Paused));
    v.media_info = Some(MediaInfo {
        title: MediaText::from_text(title),
        artist: MediaText::from_text(artist),
    });
    v.elapsed_ms = elapsed_ms;
    v
}

#[test]
fn a_long_title_scrolls_deterministically_and_a_short_one_stays_put() {
    let long = "An unreasonably long track title that cannot fit";
    let frame = |title, ms| full(|b| scene(b, &media_view(title, "Artist", ms), None));
    // Holds still at the start of each pass, then moves with elapsed time only.
    assert_eq!(frame(long, 0), frame(long, 1_000));
    assert_ne!(frame(long, 0), frame(long, 3_000));
    assert_eq!(frame(long, 3_000), frame(long, 3_000));
    let moved = diff(&frame(long, 2_000), &frame(long, 2_500));
    assert!(!moved.is_empty());
    assert!(
        moved.iter().all(|&(_, y)| (130..170).contains(&y)),
        "{moved:?}"
    );
    let f = |b: &mut TileBand| scene(b, &media_view(long, "Artist", 2_345), None);
    assert_eq!(tiled(f), full(f));
    // A title that fits never moves.
    assert_eq!(frame("Short", 0), frame("Short", 7_777));
}

#[test]
fn every_latin1_byte_renders_without_panicking() {
    let bytes: Vec<u8> = (0x20..=0xFFu8).collect();
    for chunk in bytes.chunks(MediaText::CAPACITY) {
        let s: String = chunk.iter().map(|&b| char::from(b)).collect();
        for ms in [0, 2_000, 9_999] {
            let v = media_view(&s, &s, ms);
            let f = |b: &mut TileBand| scene(b, &v, None);
            assert_eq!(tiled(f), full(f));
        }
    }
}

#[test]
fn media_info_unknown_says_so_and_known_info_is_shown() {
    let mut none = view(DisplayMode::Media, |s| s.media = Some(MediaStatus::Playing));
    none.media_info = None;
    let mut some = none;
    some.media_info = Some(MediaInfo {
        title: MediaText::from_text("Title"),
        artist: MediaText::from_text("Artist"),
    });
    assert_ne!(
        full(|b| scene(b, &none, None)),
        full(|b| scene(b, &some, None))
    );
}

#[test]
fn the_cpu_history_draws_a_sparkline() {
    let mut empty = view(DisplayMode::System, |s| s.cpu_percent = Some(40));
    empty.cpu_history = CpuHistory::EMPTY;
    let mut filled = empty;
    for n in 0..60u8 {
        filled.cpu_history.push(n);
    }
    assert_ne!(
        full(|b| scene(b, &empty, None)),
        full(|b| scene(b, &filled, None))
    );
    let f = |b: &mut TileBand| scene(b, &filled, None);
    assert_eq!(tiled(f), full(f));
}

// 3. Distinctness -----------------------------------------------------------------------------

fn badge_crop(kind: FeedbackKind) -> Vec<Rgb565> {
    let mut v = view(DisplayMode::Volume, |s| s.volume_percent = Some(40));
    v.feedback = feedback(kind);
    let frame = full(|b| scene(b, &v, None));
    let mut out = Vec::new();
    for y in 8..48 {
        for x in 8..52 {
            out.push(px(&frame, x, y));
        }
    }
    out
}

#[test]
fn every_feedback_kind_has_its_own_badge() {
    for (i, a) in FEEDBACKS.iter().enumerate() {
        for b in &FEEDBACKS[i + 1..] {
            assert_ne!(badge_crop(*a), badge_crop(*b), "{a:?} vs {b:?}");
        }
    }
}

#[test]
fn unverified_never_looks_like_success() {
    let mut v = view(DisplayMode::Volume, |s| s.volume_percent = Some(40));
    v.feedback = feedback(FeedbackKind::Unverified);
    let frame = full(|b| scene(b, &v, None));
    let baseline = full(|b| {
        let mut v = v;
        v.feedback = None;
        scene(b, &v, None)
    });
    for y in 0..DIM {
        for x in 0..DIM {
            let (r, g, b) = px(&frame, x, y).to_rgb888();
            let green = i32::from(g) > i32::from(r) + 40 && i32::from(g) > i32::from(b) + 40;
            assert!(!green, "green pixel at {x},{y}");
        }
    }
    // A ring, not a disc: the middle of the badge shows the view beneath, and nothing in the
    // badge is white (a check mark would be).
    assert_eq!(px(&frame, 20, 28), px(&baseline, 20, 28));
    for y in 10..47 {
        for x in 12..49 {
            assert_ne!(px(&frame, x, y), Rgb565::WHITE, "white at {x},{y}");
        }
    }
}

#[test]
fn processing_never_shows_a_percentage_or_fill() {
    // Different spinner phases differ, but all share the same ring: no phase is "more done".
    let mut v = view(DisplayMode::Volume, |s| s.volume_percent = Some(40));
    v.feedback = feedback(FeedbackKind::Processing);
    let a = at(&v, 0);
    let b = at(&v, 400);
    assert_ne!(a, b);
    for frame in [&a, &b] {
        // The badge centre is never filled.
        assert_eq!(
            px(frame, 30, 28),
            px(
                &at(
                    &DeskView {
                        feedback: None,
                        ..v
                    },
                    0
                ),
                30,
                28
            )
        );
    }
}

#[test]
fn unknown_values_never_render_as_zero() {
    let render = |mode, edit: &dyn Fn(&mut DeskStatus)| {
        let v = view(mode, |s| edit(s));
        full(|b| scene(b, &v, None))
    };
    assert_ne!(
        render(DisplayMode::System, &|s| s.cpu_percent = None),
        render(DisplayMode::System, &|s| s.cpu_percent = Some(0)),
    );
    assert_ne!(
        render(DisplayMode::System, &|s| s.ram_percent = None),
        render(DisplayMode::System, &|s| s.ram_percent = Some(0)),
    );
    assert_ne!(
        render(DisplayMode::Volume, &|s| s.volume_percent = None),
        render(DisplayMode::Volume, &|s| s.volume_percent = Some(0)),
    );
    assert_ne!(
        render(DisplayMode::Clock, &|s| s.clock = None),
        render(DisplayMode::Clock, &|s| s.clock = clock(0, 0, 0)),
    );
    assert_ne!(
        render(DisplayMode::Media, &|s| s.media = None),
        render(DisplayMode::Media, &|s| s.media =
            Some(MediaStatus::Stopped)),
    );
}

#[test]
fn unknown_mute_is_omitted_and_never_shown_as_unmuted_or_muted() {
    let render = |muted| {
        let v = view(DisplayMode::Volume, |s| {
            s.volume_percent = Some(50);
            s.muted = muted;
        });
        full(|b| scene(b, &v, None))
    };
    assert_eq!(render(None), render(Some(false)));
    assert_ne!(render(None), render(Some(true)));
}

#[test]
fn the_three_media_states_and_unknown_are_distinct() {
    let render = |m| {
        let v = view(DisplayMode::Media, |s| s.media = m);
        full(|b| scene(b, &v, None))
    };
    let frames = [
        render(Some(MediaStatus::Playing)),
        render(Some(MediaStatus::Paused)),
        render(Some(MediaStatus::Stopped)),
        render(None),
    ];
    for (i, a) in frames.iter().enumerate() {
        for b in &frames[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn high_load_marks_the_cpu_bar_and_the_buddy_but_not_other_views() {
    let render = |mode, high_load| {
        let v = view(mode, |s| {
            s.cpu_percent = Some(90);
            s.high_load = high_load;
        });
        full(|b| scene(b, &v, None))
    };
    assert_ne!(
        render(DisplayMode::System, true),
        render(DisplayMode::System, false)
    );
    assert_ne!(
        render(DisplayMode::Buddy, true),
        render(DisplayMode::Buddy, false)
    );
    assert_eq!(
        render(DisplayMode::Clock, true),
        render(DisplayMode::Clock, false)
    );
}

#[test]
fn the_volume_view_follows_overlay_confidence_like_the_overlay_bar() {
    let v = view(DisplayMode::Volume, |s| s.volume_percent = Some(10));
    let render = |o| full(|b| scene(b, &v, o));
    let confirmed = render(Some(overlay(ValueConfidence::Confirmed, 50)));
    let preview = render(Some(overlay(ValueConfidence::Preview, 50)));
    let unverified = render(Some(overlay(ValueConfidence::Unverified, 50)));
    assert_ne!(confirmed, preview);
    assert_ne!(confirmed, unverified);
    assert_ne!(preview, unverified);
    // The overlay wins over the status value, and its Confirmed bar is the status bar.
    let from_status = full(|b| {
        scene(
            b,
            &view(DisplayMode::Volume, |s| s.volume_percent = Some(50)),
            None,
        )
    });
    assert_eq!(confirmed, from_status);
}

// 4. Full-screen views write every pixel -------------------------------------------------------

fn assert_no_sentinel(frame: &[Rgb565], what: &str) {
    if let Some(i) = frame.iter().position(|p| *p == SENT) {
        panic!("{what}: pixel ({}, {}) never written", i % 240, i / 240);
    }
}

#[test]
fn render_mode_writes_every_pixel() {
    for mode in DisplayMode::ALL {
        for status in statuses(mode) {
            let v = DeskView {
                status,
                ..DeskView::default()
            };
            assert_no_sentinel(&full(|b| render_mode(b, &v, None)), &format!("{v:?}"));
            let o = Some(overlay(ValueConfidence::Preview, 33));
            assert_no_sentinel(&full(|b| render_mode(b, &v, o)), &format!("{v:?} overlay"));
        }
    }
}

#[test]
fn render_updating_writes_every_pixel() {
    assert_no_sentinel(&full(render_updating), "updating");
}

#[test]
fn render_recovery_writes_every_pixel() {
    for percent in [0, 1, 50, 99, 100, 255] {
        assert_no_sentinel(
            &full(|b| render_recovery(b, percent)),
            &format!("{percent}"),
        );
    }
}

#[test]
fn recovery_progress_is_real_and_clamped() {
    let frame = |p| full(|b| render_recovery(b, p));
    assert_ne!(frame(0), frame(50));
    assert_ne!(frame(50), frame(100));
    assert_eq!(frame(100), frame(200));
    // The track ring is visible even at 0%: its top differs from the background.
    assert_ne!(px(&frame(0), 120, 100 - 57), px(&frame(0), 5, 5));
}

// 5. Nothing to show draws nothing -------------------------------------------------------------

#[test]
fn chrome_with_nothing_to_show_leaves_the_band_untouched() {
    let mut quiet = Vec::new();
    for mode in DisplayMode::ALL {
        for muted in [None, Some(false)] {
            for media in [None, Some(MediaStatus::Paused), Some(MediaStatus::Stopped)] {
                // High load only has a cue on the buddy.
                let high_load = mode != DisplayMode::Buddy;
                quiet.push(view(mode, |s| {
                    s.muted = muted;
                    s.media = media;
                    s.high_load = high_load;
                    s.volume_percent = Some(50);
                    s.cpu_percent = Some(99);
                    // The status-row time is something to show, except on the Clock view.
                    s.clock = (mode == DisplayMode::Clock).then_some(ClockTime {
                        hour: 1,
                        minute: 2,
                        second: 3,
                    });
                }));
            }
        }
    }
    for v in quiet {
        let frame = full(|b| render_chrome(b, &v));
        assert!(frame.iter().all(|p| *p == SENT), "{v:?}");
    }
}

#[test]
fn the_status_row_shows_a_known_time_except_on_the_clock_view() {
    for mode in DisplayMode::ALL {
        let with = |c| full(|b| render_chrome(b, &view(mode, |s| s.clock = c)));
        let drawn = with(clock(9, 41, 0));
        assert!(with(None).iter().all(|p| *p == SENT));
        let touched: Vec<_> = (0..DIM * DIM)
            .filter(|i| drawn[usize::from(*i)] != SENT)
            .map(|i| (i % DIM, i / DIM))
            .collect();
        if mode == DisplayMode::Clock {
            assert!(touched.is_empty());
        } else {
            assert!(!touched.is_empty());
            // A small centred row clear of the badge and the indicators.
            assert!(touched
                .iter()
                .all(|&(x, y)| (60..180).contains(&x) && y < 32));
        }
    }
}

#[test]
fn chrome_keeps_out_of_the_volume_bar_zone_except_for_the_press_frame() {
    let mut v = view(DisplayMode::Buddy, |s| {
        s.high_load = true;
        s.muted = Some(true);
        s.media = Some(MediaStatus::Playing);
    });
    v.feedback = feedback(FeedbackKind::Error);
    let frame = full(|b| render_chrome(b, &v));
    for y in 180..196 {
        for x in 0..DIM {
            assert_eq!(px(&frame, x, y), SENT, "{x},{y}");
        }
    }
    v.button_down = true;
    let frame = full(|b| render_chrome(b, &v));
    for y in 180..196 {
        for x in 3..DIM - 3 {
            assert_eq!(px(&frame, x, y), SENT, "{x},{y}");
        }
        assert_eq!(px(&frame, 0, y), Rgb565::WHITE);
        assert_eq!(px(&frame, DIM - 1, y), Rgb565::WHITE);
    }
}
