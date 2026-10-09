//! Issue #26: same-session input freshness. A discrete action older than the limit when the
//! desktop is about to run it is dropped and logged; detents and gesture boundaries always get
//! through. Deterministic: device and host times are injected, no wall clock decides a result.

use std::sync::Arc;
use std::time::Duration;

use kivori_desktop::activity::{ActivityEventKind, SessionActivity};
use kivori_desktop::desk::DeskRuntime;
use kivori_desktop::input::{DeviceClock, Freshness, LogicalInput};
use kivori_desktop::platform::system::NoSystemProbe;
use kivori_desktop::platform::{
    FakeForeground, FakeInputSynth, FakeMediaObserver, FakeVolumeBackend, Foreground, OsServices,
};
use kivori_desktop::runtime::device_task::RotaryPipeline;
use kivori_model::input::Direction;
use kivori_model::ConnectionState;
use kivori_protocol::message::{ControlId, InputEvent, InputKind};

const SESSION: u32 = 0x5EED;
/// Host time of the burst's arrival; the device clock is 10_000 ms ahead of it.
const HOST_NOW: u32 = 5_000;
const OFFSET: u32 = 10_000;

fn clock() -> DeviceClock {
    let mut clock = DeviceClock::default();
    // A Pong answered instantly: device uptime 10_000 at host 0.
    clock.on_pong(0, 0, OFFSET);
    clock
}

fn freshness() -> Freshness {
    Freshness {
        clock: clock(),
        host_now_ms: HOST_NOW,
    }
}

/// An event the device stamped `age_ms` before the burst arrived.
fn ev(gesture_id: u16, control: ControlId, kind: InputKind, age_ms: u32) -> InputEvent {
    InputEvent {
        session: SESSION,
        gesture_id,
        control,
        kind,
        device_ms: (HOST_NOW + OFFSET).wrapping_sub(age_ms),
    }
}

fn press(gesture_id: u16, age_ms: u32) -> InputEvent {
    ev(gesture_id, ControlId::Button, InputKind::Press, age_ms)
}

fn pipeline(backend: &FakeVolumeBackend) -> RotaryPipeline {
    let mut rotary = RotaryPipeline::new(backend);
    rotary.on_connection_state(
        ConnectionState::Connecting,
        ConnectionState::Connected,
        Some(SESSION),
    );
    rotary
}

/// Runs `events` through the pipeline, returning what reached the desk and what was logged.
fn run(
    rotary: &mut RotaryPipeline,
    backend: &FakeVolumeBackend,
    freshness: Freshness,
    events: &[InputEvent],
) -> (Vec<LogicalInput>, Vec<ActivityEventKind>) {
    let mut delivered = Vec::new();
    let mut logged = Vec::new();
    rotary.accept_inputs_fresh(
        events,
        freshness,
        backend,
        &mut Vec::new(),
        |input, _| {
            delivered.push(input);
            false
        },
        |o: SessionActivity| logged.push(o.kind),
    );
    (delivered, logged)
}

#[test]
fn a_backlog_drops_old_presses_and_runs_fresh_ones() {
    let backend = FakeVolumeBackend::new(50);
    let mut rotary = pipeline(&backend);
    let (delivered, logged) = run(
        &mut rotary,
        &backend,
        freshness(),
        &[
            press(1, 2_000),
            ev(2, ControlId::Button, InputKind::Hold, 1_000),
            ev(3, ControlId::ContextButton(0), InputKind::Press, 751),
            ev(4, ControlId::ContextButton(1), InputKind::Hold, 900),
            ev(5, ControlId::Button, InputKind::DoublePress, 5_000),
            // Fresh: exactly at the limit, and recent.
            press(6, 750),
            ev(7, ControlId::ContextButton(2), InputKind::Press, 10),
        ],
    );
    assert_eq!(
        logged,
        vec![ActivityEventKind::InputStale; 5],
        "five stale actions logged, no payload"
    );
    assert_eq!(
        delivered,
        [
            LogicalInput::Press { gesture_id: 6 },
            LogicalInput::ButtonPress {
                button: 2,
                gesture_id: 7
            },
        ]
    );
}

#[test]
fn rotation_in_the_same_backlog_is_always_delivered_in_order() {
    let backend = FakeVolumeBackend::new(50);
    let mut rotary = pipeline(&backend);
    let mut events = vec![ev(1, ControlId::Rotary, InputKind::GestureStarted, 4_000)];
    for age in (0..20).map(|i| 3_900 - i * 100) {
        events.push(ev(
            1,
            ControlId::Rotary,
            InputKind::Detent(Direction::Cw),
            age,
        ));
    }
    events.push(press(2, 3_000));
    events.push(ev(1, ControlId::Rotary, InputKind::GestureEnded, 1_900));

    let (delivered, logged) = run(&mut rotary, &backend, freshness(), &events);

    assert_eq!(logged, [ActivityEventKind::InputStale], "only the press");
    assert_eq!(delivered.len(), 22);
    assert_eq!(delivered[0], LogicalInput::GestureStarted { gesture_id: 1 });
    assert!(delivered[1..21]
        .iter()
        .all(|i| matches!(i, LogicalInput::Detent { gesture_id: 1, .. })));
    assert_eq!(delivered[21], LogicalInput::GestureEnded { gesture_id: 1 });
}

