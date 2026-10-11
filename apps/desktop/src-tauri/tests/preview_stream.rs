//! Preview-stream tests (T053; contracts/ipc.md §3) + the per-frame-invoke performance check that
//! justifies keeping `render_preview_frame` as the scrub/step path. No webview required: the registry
//! and its control primitives are plain `std` types.
#![cfg(feature = "device-studio")]

use std::time::Instant;

use kivori_desktop::ipc::channels::{
    step_ms, PreviewStreams, StreamControl, MAX_FPS, MAX_IN_FLIGHT, MIN_FPS,
};
use kivori_desktop::render::render_preview_bundled;
use kivori_model::{frame_step_ms, CompanionState};

#[test]
fn live_updates_keep_only_the_latest_timestamp() {
    use kivori_desktop::render::animation::AnimationTimeline;
    let control = StreamControl::default();
    for ms in [33, 66, 100] {
        control.update(
            AnimationTimeline {
                initial_state: "idle".into(),
                events: vec![],
                action_events: vec![],
                idle_life: None,
            },
            ms,
        );
    }
    assert_eq!(control.take_request().unwrap().1, 100);
    assert!(control.take_request().is_none());
    control.update(
        AnimationTimeline {
            initial_state: "idle".into(),
            events: vec![],
            action_events: vec![],
            idle_life: None,
        },
        133,
    );
    let (_, _, revision) = control.take_request().unwrap();
    assert!(control.is_current(revision));
    control.update(
        AnimationTimeline {
            initial_state: "idle".into(),
            events: vec![],
            action_events: vec![],
            idle_life: None,
        },
        166,
    );
    assert!(
        !control.is_current(revision),
        "superseded frame must not be sent"
    );
}

#[test]
fn step_ms_matches_the_canonical_timeline_at_30fps() {
    // The stream's clock must be the same integer timeline the goldens use (Principle III).
    for step in [0u64, 1, 2, 29, 30, 31, 100, 12_345] {
        assert_eq!(
            step_ms(step, 30),
            frame_step_ms(step as i64),
            "step {step} must match frame_step_ms"
        );
    }
}

#[test]
fn step_ms_is_drift_free_over_a_long_stream() {
    // 30 steps advance exactly one second, forever — no accumulation.
    for minute in 0..60u64 {
        let base = minute * 1800; // 1800 steps = 60 s at 30 fps
        assert_eq!(step_ms(base + 30, 30) - step_ms(base, 30), 1000);
    }
    // A given step always yields the same timestamp (pure function of the index).
    assert_eq!(step_ms(4242, 30), step_ms(4242, 30));
    assert_eq!(step_ms(0, 30), 0);
}

#[test]
fn step_ms_clamps_the_requested_rate() {
    assert_eq!(step_ms(5, 0), step_ms(5, MIN_FPS), "0 fps clamps up to MIN");
    assert_eq!(
        step_ms(5, 9_999),
        step_ms(5, MAX_FPS),
        "absurd fps clamps down to MAX"
    );
    // At 1 fps each step is a whole second.
    assert_eq!(step_ms(7, 1), 7000);
}

#[test]
fn back_pressure_bounds_frames_in_flight() {
    let control = StreamControl::default();
    for expected in 1..=MAX_IN_FLIGHT {
        assert!(control.try_reserve(), "slot {expected} available");
        assert_eq!(control.in_flight(), expected);
    }
    // Saturated: the producer must stall rather than queue.
    assert!(!control.try_reserve(), "no slot beyond MAX_IN_FLIGHT");
    assert_eq!(control.in_flight(), MAX_IN_FLIGHT);

    // An ack frees exactly one slot.
    control.release();
    assert_eq!(control.in_flight(), MAX_IN_FLIGHT - 1);
    assert!(control.try_reserve());
    assert_eq!(control.in_flight(), MAX_IN_FLIGHT);
}

#[test]
fn duplicate_acks_cannot_underflow_the_slot_count() {
    let control = StreamControl::default();
    control.release();
    control.release();
    assert_eq!(control.in_flight(), 0, "saturating release");
    assert!(control.try_reserve());
    assert_eq!(control.in_flight(), 1);
}

#[test]
fn cancel_marks_the_stream_and_drops_the_registry_entry() {
    let streams = PreviewStreams::new();
    assert!(streams.is_empty());
    let control = streams.register(9);
    assert_eq!(streams.len(), 1);
    assert!(!control.is_cancelled());

    assert!(streams.cancel(9), "registered stream cancels");
    assert!(control.is_cancelled(), "producer sees cancellation");
    assert!(streams.is_empty(), "entry removed");
    assert!(
        !streams.cancel(9),
        "closing an unknown handle is not an error"
    );
}

#[test]
fn cancel_all_stops_every_stream_on_teardown() {
    let streams = PreviewStreams::new();
    let controls: Vec<_> = (1..=3).map(|id| streams.register(id)).collect();
    assert_eq!(streams.len(), 3);

    streams.cancel_all();

    assert!(streams.is_empty(), "registry drained");
    assert!(
        controls.iter().all(|c| c.is_cancelled()),
        "every producer told to stop"
    );
}

#[test]
fn ack_only_applies_to_registered_streams() {
    let streams = PreviewStreams::new();
    let control = streams.register(4);
    assert!(control.try_reserve());
    assert!(streams.ack(4), "known handle acked");
    assert_eq!(control.in_flight(), 0);
    assert!(!streams.ack(77), "unknown handle rejected");
}

#[test]
fn producer_cleanup_removes_the_entry() {
    // Mirrors the producer's exit path: remove(id) after the loop.
    let streams = PreviewStreams::new();
    streams.register(5);
    streams.remove(5);
    assert!(streams.is_empty());
    assert!(streams.control(5).is_none());
}

#[test]
fn per_frame_render_fits_the_30fps_budget() {
    // Verifies the premise of T053's decision: a single preview frame renders far inside the 33 ms
    // frame budget, so per-frame `render_preview_frame` invokes are adequate for scrub/step, and the
    // channel exists for sustained play rather than to fix a per-frame cost problem.
    let _warm = render_preview_bundled(CompanionState::Idle, 0);

    let frames = 30u32;
    let start = Instant::now();
    for step in 0..frames {
        let rgba = render_preview_bundled(CompanionState::Happy, step_ms(u64::from(step), 30));
        assert_eq!(rgba.len(), 240 * 240 * 4);
    }
    let per_frame = start.elapsed() / frames;
    assert!(
        per_frame.as_millis() < 33,
        "one preview frame must fit the 30fps budget, took {per_frame:?}"
    );
}
