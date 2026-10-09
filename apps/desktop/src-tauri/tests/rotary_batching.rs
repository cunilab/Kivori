//! Issue #18: a burst of detents in one loop pass costs one OS round trip and one `Presentation`,
//! with the same final value the per-detent path produced. Value detents only: a knob bound to
//! shortcuts still emits one keystroke per detent.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kivori_desktop::activity::SessionActivity;
use kivori_desktop::desk::DeskRuntime;
use kivori_desktop::input::Freshness;
use kivori_desktop::platform::system::NoSystemProbe;
use kivori_desktop::platform::{
    ActionAvailability, BackendError, FakeAppVolumeBackend, FakeForeground, FakeInputSynth,
    FakeMediaObserver, FakeVolumeBackend, Foreground, OsServices, VolumeBackend,
};
use kivori_desktop::runtime::device_task::RotaryPipeline;
use kivori_model::input::Direction;
use kivori_model::presentation::ValueConfidence;
use kivori_model::ConnectionState;
use kivori_protocol::message::{ControlId, InputEvent, InputKind};
use kivori_protocol::Presentation;

const SESSION: u32 = 0xBA7C;
/// Stand-in for a blocking Core Audio set + read-back.
const SET_DELAY: Duration = Duration::from_millis(15);

/// A backend whose `set` blocks like the OS does and counts how often it is called.
struct SlowBackend {
    inner: FakeVolumeBackend,
    sets: AtomicUsize,
}

impl SlowBackend {
    fn new(initial: u8) -> Self {
        Self {
            inner: FakeVolumeBackend::new(initial),
            sets: AtomicUsize::new(0),
        }
    }

    fn sets(&self) -> usize {
        self.sets.load(Ordering::SeqCst)
    }
}

impl VolumeBackend for SlowBackend {
    fn availability(&self) -> ActionAvailability {
        self.inner.availability()
    }
    fn read(&self) -> Result<u8, BackendError> {
        self.inner.read()
    }
    fn set(&self, percent: u8) -> Result<u8, BackendError> {
        self.sets.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(SET_DELAY);
        self.inner.set(percent)
    }
    fn read_mute(&self) -> Result<bool, BackendError> {
        self.inner.read_mute()
    }
    fn set_mute(&self, muted: bool) -> Result<bool, BackendError> {
        self.inner.set_mute(muted)
    }
}

fn ev(gesture_id: u16, kind: InputKind) -> InputEvent {
    InputEvent {
        session: SESSION,
        gesture_id,
        control: ControlId::Rotary,
        kind,
        device_ms: 0,
    }
}

fn gesture(gesture_id: u16, detents: &[Direction]) -> Vec<InputEvent> {
    let mut events = vec![ev(gesture_id, InputKind::GestureStarted)];
    events.extend(
        detents
            .iter()
            .map(|d| ev(gesture_id, InputKind::Detent(*d))),
    );
    events
}

fn pipeline(backend: &dyn VolumeBackend) -> RotaryPipeline {
    let mut rotary = RotaryPipeline::new(backend);
    rotary.on_connection_state(
        ConnectionState::Connecting,
        ConnectionState::Connected,
        Some(SESSION),
    );
    rotary
}

/// Every input is a volume input, as in the General profile.
fn feed_volume(
    rotary: &mut RotaryPipeline,
    backend: &dyn VolumeBackend,
    events: &[InputEvent],
) -> Vec<Presentation> {
    let mut presentations = Vec::new();
    rotary.accept_inputs_fresh(
        events,
        Freshness::default(),
        backend,
        &mut presentations,
        |_, _| true,
        |_: SessionActivity| {},
    );
    presentations
}

fn percent(p: &Presentation) -> u8 {
    p.value.expect("a value presentation").current_percent
}

#[test]
fn a_20_detent_burst_is_one_backend_set_and_one_presentation() {
    let backend = SlowBackend::new(10);
    let mut rotary = pipeline(&backend);

    let started = Instant::now();
    let out = feed_volume(&mut rotary, &backend, &gesture(1, &[Direction::Cw; 20]));
    let elapsed = started.elapsed();

    assert_eq!(backend.sets(), 1, "one OS round trip for the whole burst");
    assert_eq!(out.len(), 1, "one presentation for the whole burst");
    assert_eq!(percent(&out[0]), 50, "10 + 20 detents * 2");
    assert_eq!(backend.read().unwrap(), 50);
    assert!(
        elapsed < SET_DELAY * 5,
        "the burst must not pay a round trip per detent: {elapsed:?}"
    );
}

#[test]
fn the_batched_value_matches_per_detent_stepping_at_the_top_bound() {
    let backend = SlowBackend::new(90);
    let mut rotary = pipeline(&backend);
    let out = feed_volume(&mut rotary, &backend, &gesture(1, &[Direction::Cw; 20]));
    assert_eq!(backend.sets(), 1);
    assert_eq!(percent(&out[0]), 100, "clamped, never above 100");
    assert_eq!(backend.read().unwrap(), 100);
}

