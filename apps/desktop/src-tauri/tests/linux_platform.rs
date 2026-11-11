//! Linux backends against the real machine. Read-only tests run by default and skip (loudly) what
//! the machine lacks, such as an audio server or a session bus; tests that change the user's
//! volume or microphone mute are `#[ignore]` and restore the original state via a Drop guard.
//!
//! Run the opt-in ones with `cargo test -p kivori-desktop --test linux_platform -- --ignored`.
#![cfg(target_os = "linux")]

use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use kivori_desktop::platform::host_events::HostEventsError;
use kivori_desktop::platform::linux::{
    local_time, session_kind, LinuxForeground, LinuxMediaObserver, LinuxMicBackend,
    LinuxSystemProbe, LinuxVolumeBackend, Mpris, SessionKind,
};
use kivori_desktop::platform::system::{cpu_percent, SystemProbe};
use kivori_desktop::platform::{
    mics_muted, ActionAvailability, AppVolumeBackend, AppVolumeError, ChangeOrigin,
    ConfirmationClass, Foreground, ForegroundObserver, MediaObserver, MicBackend, MicMute,
    VolumeBackend, VolumeChange,
};
use std::sync::Arc;

#[test]
fn volume_backend_reads_the_default_output_or_reports_runtime_unavailable() {
    let backend = LinuxVolumeBackend::new();
    match VolumeBackend::availability(&backend) {
        ActionAvailability::Available { confirmation } => {
            assert_eq!(confirmation, ConfirmationClass::StateConfirmed);
            let percent = VolumeBackend::read(&backend).expect("read volume");
            assert!(percent <= 100);
            let _muted = VolumeBackend::read_mute(&backend).expect("read mute");
            eprintln!("master volume {percent} %");
        }
        ActionAvailability::RuntimeUnavailable { reason } => {
            eprintln!("no usable output device: {reason}");
            assert!(VolumeBackend::read(&backend).is_err());
        }
        other => panic!("Linux is implemented; got {other:?}"),
    }
}

/// An app with no audio stream is a `NoSession`, never an error and never the master volume.
#[test]
fn an_app_with_no_stream_has_no_session() {
    let backend = LinuxVolumeBackend::new();
    if !matches!(
        VolumeBackend::availability(&backend),
        ActionAvailability::Available { .. }
    ) {
        eprintln!("skipped: no audio server");
        return;
    }
    let app = "kivori-no-such-app";
    assert_eq!(
        AppVolumeBackend::read(&backend, app),
        Err(AppVolumeError::NoSession)
    );
    assert_eq!(
        AppVolumeBackend::set(&backend, app, 50),
        Err(AppVolumeError::NoSession)
    );
    assert_eq!(
        AppVolumeBackend::set_mute(&backend, app, true),
        Err(AppVolumeError::NoSession)
    );
}

/// Read-only: lists the capture devices and reads their mute and use. It never writes a mute.
#[test]
fn mic_backend_lists_capture_devices_and_never_claims_muted_without_reading_it() {
    let backend = LinuxMicBackend::new();
    let devices = match backend.devices() {
        Ok(devices) => devices,
        Err(error) => {
            eprintln!("skipped: {error:?}");
            return;
        }
    };
    for device in &devices {
        assert!(!device.name.is_empty());
        assert!(
            !device.name.ends_with(".monitor"),
            "a monitor is not a microphone"
        );
    }
    eprintln!(
        "{} microphone(s), in use: {:?}",
        devices.len(),
        backend.in_use()
    );
    match backend.availability() {
        ActionAvailability::Available { .. } => assert!(!devices.is_empty()),
        ActionAvailability::RuntimeUnavailable { .. } => assert!(devices.is_empty()),
        other => panic!("Linux is implemented; got {other:?}"),
    }
    // An unmuted microphone must never read as muted.
    if devices.iter().any(|d| d.mute == MicMute::Live) {
        assert_eq!(mics_muted(&devices), Some(false));
    }
}

