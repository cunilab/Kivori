//! Smoke tests against the real Windows APIs. Nothing here asserts machine-specific values.
#![cfg(windows)]

use std::time::{Duration, Instant};

use kivori_desktop::platform::system::{SystemMonitor, SystemProbe};
use kivori_desktop::platform::windows::WindowsVolumeBackend;
use kivori_desktop::platform::windows::{local_time, WindowsMediaObserver, WindowsSystemProbe};
use kivori_desktop::platform::{AppVolumeBackend, AppVolumeError, MediaObserver};

#[test]
fn ram_percent_is_a_real_percentage() {
    let ram = WindowsSystemProbe
        .ram_percent()
        .expect("GlobalMemoryStatusEx");
    assert!(ram <= 100);
}

#[test]
fn two_cpu_samples_give_a_percentage() {
    let mut monitor = SystemMonitor::new(WindowsSystemProbe);
    assert_eq!(monitor.sample().cpu_percent, None, "first sample primes");
    std::thread::sleep(Duration::from_millis(200));
    let cpu = monitor.sample().cpu_percent.expect("GetSystemTimes");
    assert!(cpu <= 100);
}

#[test]
fn local_time_is_readable() {
    let t = local_time().expect("GetLocalTime");
    assert!(t.hour < 24 && t.minute < 60 && t.second < 60);
}

#[test]
fn media_observer_constructs_reads_and_drops_promptly() {
    let started = Instant::now();
    let observer = WindowsMediaObserver::new();
    let _ = observer.status(); // None or Some: both honest; it must just not panic.
    drop(observer);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_session_for_an_app_that_is_not_running_is_no_session() {
    let backend = WindowsVolumeBackend::new();
    let app = "kivori-test-not-running.exe";
    // Enumerating the real sessions must not panic, with or without an audio device.
    assert_eq!(backend.read(app), Err(AppVolumeError::NoSession));
    assert_eq!(backend.set(app, 50), Err(AppVolumeError::NoSession));
    assert_eq!(backend.read_mute(app), Err(AppVolumeError::NoSession));
    assert_eq!(backend.set_mute(app, true), Err(AppVolumeError::NoSession));
}

mod host_events {
    use std::sync::mpsc;
    use std::time::Duration;

    use kivori_desktop::platform::host_events::{HostEvent, HostEventsError};
    use kivori_desktop::platform::windows::host_events::WindowsHostEvents;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        PostMessageW, WM_WTSSESSION_CHANGE, WTS_CONSOLE_DISCONNECT, WTS_SESSION_LOCK,
        WTS_SESSION_UNLOCK,
    };

    fn post(watcher: &WindowsHostEvents, code: u32) {
        // SAFETY: the handle is the watcher's live message-only window.
        unsafe {
            PostMessageW(
                HWND(watcher.window_handle() as *mut _),
                WM_WTSSESSION_CHANGE,
                WPARAM(code as usize),
                LPARAM(1),
            )
            .expect("post");
        }
    }

    #[test]
    fn the_watcher_starts_or_reports_unavailable_and_never_panics() {
        let (tx, _rx) = mpsc::channel();
        match WindowsHostEvents::start(tx) {
            Ok(watcher) => drop(watcher),
            Err(HostEventsError::Unavailable(reason)) => assert!(!reason.is_empty()),
        }
    }

    #[test]
    fn a_session_change_posted_to_its_window_arrives_as_a_host_event() {
        let (tx, rx) = mpsc::channel();
        let Ok(watcher) = WindowsHostEvents::start(tx) else {
            return; // Registration refused on this runner: covered by the test above.
        };
        for (code, expected) in [
            (WTS_SESSION_LOCK, HostEvent::Locked),
            (WTS_SESSION_UNLOCK, HostEvent::Unlocked),
            (WTS_CONSOLE_DISCONNECT, HostEvent::ConsoleDisconnected),
        ] {
            post(&watcher, code);
            let signal = rx
                .recv_timeout(Duration::from_secs(2))
                .expect("the window thread forwards it");
            assert_eq!(signal.event, expected);
            assert!(signal.ack.is_none());
        }
    }
}