#[test]
fn the_batched_value_matches_per_detent_stepping_at_the_bottom_bound() {
    let backend = SlowBackend::new(6);
    let mut rotary = pipeline(&backend);
    let out = feed_volume(&mut rotary, &backend, &gesture(1, &[Direction::Ccw; 10]));
    assert_eq!(backend.sets(), 1);
    assert_eq!(percent(&out[0]), 0, "clamped, never below 0");
}

#[test]
fn reversal_inside_a_batch_clamps_per_detent_before_it_nets() {
    // At 98: Cw clamps to 100, then Ccw x3 -> 94. Summing first (+2 -6 = -4 -> 94) would agree
    // here, so also cover a case where it would not: 100 is absorbed, not banked.
    let backend = SlowBackend::new(98);
    let mut rotary = pipeline(&backend);
    let burst = [
        Direction::Cw,
        Direction::Cw,
        Direction::Cw,
        Direction::Ccw,
        Direction::Ccw,
        Direction::Ccw,
    ];
    let out = feed_volume(&mut rotary, &backend, &gesture(1, &burst));
    assert_eq!(backend.sets(), 1);
    assert_eq!(percent(&out[0]), 94, "98 -> 100 (clamp) -> 94, not 98");
}

#[test]
fn a_burst_that_nets_to_nothing_still_confirms_one_value() {
    let backend = SlowBackend::new(50);
    let mut rotary = pipeline(&backend);
    let burst = [Direction::Cw, Direction::Ccw, Direction::Ccw, Direction::Cw];
    let out = feed_volume(&mut rotary, &backend, &gesture(1, &burst));
    assert_eq!(backend.sets(), 1);
    assert_eq!(percent(&out[0]), 50);
}

#[test]
fn a_gesture_end_in_the_same_pass_flushes_first_and_confirms_the_os_value() {
    let backend = SlowBackend::new(20);
    let mut rotary = pipeline(&backend);
    let mut events = gesture(1, &[Direction::Cw; 5]);
    events.push(ev(1, InputKind::GestureEnded));
    let out = feed_volume(&mut rotary, &backend, &events);

    assert_eq!(backend.sets(), 1);
    assert_eq!(out.len(), 2, "the batched preview, then the confirmation");
    assert_eq!(percent(&out[0]), 30);
    assert_eq!(out[0].value.unwrap().confidence, ValueConfidence::Preview);
    assert_eq!(percent(&out[1]), 30);
    assert_eq!(out[1].value.unwrap().confidence, ValueConfidence::Confirmed);
    assert!(
        out[1].revision > out[0].revision,
        "revisions only increase, so the latest supersedes"
    );
}

#[test]
fn bursts_in_separate_passes_each_cost_one_set() {
    let backend = SlowBackend::new(0);
    let mut rotary = pipeline(&backend);
    let first = feed_volume(&mut rotary, &backend, &gesture(1, &[Direction::Cw; 4]));
    let second = feed_volume(
        &mut rotary,
        &backend,
        &[ev(1, InputKind::Detent(Direction::Cw)); 4],
    );
    assert_eq!(backend.sets(), 2);
    assert_eq!(percent(&first[0]), 8);
    assert_eq!(percent(&second[0]), 16);
}

#[test]
fn a_failing_backend_reports_one_failure_for_the_batch() {
    let backend = FakeVolumeBackend::with_availability(ActionAvailability::Unknown);
    let mut rotary = pipeline(&backend);
    let out = feed_volume(&mut rotary, &backend, &gesture(1, &[Direction::Cw; 5]));
    assert_eq!(out.len(), 1);
    assert!(rotary.take_volume_failure());
}

#[test]
fn shortcut_detents_are_never_batched() {
    let fg = Arc::new(FakeForeground::default());
    let synth = Arc::new(FakeInputSynth::new(Ok(())));
    let backend = SlowBackend::new(50);
    let mut desk = DeskRuntime::new(OsServices {
        volume: Arc::new(FakeVolumeBackend::new(20)),
        app_volume: Arc::new(FakeAppVolumeBackend::new()),
        synth: synth.clone(),
        media: Arc::new(FakeMediaObserver::default()),
        system: Box::new(NoSystemProbe),
        clock: || None,
        foreground: fg.clone(),
    });
    desk.on_session_begin();
    let _ = desk.tick(Duration::ZERO, &mut |_| {});
    // The browser binds the knob to tab shortcuts.
    *fg.0.lock().unwrap() = Foreground::App {
        id: "chrome.exe".into(),
        name: "chrome.exe".into(),
    };
    let _ = desk.tick(Duration::from_millis(500), &mut |_| {});

    let mut rotary = pipeline(&backend);
    let mut presentations = Vec::new();
    let now = Duration::from_millis(600);
    rotary.accept_inputs_fresh(
        &gesture(1, &[Direction::Cw; 3]),
        Freshness::default(),
        &backend,
        &mut presentations,
        |input, remaining| desk.on_input_within(&input, now, remaining, &mut |_| {}),
        |_| {},
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    while synth.sent.lock().unwrap().len() < 3 && Instant::now() < deadline {
        let _ = desk.tick(now, &mut |_| {});
        std::thread::sleep(Duration::from_millis(5));
    }
    let sent = synth.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 3, "one keystroke per detent: {sent:?}");
    assert_eq!(backend.sets(), 0, "the volume backend is never touched");
    assert!(presentations.is_empty());
}
