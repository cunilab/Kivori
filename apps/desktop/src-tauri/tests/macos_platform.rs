//! macOS backends against the real machine. Read-only tests run by default; tests that change
//! the user's volume/mute are `#[ignore]` and restore the original state via a Drop guard.
#![cfg(target_os = "macos")]

use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use kivori_desktop::platform::macos::{local_time, MacSystemProbe, MacVolumeBackend};
use kivori_desktop::platform::system::{cpu_percent, SystemProbe};
use kivori_desktop::platform::{
    ActionAvailability, ChangeOrigin, ConfirmationClass, VolumeBackend, VolumeChange,
};

#[test]
fn volume_backend_reads_the_default_output_or_reports_runtime_unavailable() {
    let backend = MacVolumeBackend::new();
    match backend.availability() {
        ActionAvailability::Available { confirmation } => {
            assert_eq!(confirmation, ConfirmationClass::StateConfirmed);
            let percent = backend.read().expect("read volume");
            assert!(percent <= 100);
            let _muted = backend.read_mute().expect("read mute");
        }
        ActionAvailability::RuntimeUnavailable { reason } => {
            eprintln!("no usable output device: {reason}");
            assert!(backend.read().is_err());
        }
        other => panic!("macOS is implemented; got {other:?}"),
    }
}

#[test]
fn cpu_is_a_percentage_between_two_samples() {
    let probe = MacSystemProbe;
    let a = probe.cpu_times().expect("first sample");
    sleep(Duration::from_millis(200));
    let b = probe.cpu_times().expect("second sample");
    let pct = cpu_percent(a, b).unwrap_or_else(|| panic!("ticks did not advance: {a:?} -> {b:?}"));
    assert!(pct <= 100, "{pct}");
}

#[test]
fn ram_is_a_plausible_percentage() {
    let pct = MacSystemProbe.ram_percent().expect("ram");
    assert!((1..=100).contains(&pct), "{pct}");
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

/// Restores the original volume and mute on drop, including on panic.
struct Restore<'a> {
    backend: &'a MacVolumeBackend,
    volume: u8,
    muted: bool,
}

impl<'a> Restore<'a> {
    fn capture(backend: &'a MacVolumeBackend) -> Self {
        Self {
            backend,
            volume: backend.read().expect("read volume"),
            muted: backend.read_mute().expect("read mute"),
        }
    }
}

impl Drop for Restore<'_> {
    fn drop(&mut self) {
        let _ = self.backend.set(self.volume);
        let _ = self.backend.set_mute(self.muted);
        // Belt and braces: osascript works even if the backend thread were wedged.
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "set volume output volume {} output muted {}",
                self.volume, self.muted
            ))
            .status();
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
    backend: &MacVolumeBackend,
    timeout: Duration,
    done: impl Fn(&[VolumeChange]) -> bool,
) -> Vec<VolumeChange> {
    let deadline = Instant::now() + timeout;
    let mut seen = Vec::new();
    while Instant::now() < deadline && !done(&seen) {
        if let Some(c) = backend.try_recv_change() {
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
    let backend = MacVolumeBackend::new();
    assert!(matches!(
        backend.availability(),
        ActionAvailability::Available { .. }
    ));
    let guard = Restore::capture(&backend);
    eprintln!("original: volume={} muted={}", guard.volume, guard.muted);

    // The initial bind publishes EndpointRebind; drain it.
    let initial = collect(&backend, Duration::from_millis(300), |s| !s.is_empty());
    eprintln!("initial: {initial:?}");

    // Volume: original +/- 10 and back, read-back must equal.
    let t = target(guard.volume);
    assert_eq!(backend.set(t), Ok(t));
    assert_eq!(backend.read(), Ok(t));
    let echo = collect(&backend, Duration::from_millis(500), |s| !s.is_empty());
    eprintln!("after own set({t}): {echo:?}");
    assert!(
        echo.iter().all(|c| c.origin == ChangeOrigin::Kivori),
        "own write must echo as Kivori or not at all: {echo:?}"
    );

    // External change via AppleScript.
    let ext = target(t);
    let ok = Command::new("osascript")
        .arg("-e")
        .arg(format!("set volume output volume {ext}"))
        .status()
        .expect("osascript")
        .success();
    assert!(ok);
    let seen = collect(&backend, Duration::from_secs(2), |s| {
        s.iter().any(|c| c.origin == ChangeOrigin::External)
    });
    eprintln!("after osascript {ext}: {seen:?}");
    let external = seen
        .iter()
        .find(|c| c.origin == ChangeOrigin::External)
        .expect("external change observed");
    // osascript's 0..100 maps through its own scale; allow one point of rounding.
    assert!(external.percent.abs_diff(ext) <= 1, "{external:?}");

    assert_eq!(backend.set(guard.volume), Ok(guard.volume));

    // Mute: toggle and back.
    assert_eq!(backend.set_mute(!guard.muted), Ok(!guard.muted));
    assert_eq!(backend.read_mute(), Ok(!guard.muted));
    let echo = collect(&backend, Duration::from_millis(500), |s| {
        s.iter().any(|c| c.muted != Some(guard.muted))
    });
    eprintln!("after own set_mute({}): {echo:?}", !guard.muted);
    assert!(
        echo.iter().all(|c| c.origin == ChangeOrigin::Kivori),
        "{echo:?}"
    );
    assert_eq!(backend.set_mute(guard.muted), Ok(guard.muted));

    drop(guard);
    sleep(Duration::from_millis(100));
    eprintln!(
        "restored: volume={:?} muted={:?}",
        backend.read(),
        backend.read_mute()
    );
}
