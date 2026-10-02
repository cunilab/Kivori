//! Windows media playback observation via the Global System Media Transport Controls.
//!
//! THREADING: a dedicated `kivori-media` thread joins the MTA, owns every WinRT object, and polls
//! the current session once a second, storing the mapped result for the (Sync) public API. The
//! blocking `RequestAsync().get()` is acceptable there because nothing else runs on that thread.
//! The thread stops (within ~100 ms of its sleep slice) when the observer is dropped.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use kivori_model::desk::MediaStatus;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

use crate::platform::MediaObserver;

const POLL_SLICES: u32 = 10;
const SLICE: Duration = Duration::from_millis(100);

type Shared = Arc<Mutex<Option<MediaStatus>>>;

pub struct WindowsMediaObserver {
    status: Shared,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WindowsMediaObserver {
    pub fn new() -> Self {
        let status: Shared = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_status, thread_stop) = (Arc::clone(&status), Arc::clone(&stop));
        let thread = std::thread::Builder::new()
            .name("kivori-media".to_string())
            .spawn(move || media_thread(&thread_status, &thread_stop))
            .ok();
        // A failed spawn leaves the status at `None`: unknown, not a panic.
        Self {
            status,
            stop,
            thread,
        }
    }
}

impl Default for WindowsMediaObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WindowsMediaObserver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl MediaObserver for WindowsMediaObserver {
    fn status(&self) -> Option<MediaStatus> {
        *self.status.lock().expect("media status mutex")
    }
}

/// Maps a GSMTC playback status to ours. `Opened`/`Changing` are transitions, so they keep the
/// previous known value, or stay unknown if there is none (never guessed as Stopped).
fn map_playback(status: PlaybackStatus, prev: Option<MediaStatus>) -> Option<MediaStatus> {
    if status == PlaybackStatus::Playing {
        Some(MediaStatus::Playing)
    } else if status == PlaybackStatus::Paused {
        Some(MediaStatus::Paused)
    } else if status == PlaybackStatus::Opened || status == PlaybackStatus::Changing {
        prev
    } else {
        // Stopped, Closed.
        Some(MediaStatus::Stopped)
    }
}

/// One poll. `Err` = a WinRT call failed (unknown).
fn poll(
    manager: &Manager,
    prev: Option<MediaStatus>,
) -> windows::core::Result<Option<MediaStatus>> {
    let session = match manager.GetCurrentSession() {
        Ok(session) => session,
        // No current session: windows-rs turns the null object into an *empty* error (code
        // S_OK, windows-core `Type::from_abi`). That is "nothing is playing"; any other error is
        // a real failure and stays unknown.
        Err(error) if error.code() == windows::core::HRESULT(0) => {
            return Ok(Some(MediaStatus::Stopped))
        }
        Err(error) => return Err(error),
    };
    let playback = session.GetPlaybackInfo()?.PlaybackStatus()?;
    Ok(map_playback(playback, prev))
}

fn media_thread(shared: &Shared, stop: &AtomicBool) {
    // SAFETY: first COM call on this dedicated thread; paired with `CoUninitialize` below.
    // WinRT objects work in the MTA this joins.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    poll_loop(shared, stop);

    // SAFETY: matches the `CoInitializeEx` above on this thread; every WinRT object lived inside
    // `poll_loop` and has been released.
    unsafe { CoUninitialize() };
}

fn poll_loop(shared: &Shared, stop: &AtomicBool) {
    let mut manager: Option<Manager> = None;
    while !stop.load(Ordering::SeqCst) {
        if manager.is_none() {
            manager = Manager::RequestAsync().and_then(|op| op.get()).ok();
        }
        let prev = *shared.lock().expect("media status mutex");
        let next = match manager.as_ref().map(|m| poll(m, prev)) {
            Some(Ok(status)) => status,
            Some(Err(_)) => {
                // Drop the manager so the next tick re-requests a fresh one.
                manager = None;
                None
            }
            None => None,
        };
        *shared.lock().expect("media status mutex") = next;

        for _ in 0..POLL_SLICES {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(SLICE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_paused_and_terminal_states_map_directly() {
        for prev in [None, Some(MediaStatus::Paused)] {
            assert_eq!(
                map_playback(PlaybackStatus::Playing, prev),
                Some(MediaStatus::Playing)
            );
            assert_eq!(
                map_playback(PlaybackStatus::Paused, prev),
                Some(MediaStatus::Paused)
            );
            assert_eq!(
                map_playback(PlaybackStatus::Stopped, prev),
                Some(MediaStatus::Stopped)
            );
            assert_eq!(
                map_playback(PlaybackStatus::Closed, prev),
                Some(MediaStatus::Stopped)
            );
        }
    }

    #[test]
    fn opened_and_changing_keep_the_previous_value() {
        for transient in [PlaybackStatus::Opened, PlaybackStatus::Changing] {
            assert_eq!(
                map_playback(transient, Some(MediaStatus::Playing)),
                Some(MediaStatus::Playing)
            );
            assert_eq!(
                map_playback(transient, Some(MediaStatus::Paused)),
                Some(MediaStatus::Paused)
            );
            assert_eq!(map_playback(transient, None), None, "never guessed");
        }
    }
}