#[test]
fn the_device_clock_wrapping_does_not_hide_or_invent_staleness() {
    let backend = FakeVolumeBackend::new(50);
    let mut rotary = pipeline(&backend);
    // The device clock reads 100 ms before its wrap at host 0.
    let mut wrapped = DeviceClock::default();
    wrapped.on_pong(0, 0, u32::MAX - 99);
    let at = Freshness {
        clock: wrapped,
        host_now_ms: 1_000,
    };
    // Host 1_000 is device 900 (after the wrap, +1 for the zero tick).
    let stamp = |age: u32| (u32::MAX - 99).wrapping_add(1_000).wrapping_sub(age);
    let mk = |gesture_id, age| InputEvent {
        device_ms: stamp(age),
        ..press(gesture_id, 0)
    };
    let (delivered, logged) = run(&mut rotary, &backend, at, &[mk(1, 900), mk(2, 100)]);
    assert_eq!(logged, [ActivityEventKind::InputStale]);
    assert_eq!(delivered, [LogicalInput::Press { gesture_id: 2 }]);
}

#[test]
fn without_a_pong_nothing_is_dropped() {
    let backend = FakeVolumeBackend::new(50);
    let mut rotary = pipeline(&backend);
    let unknown = Freshness {
        clock: DeviceClock::default(),
        host_now_ms: HOST_NOW,
    };
    let (delivered, logged) = run(
        &mut rotary,
        &backend,
        unknown,
        &[press(1, 60_000), press(2, 0)],
    );
    assert!(logged.is_empty());
    assert_eq!(delivered.len(), 2);
}

#[test]
fn session_and_gesture_checks_still_come_first() {
    let backend = FakeVolumeBackend::new(50);
    let mut rotary = pipeline(&backend);
    let other_session = InputEvent {
        session: SESSION + 1,
        ..press(1, 5_000)
    };
    let orphan_detent = ev(
        9,
        ControlId::Rotary,
        InputKind::Detent(Direction::Cw),
        5_000,
    );
    let (delivered, logged) = run(
        &mut rotary,
        &backend,
        freshness(),
        &[other_session, orphan_detent],
    );
    assert!(delivered.is_empty());
    assert_eq!(
        logged,
        [
            ActivityEventKind::InputStaleSessionRejected,
            ActivityEventKind::InputUnstartedGestureRejected
        ]
    );
}

fn desk(foreground: &Arc<FakeForeground>, synth: &Arc<FakeInputSynth>) -> DeskRuntime {
    let mut desk = DeskRuntime::new(OsServices {
        volume: Arc::new(FakeVolumeBackend::new(20)),
        synth: synth.clone(),
        media: Arc::new(FakeMediaObserver::default()),
        system: Box::new(NoSystemProbe),
        clock: || None,
        foreground: foreground.clone(),
    });
    desk.on_session_begin();
    desk
}

fn settle(desk: &mut DeskRuntime, at: Duration, wanted: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut seen = 0;
    while seen < wanted && std::time::Instant::now() < deadline {
        seen += desk.tick(at, &mut |_| {}).feedback.len();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Drives a backlog through the real pipeline and desk, as the device task does.
fn run_on_desk(
    desk: &mut DeskRuntime,
    events: &[InputEvent],
    now: Duration,
) -> Vec<ActivityEventKind> {
    let backend = FakeVolumeBackend::new(50);
    let mut rotary = pipeline(&backend);
    let mut logged = Vec::new();
    rotary.accept_inputs_fresh(
        events,
        freshness(),
        &backend,
        &mut Vec::new(),
        |input, remaining| desk.on_input_within(&input, now, remaining, &mut |_| {}),
        |o| logged.push(o.kind),
    );
    logged
}

#[test]
fn a_stale_press_does_not_run_in_the_app_that_took_focus_meanwhile() {
    let fg = Arc::new(FakeForeground::default());
    let synth = Arc::new(FakeInputSynth::new(Ok(())));
    let mut desk = desk(&fg, &synth);
    let _ = desk.tick(Duration::ZERO, &mut |_| {});
    // The user pressed the middle contextual button (Play/Pause in General) 2 s ago; since
    // then the browser took focus, where that button is Reload. Only the fresh press runs.
    *fg.0.lock().unwrap() = Foreground::App {
        id: "chrome.exe".into(),
        name: "chrome.exe".into(),
    };
    let logged = run_on_desk(
        &mut desk,
        &[
            ev(1, ControlId::ContextButton(1), InputKind::Press, 2_000),
            ev(2, ControlId::ContextButton(0), InputKind::Press, 100),
        ],
        Duration::from_millis(50),
    );
    settle(&mut desk, Duration::from_millis(60), 1);
    assert_eq!(logged, [ActivityEventKind::InputStale]);
    let sent = synth.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "only the fresh press ran: {sent:?}");
    assert!(
        !sent.iter().any(|s| s.ends_with('R')),
        "the stale press never became Reload in the browser: {sent:?}"
    );
}

#[test]
fn a_stale_shortcut_does_not_run_once_the_context_turned_protected() {
    let fg = Arc::new(FakeForeground::default());
    let synth = Arc::new(FakeInputSynth::new(Ok(())));
    let mut desk = desk(&fg, &synth);
    let _ = desk.tick(Duration::ZERO, &mut |_| {});
    *fg.0.lock().unwrap() = Foreground::App {
        id: "chrome.exe".into(),
        name: "chrome.exe".into(),
    };
    let _ = desk.tick(Duration::from_millis(500), &mut |_| {});
    *fg.0.lock().unwrap() = Foreground::Protected;
    let logged = run_on_desk(
        &mut desk,
        &[ev(1, ControlId::ContextButton(1), InputKind::Press, 3_000)],
        Duration::from_millis(600),
    );
    assert_eq!(logged, [ActivityEventKind::InputStale]);
    // Dropped before the desk: not even the "can't run here" error is raised for it.
    let out = desk.tick(Duration::from_millis(610), &mut |_| {});
    assert!(out.feedback.is_empty());
    assert!(synth.sent.lock().unwrap().is_empty());
}