#[test]
fn cpu_is_a_percentage_between_two_samples() {
    let probe = LinuxSystemProbe;
    let a = probe.cpu_times().expect("first sample");
    sleep(Duration::from_millis(250));
    let b = probe.cpu_times().expect("second sample");
    let pct = cpu_percent(a, b).unwrap_or_else(|| panic!("ticks did not advance: {a:?} -> {b:?}"));
    assert!(pct <= 100, "{pct}");
    eprintln!("cpu {pct} %");
}

#[test]
fn ram_is_a_plausible_percentage() {
    let pct = LinuxSystemProbe.ram_percent().expect("ram");
    assert!((1..=100).contains(&pct), "{pct}");
    eprintln!("ram {pct} %");
}

#[test]
fn local_time_matches_date_within_a_minute() {
    let out = Command::new("date")
        .arg("+%H:%M")
        .output()
        .expect("run date");
    let ours = local_time().expect("local time");
    let text = String::from_utf8(out.stdout).unwrap();
    let (h, m) = text.trim().split_once(':').unwrap();
    let theirs = h.parse::<i32>().unwrap() * 60 + m.parse::<i32>().unwrap();
    let mine = i32::from(ours.hour) * 60 + i32::from(ours.minute);
    let diff = (theirs - mine).rem_euclid(24 * 60);
    assert!(
        diff <= 1 || diff == 24 * 60 - 1,
        "date={text} ours={ours:?}"
    );
}

/// Live smoke test: whatever is (or is not) playing, reads never block and drop is prompt.
/// Prints only the status and whether a title is known (never the title: ADR-0005).
#[test]
fn media_observer_reads_without_blocking_and_stops_promptly() {
    let observer = LinuxMediaObserver::new(Arc::new(Mpris::new()));
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut status = None;
    while Instant::now() < deadline {
        let t = Instant::now();
        status = observer.status();
        let now = observer.now_playing();
        assert!(
            t.elapsed() < Duration::from_millis(50),
            "reads must not block"
        );
        if status.is_some() {
            eprintln!("media: status={status:?} title_known={}", now.is_some());
            break;
        }
        sleep(Duration::from_millis(100));
    }
    eprintln!("media after <=3 s: status={status:?}");
    let t = Instant::now();
    drop(observer);
    assert!(
        t.elapsed() < Duration::from_millis(1500),
        "drop took {:?}",
        t.elapsed()
    );
}

/// The honest answer per session: X11 reads the focused app (or says unknown), anything else is
/// unknown, never a stale guess.
#[test]
fn foreground_is_unknown_on_wayland_and_never_a_stale_guess() {
    let seen = LinuxForeground::new().foreground();
    eprintln!("session {:?}: foreground {seen:?}", session_kind());
    match session_kind() {
        SessionKind::X11 => {
            if let Foreground::App { id, .. } = &seen {
                assert_eq!(id, &id.to_lowercase());
            }
        }
        SessionKind::Wayland | SessionKind::Headless => assert_eq!(seen, Foreground::Unknown),
    }
}

/// Shortcut availability is the honest per-session answer: Wayland and headless refuse, and X11 says
/// `Available` only when the X server is reachable with XTEST (never from `DISPLAY` alone).
#[test]
fn shortcut_availability_is_honest_for_this_session() {
    let synth = kivori_desktop::platform::linux::LinuxInputSynth::new(Arc::new(Mpris::new()));
    let availability = kivori_desktop::platform::InputSynth::keyboard_availability(&synth)
        .expect("Linux reports its own");
    eprintln!("session {:?}: shortcuts {availability:?}", session_kind());
    match session_kind() {
        SessionKind::Wayland | SessionKind::Headless => {
            assert!(matches!(
                availability,
                ActionAvailability::RuntimeUnavailable { .. }
            ));
        }
        SessionKind::X11 => assert!(matches!(
            availability,
            ActionAvailability::Available { .. } | ActionAvailability::RuntimeUnavailable { .. }
        )),
    }
}

/// The logind source starts (or says why not) and stops promptly when its guard is dropped.
#[test]
fn host_events_start_and_stop_promptly() {
    let (tx, _rx) = std::sync::mpsc::channel();
    match kivori_desktop::platform::start_host_events(tx) {
        Ok(guard) => {
            let t = Instant::now();
            drop(guard);
            assert!(
                t.elapsed() < Duration::from_secs(2),
                "stop took {:?}",
                t.elapsed()
            );
        }
        Err(HostEventsError::Unavailable(reason)) => eprintln!("skipped: {reason}"),
    }
}

