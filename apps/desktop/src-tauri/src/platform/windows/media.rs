//! Windows media playback observation via the Global System Media Transport Controls.
//!
//! THREADING: a dedicated `kivori-media` thread joins the MTA, owns every WinRT object, and polls
//! the current session once a second (playback status, plus title and artist from the media
//! properties), storing the mapped result for the (Sync) public API. The blocking
//! `RequestAsync().get()` / `TryGetMediaPropertiesAsync().get()` are acceptable there because
//! nothing else runs on that thread. Titles and artists are never logged (ADR-0005).
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

use crate::platform::{MediaObserver, NowPlaying};

const POLL_SLICES: u32 = 10;
const SLICE: Duration = Duration::from_millis(100);

type View = (Option<MediaStatus>, Option<NowPlaying>);
type Shared = Arc<Mutex<View>>;

pub struct WindowsMediaObserver {
    status: Shared,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WindowsMediaObserver {
    pub fn new() -> Self {
        let status: Shared = Arc::new(Mutex::new((None, None)));
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
        self.status.lock().expect("media status mutex").0
    }
    fn now_playing(&self) -> Option<NowPlaying> {
        self.status.lock().expect("media status mutex").1.clone()
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

/// What is playing, kept consistent with the status: nothing when the status is unknown or
/// Stopped, or when the session gives neither a title nor an artist (unknown, not "untitled").
fn map_now_playing(status: Option<MediaStatus>, title: &str, artist: &str) -> Option<NowPlaying> {
    let active = matches!(status, Some(MediaStatus::Playing | MediaStatus::Paused));
    (active && (!title.is_empty() || !artist.is_empty())).then(|| NowPlaying {
        title: title.to_string(),
        artist: artist.to_string(),
    })
}

/// One poll. `Err` = a WinRT call failed (unknown).
fn poll(manager: &Manager, prev: Option<MediaStatus>) -> windows::core::Result<View> {
    let session = match manager.GetCurrentSession() {
        Ok(session) => session,
        // No current session: windows-rs turns the null object into an *empty* error (code
        // S_OK, windows-core `Type::from_abi`). That is "nothing is playing"; any other error is
        // a real failure and stays unknown.
        Err(error) if error.code() == windows::core::HRESULT(0) => {
            return Ok((Some(MediaStatus::Stopped), None))
        }
        Err(error) => return Err(error),
    };
    let playback = session.GetPlaybackInfo()?.PlaybackStatus()?;
    let status = map_playback(playback, prev);
    // Properties can fail while a player is still loading its track: the status stays known and
    // only the title is unknown, so this failure does not reset the manager.
    let now = session
        .TryGetMediaPropertiesAsync()
        .and_then(|op| op.get())
        .and_then(|props| Ok((props.Title()?, props.Artist()?)))
        .ok()
        .and_then(|(title, artist)| {
            map_now_playing(status, &title.to_string_lossy(), &artist.to_string_lossy())
        });
    Ok((status, now))
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
        let prev = shared.lock().expect("media status mutex").0;
        let next = match manager.as_ref().map(|m| poll(m, prev)) {
            Some(Ok(view)) => view,
            Some(Err(_)) => {
                // Drop the manager so the next tick re-requests a fresh one.
                manager = None;
                (None, None)
            }
            None => (None, None),
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

    #[test]
    fn now_playing_follows_the_status() {
        let both = Some(NowPlaying {
            title: "T".to_string(),
            artist: "A".to_string(),
        });
        for active in [MediaStatus::Playing, MediaStatus::Paused] {
            assert_eq!(map_now_playing(Some(active), "T", "A"), both);
            assert_eq!(
                map_now_playing(Some(active), "", "A"),
                Some(NowPlaying {
                    title: String::new(),
                    artist: "A".to_string(),
                })
            );
            assert_eq!(map_now_playing(Some(active), "", ""), None, "unknown");
        }
        assert_eq!(map_now_playing(Some(MediaStatus::Stopped), "T", "A"), None);
        assert_eq!(map_now_playing(None, "T", "A"), None);
    }
}