/// Restores the original volume and mute on drop, including on panic.
struct Restore<'a> {
    backend: &'a LinuxVolumeBackend,
    volume: u8,
    muted: bool,
}

impl<'a> Restore<'a> {
    fn capture(backend: &'a LinuxVolumeBackend) -> Self {
        Self {
            backend,
            volume: VolumeBackend::read(backend).expect("read volume"),
            muted: VolumeBackend::read_mute(backend).expect("read mute"),
        }
    }
}

impl Drop for Restore<'_> {
    fn drop(&mut self) {
        let _ = VolumeBackend::set(self.backend, self.volume);
        let _ = VolumeBackend::set_mute(self.backend, self.muted);
        // WirePlumber is the fallback: it can round an otherwise exact native restoration.
        if VolumeBackend::read(self.backend) != Ok(self.volume)
            || VolumeBackend::read_mute(self.backend) != Ok(self.muted)
        {
            let _ = Command::new("wpctl")
                .args([
                    "set-volume",
                    "@DEFAULT_AUDIO_SINK@",
                    &format!("{:.2}", f32::from(self.volume) / 100.0),
                ])
                .status();
            let _ = Command::new("wpctl")
                .args([
                    "set-mute",
                    "@DEFAULT_AUDIO_SINK@",
                    if self.muted { "1" } else { "0" },
                ])
                .status();
        }
    }
}

fn target(original: u8) -> u8 {
    if original >= 50 {
        original - 10
    } else {
        original + 10
    }
}

/// Collects changes for up to `timeout`, stopping early once `done` is satisfied.
fn collect(
    backend: &LinuxVolumeBackend,
    timeout: Duration,
    done: impl Fn(&[VolumeChange]) -> bool,
) -> Vec<VolumeChange> {
    let deadline = Instant::now() + timeout;
    let mut seen = Vec::new();
    while Instant::now() < deadline && !done(&seen) {
        if let Some(c) = VolumeBackend::try_recv_change(backend) {
            seen.push(c);
        } else {
            sleep(Duration::from_millis(10));
        }
    }
    seen
}

#[test]
#[ignore = "changes the real system volume and mute (restored afterwards)"]
fn write_round_trips_and_classifies_echo_versus_external() {
    let backend = LinuxVolumeBackend::new();
    assert!(matches!(
        VolumeBackend::availability(&backend),
        ActionAvailability::Available { .. }
    ));
    let guard = Restore::capture(&backend);
    eprintln!("original: volume={} muted={}", guard.volume, guard.muted);

    // The initial bind publishes EndpointRebind; drain it.
    let initial = collect(&backend, Duration::from_millis(300), |s| !s.is_empty());
    eprintln!("initial: {initial:?}");
    assert!(initial
        .iter()
        .any(|c| c.origin == ChangeOrigin::EndpointRebind));

    // Volume: original +/- 10 and back, read-back must equal.
    let t = target(guard.volume);
    assert_eq!(VolumeBackend::set(&backend, t), Ok(t));
    assert_eq!(VolumeBackend::read(&backend), Ok(t));
    let echo = collect(&backend, Duration::from_millis(500), |s| !s.is_empty());
    eprintln!("after own set({t}): {echo:?}");
    assert!(
        echo.iter().all(|c| c.origin == ChangeOrigin::Kivori),
        "own write must echo as Kivori or not at all: {echo:?}"
    );

    // External change through WirePlumber.
    let ext = target(t);
    let ok = Command::new("wpctl")
        .args([
            "set-volume",
            "@DEFAULT_AUDIO_SINK@",
            &format!("{:.2}", f32::from(ext) / 100.0),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        let seen = collect(&backend, Duration::from_secs(2), |s| {
            s.iter().any(|c| c.origin == ChangeOrigin::External)
        });
        eprintln!("after wpctl {ext}: {seen:?}");
        let external = seen
            .iter()
            .find(|c| c.origin == ChangeOrigin::External)
            .expect("external change observed");
        assert!(external.percent.abs_diff(ext) <= 1, "{external:?}");
    } else {
        eprintln!("wpctl unavailable: external-change classification not exercised");
    }

    // Mute: toggle and back.
    assert_eq!(VolumeBackend::set(&backend, t), Ok(t));
    let muted_before = VolumeBackend::read_mute(&backend).expect("read mute");
    let _ = collect(&backend, Duration::from_millis(400), |_| false);
    assert_eq!(
        VolumeBackend::set_mute(&backend, !muted_before),
        Ok(!muted_before)
    );
    assert_eq!(VolumeBackend::read_mute(&backend), Ok(!muted_before));
    let echo = collect(&backend, Duration::from_millis(500), |s| {
        s.iter().any(|c| c.muted == Some(!muted_before))
    });
    eprintln!("after own set_mute({}): {echo:?}", !muted_before);
    assert!(
        echo.iter().any(|c| c.muted == Some(!muted_before)),
        "the mute toggle is observed: {echo:?}"
    );
    assert!(
        echo.iter().all(|c| c.origin == ChangeOrigin::Kivori),
        "{echo:?}"
    );
    assert_eq!(
        VolumeBackend::set_mute(&backend, muted_before),
        Ok(muted_before)
    );

    let original = (guard.volume, guard.muted);
    drop(guard);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let restored = (
            VolumeBackend::read(&backend),
            VolumeBackend::read_mute(&backend),
        );
        if restored == (Ok(original.0), Ok(original.1)) {
            eprintln!("restored: volume={} muted={}", original.0, original.1);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "original audio state was not restored: {restored:?}"
        );
        sleep(Duration::from_millis(10));
    }
}

/// Per-app volume on a stream that exists right now: set to one value, every stream of the app
/// agrees, then restore. Names the app with `KIVORI_TEST_APP` (default `brave`); with no stream it
/// reports so and passes.
#[test]
#[ignore = "changes one app's real volume (restored afterwards)"]
fn app_volume_round_trips_on_a_live_stream() {
    let backend = LinuxVolumeBackend::new();
    let app = std::env::var("KIVORI_TEST_APP").unwrap_or_else(|_| "brave".to_string());
    let Ok(volume) = AppVolumeBackend::read(&backend, &app) else {
        eprintln!("no audio stream for {app:?}: nothing to round-trip");
        return;
    };
    let muted = AppVolumeBackend::read_mute(&backend, &app).expect("read app mute");
    eprintln!("{app}: volume {volume} %, muted {muted}");
    let t = target(volume);
    let after = AppVolumeBackend::set(&backend, &app, t);
    let after_mute = AppVolumeBackend::set_mute(&backend, &app, !muted);
    // Restore first, assert after, so a failure cannot leave the app changed.
    let restored = (
        AppVolumeBackend::set(&backend, &app, volume),
        AppVolumeBackend::set_mute(&backend, &app, muted),
    );
    assert_eq!(after, Ok(t));
    assert_eq!(after_mute, Ok(!muted));
    assert_eq!(restored, (Ok(volume), Ok(muted)));
}

/// Mutes every microphone, reads each back, then unmutes them. The backend has no per-device API,
/// by design, so it only runs when every microphone starts live: then "unmute all" restores the
/// exact original state.
#[test]
#[ignore = "changes the real microphone mute (restored afterwards)"]
fn mic_mute_round_trips_on_every_capture_device() {
    let backend = LinuxMicBackend::new();
    let before = backend.devices().expect("list microphones");
    if before.is_empty() {
        eprintln!("no microphone: nothing to round-trip");
        return;
    }
    if before.iter().any(|d| d.mute != MicMute::Live) {
        eprintln!("a microphone is already muted: not changing what cannot be restored exactly");
        return;
    }
    let muted = backend.set_mute(true);
    let live = backend.set_mute(false);
    let muted = muted.expect("mute all");
    assert!(muted.iter().all(|d| d.mute == MicMute::Muted), "{muted:?}");
    assert_eq!(mics_muted(&muted), Some(true));
    let live = live.expect("unmute all");
    assert!(live.iter().all(|d| d.mute == MicMute::Live), "{live:?}");
    assert_eq!(backend.devices().expect("list again"), before);
}
