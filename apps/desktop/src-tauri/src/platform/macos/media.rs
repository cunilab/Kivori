//! macOS now-playing observation (ADR-0009). macOS has no public system-wide now-playing API
//! (`MPNowPlayingInfoCenter` only publishes; MediaRemote is private and entitlement-gated since
//! 15.4), so this is layered:
//!
//! 1. ADAPTER (primary): the vendored ungive/mediaremote-adapter (BSD-3, `vendor/`, built by
//!    `build.rs`). Apple's own `/usr/bin/perl` is entitled to MediaRemote and loads the adapter
//!    dylib, which streams one JSON line per change. Each start first runs the adapter's `test`
//!    command, which proves the entitlement works (publishing a silent test session if nothing
//!    plays), so an empty payload afterwards really means "nothing is playing" anywhere: Stopped.
//! 2. APPLESCRIPT (fallback, only while the adapter is unavailable): Spotify and Music through
//!    public Apple Events, never launching them. They cannot see browsers, so finding nothing
//!    playing there is unknown, not Stopped.
//! 3. Otherwise unknown (`None`).
//!
//! THREADING: a `kivori-media` thread owns every child process and writes the cached view the
//! (Sync) public API reads; a short-lived reader thread turns the adapter's stdout into lines.
//! Children get SIGTERM then SIGKILL and are reaped on every exit path; the thread stops within
//! about a second of drop.
//!
//! PRIVACY: titles and artists are never logged (ADR-0005).

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kivori_model::desk::MediaStatus;
use serde_json::Value;

use crate::platform::{MediaObserver, NowPlaying};

type View = (Option<MediaStatus>, Option<NowPlaying>);
type Shared = Arc<Mutex<View>>;

const UNKNOWN: View = (None, None);
const PERL: &str = "/usr/bin/perl";
const OSASCRIPT: &str = "/usr/bin/osascript";
const SCRIPT: &str = "mediaremote-adapter.pl";
const SLICE: Duration = Duration::from_millis(100);
const POLL: Duration = Duration::from_secs(1);
/// An empty adapter payload must last this long before it means Stopped: the stream prints one
/// while its first queries are still in flight (~40 ms here), and briefly when apps switch.
const SETTLE: Duration = Duration::from_secs(1);
/// `test` takes ~0.5 s, up to ~3.5 s when it has to start its test client.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);
/// osascript blocks while macOS shows the one-time Automation prompt; give up and stay unknown.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(3);
const MIN_BACKOFF: Duration = Duration::from_secs(2);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// A stream that lasted this long was healthy: its exit restarts the backoff from the minimum.
const HEALTHY_RUN: Duration = Duration::from_secs(60);

/// Spotify then Music. `is running` never launches an app; `run script` compiles the `tell` only
/// when it runs, so a missing app never shows a "Where is …?" dialog. One record per player
/// (RS-separated): `name US state US title US artist`, or `name US not-running` / `name US error`.
const PLAYERS_SCRIPT: &str = r#"set us to character id 31
set rs to character id 30
set out to ""
repeat with p in {"Spotify", "Music"}
	set p to contents of p
	if application p is running then
		try
			set r to run script "tell application \"" & p & "\"
				set s to player state as text
				if s is not \"playing\" and s is not \"paused\" then return {s, \"\", \"\"}
				return {s, name of current track, artist of current track}
			end tell"
			set out to out & p & us & (item 1 of r) & us & (item 2 of r) & us & (item 3 of r) & rs
		on error
			set out to out & p & us & "error" & rs
		end try
	else
		set out to out & p & us & "not-running" & rs
	end if
end repeat
return out"#;

pub struct MacMediaObserver {
    shared: Shared,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MacMediaObserver {
    pub fn new() -> Self {
        let shared: Shared = Arc::new(Mutex::new(UNKNOWN));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_shared, thread_stop) = (Arc::clone(&shared), Arc::clone(&stop));
        let thread = std::thread::Builder::new()
            .name("kivori-media".to_string())
            .spawn(move || media_thread(&thread_shared, &thread_stop))
            .ok();
        // A failed spawn leaves everything unknown, not a panic.
        Self {
            shared,
            stop,
            thread,
        }
    }

    fn view(&self) -> View {
        self.shared.lock().expect("media view mutex").clone()
    }
}

impl Default for MacMediaObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MacMediaObserver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl MediaObserver for MacMediaObserver {
    fn status(&self) -> Option<MediaStatus> {
        self.view().0
    }
    fn now_playing(&self) -> Option<NowPlaying> {
        self.view().1
    }
}

fn publish(shared: &Shared, view: View) {
    *shared.lock().expect("media view mutex") = view;
}

fn media_thread(shared: &Shared, stop: &AtomicBool) {
    let mut backoff = MIN_BACKOFF;
    while !stop.load(Ordering::SeqCst) {
        let ran = adapter_dir().map_or(Duration::ZERO, |dir| run_adapter(&dir, shared, stop));
        if ran >= HEALTHY_RUN {
            backoff = MIN_BACKOFF;
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }
        tracing::warn!(
            retry_in = ?backoff,
            "now-playing adapter unavailable; observing Spotify/Music via AppleScript"
        );
        let retry_at = Instant::now() + backoff;
        backoff = (backoff * 2).min(MAX_BACKOFF);
        while Instant::now() < retry_at {
            let view = run_bounded(
                Command::new(OSASCRIPT).args(["-e", PLAYERS_SCRIPT]),
                SCRIPT_TIMEOUT,
                stop,
            )
            .filter(|(ok, _)| *ok)
            .map_or(UNKNOWN, |(_, out)| fallback_view(&parse_players(&out)));
            publish(shared, view);
            if !sleep_unless_stopped(POLL, stop) {
                return;
            }
        }
    }
}

/// The adapter files: inside the app bundle (release), else this build's `OUT_DIR` (dev, tests).
fn adapter_dir() -> Option<PathBuf> {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("../Resources/mediaremote-adapter")));
    let built = option_env!("KIVORI_MEDIAREMOTE_ADAPTER_DIR").map(PathBuf::from);
    [bundled, built]
        .into_iter()
        .flatten()
        .find(|dir| dir.join(SCRIPT).is_file())
}

/// Runs the adapter until it exits, emits something unparseable, or `stop`. Returns how long it
/// streamed (zero when the entitlement test failed) and leaves the view unknown.
fn run_adapter(dir: &Path, shared: &Shared, stop: &AtomicBool) -> Duration {
    let perl = || {
        let mut cmd = Command::new(PERL);
        // A clean environment: no PERL5LIB/PERL5OPT from the user's session reaches the helper.
        cmd.env_clear()
            .env("PATH", "/usr/bin:/bin")
            .arg(dir.join(SCRIPT))
            .arg(dir.join("MediaRemoteAdapter.framework"));
        cmd
    };
    let entitled = run_bounded(
        perl()
            .arg(dir.join("MediaRemoteAdapterTestClient"))
            .arg("test"),
        TEST_TIMEOUT,
        stop,
    )
    .is_some_and(|(ok, _)| ok);
    if !entitled {
        return Duration::ZERO;
    }
    let Ok(child) = perl()
        .args([
            "stream",
            "--no-diff",
            "--no-artwork",
            "--allow-missing-title",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Duration::ZERO;
    };
    let mut child = Reaped(child);
    let Some(stdout) = child.0.stdout.take() else {
        return Duration::ZERO;
    };
    let (tx, rx) = mpsc::channel();
    let Ok(reader) = std::thread::Builder::new()
        .name("kivori-media-reader".to_string())
        .spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx.send(line).is_err() {
                    break;
                }
            }
        })
    else {
        return Duration::ZERO;
    };

    tracing::info!("now-playing adapter streaming");
    let started = Instant::now();
    let mut tracker = AdapterTracker::default();
    while !stop.load(Ordering::SeqCst) {
        match rx.recv_timeout(SLICE) {
            Ok(Ok(line)) => match parse_adapter_line(&line) {
                Ok(Some(event)) => tracker.observe(event, Instant::now()),
                Ok(None) => {}
                Err(()) => break,
            },
            Err(RecvTimeoutError::Timeout) => {}
            // Exited (EOF) or non-UTF-8 output.
            Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => break,
        }
        publish(shared, tracker.view(Instant::now()));
    }
    // Terminating the child closes its stdout, which ends the reader.
    drop(child);
    let _ = reader.join();
    publish(shared, UNKNOWN);
    started.elapsed()
}

/// A child that is terminated and reaped when dropped, so no helper outlives its owner.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        // Already reaped (std caches the status): the pid may be reused, never signal it.
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        // SIGTERM first: the adapter's handler stops its run loop and, in `test`, shuts down its
        // own test-client child, which a SIGKILL would orphan.
        if let Ok(pid) = i32::try_from(self.0.id()) {
            // SAFETY: plain syscall on our own unreaped child's pid.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if matches!(self.0.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs `cmd` to completion within `timeout`, giving up early on `stop`. Returns whether it
/// succeeded and its stdout. Only for commands with small output (it is read after exit).
fn run_bounded(cmd: &mut Command, timeout: Duration, stop: &AtomicBool) -> Option<(bool, String)> {
    let mut child = Reaped(
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?,
    );
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.0.try_wait().ok()? {
            let mut out = String::new();
            child.0.stdout.take()?.read_to_string(&mut out).ok()?;
            return Some((status.success(), out));
        }
        if stop.load(Ordering::SeqCst) || Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Sleeps `total` in slices; `false` if `stop` was raised.
fn sleep_unless_stopped(total: Duration, stop: &AtomicBool) -> bool {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(SLICE);
    }
    !stop.load(Ordering::SeqCst)
}

/// Title and artist, or `None` when the player gave neither (unknown, not "untitled").
fn now_playing(title: &str, artist: &str) -> Option<NowPlaying> {
    (!title.is_empty() || !artist.is_empty()).then(|| NowPlaying {
        title: title.to_string(),
        artist: artist.to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AdapterEvent {
    /// No now-playing app.
    Nothing,
    Media {
        playing: bool,
        now: Option<NowPlaying>,
    },
}

/// One `stream --no-diff` line: `{"type":"data","diff":false,"payload":{...}}`. `Ok(None)` is a
/// line kind we do not use; `Err` is anything off-format (the adapter or API changed), which
/// marks the adapter broken rather than being guessed at.
fn parse_adapter_line(line: &str) -> Result<Option<AdapterEvent>, ()> {
    let value: Value = serde_json::from_str(line).map_err(|_| ())?;
    if value.get("type").and_then(Value::as_str) != Some("data") {
        return Ok(None);
    }
    let payload = value.get("payload").and_then(Value::as_object).ok_or(())?;
    if payload.is_empty() {
        return Ok(Some(AdapterEvent::Nothing));
    }
    let playing = payload.get("playing").and_then(Value::as_bool).ok_or(())?;
    let text = |key| payload.get(key).and_then(Value::as_str).unwrap_or("");
    Ok(Some(AdapterEvent::Media {
        playing,
        now: now_playing(text("title"), text("artist")),
    }))
}

/// Turns adapter events into the published view. An empty payload becomes Stopped only once it
/// has lasted [`SETTLE`]; until then the previous view stands (unknown right after start).
#[derive(Debug, Default)]
struct AdapterTracker {
    view: View,
    nothing_since: Option<Instant>,
}

impl AdapterTracker {
    fn observe(&mut self, event: AdapterEvent, at: Instant) {
        match event {
            AdapterEvent::Nothing => {
                self.nothing_since.get_or_insert(at);
            }
            AdapterEvent::Media { playing, now } => {
                self.nothing_since = None;
                let status = if playing {
                    MediaStatus::Playing
                } else {
                    MediaStatus::Paused
                };
                self.view = (Some(status), now);
            }
        }
    }

    fn view(&mut self, at: Instant) -> View {
        if self
            .nothing_since
            .is_some_and(|since| at.duration_since(since) >= SETTLE)
        {
            self.view = (Some(MediaStatus::Stopped), None);
        }
        self.view.clone()
    }
}

/// The players [`PLAYERS_SCRIPT`] found playing or paused, in script order. Not running,
/// stopped, permission denied and anything unreadable are left out: unknown.
fn parse_players(out: &str) -> Vec<(MediaStatus, Option<NowPlaying>)> {
    out.trim_end_matches('\n')
        .split('\u{1e}')
        .filter_map(|record| {
            let fields: Vec<&str> = record.split('\u{1f}').collect();
            let status = match fields.get(1).copied() {
                Some("playing") => MediaStatus::Playing,
                Some("paused") => MediaStatus::Paused,
                _ => return None,
            };
            let field = |i: usize| fields.get(i).copied().unwrap_or("");
            Some((status, now_playing(field(2), field(3))))
        })
        .collect()
}

/// Fallback layer: a playing player wins, else a paused one. Nothing found is unknown, never
/// Stopped: a browser or another app may be playing where AppleScript cannot see.
fn fallback_view(players: &[(MediaStatus, Option<NowPlaying>)]) -> View {
    [MediaStatus::Playing, MediaStatus::Paused]
        .into_iter()
        .find_map(|wanted| players.iter().find(|(status, _)| *status == wanted))
        .map_or(UNKNOWN, |(status, now)| (Some(*status), now.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn np(title: &str, artist: &str) -> Option<NowPlaying> {
        Some(NowPlaying {
            title: title.to_string(),
            artist: artist.to_string(),
        })
    }

    #[test]
    fn adapter_lines_parse_to_events() {
        let playing = r#"{"type":"data","diff":false,"payload":{"artist":"A \"q\" ü","playbackRate":1,"title":"T","playing":true,"bundleIdentifier":"com.apple.Music","processIdentifier":711}}"#;
        assert_eq!(
            parse_adapter_line(playing),
            Ok(Some(AdapterEvent::Media {
                playing: true,
                now: np("T", "A \"q\" ü"),
            }))
        );
        let paused_untitled = r#"{"type":"data","diff":false,"payload":{"title":null,"playing":false,"processIdentifier":9}}"#;
        assert_eq!(
            parse_adapter_line(paused_untitled),
            Ok(Some(AdapterEvent::Media {
                playing: false,
                now: None,
            }))
        );
        assert_eq!(
            parse_adapter_line(r#"{"type":"data","diff":false,"payload":{}}"#),
            Ok(Some(AdapterEvent::Nothing))
        );
        assert_eq!(parse_adapter_line(r#"{"type":"other"}"#), Ok(None));
    }

    #[test]
    fn off_format_adapter_lines_are_errors_not_guesses() {
        for line in [
            "",
            "Failed to load framework",
            r#"{"type":"data"}"#,
            r#"{"type":"data","payload":{"title":"T"}}"#,
            r#"{"type":"data","payload":{"playing":"yes"}}"#,
        ] {
            assert_eq!(parse_adapter_line(line), Err(()), "{line}");
        }
    }

    #[test]
    fn empty_payload_is_stopped_only_after_it_settles() {
        let t0 = Instant::now();
        let mut tracker = AdapterTracker::default();
        // Startup: the stream's first line is empty while its queries are in flight.
        tracker.observe(AdapterEvent::Nothing, t0);
        assert_eq!(tracker.view(t0 + Duration::from_millis(40)), UNKNOWN);
        let media = AdapterEvent::Media {
            playing: true,
            now: np("T", "A"),
        };
        tracker.observe(media, t0 + Duration::from_millis(80));
        let playing = (Some(MediaStatus::Playing), np("T", "A"));
        assert_eq!(tracker.view(t0 + SETTLE * 2), playing);
        // A brief empty payload between apps keeps the last view...
        let t1 = t0 + SETTLE * 3;
        tracker.observe(AdapterEvent::Nothing, t1);
        assert_eq!(tracker.view(t1 + SETTLE / 2), playing);
        // ...a lasting one is a real "nothing playing".
        assert_eq!(
            tracker.view(t1 + SETTLE),
            (Some(MediaStatus::Stopped), None)
        );
        tracker.observe(
            AdapterEvent::Media {
                playing: false,
                now: None,
            },
            t1 + SETTLE * 2,
        );
        assert_eq!(
            tracker.view(t1 + SETTLE * 4),
            (Some(MediaStatus::Paused), None)
        );
    }

    fn record(fields: &[&str]) -> String {
        fields.join("\u{1f}") + "\u{1e}"
    }

    #[test]
    fn players_output_parses_playing_and_paused_only() {
        let out = record(&["Spotify", "paused", "S", "SA"])
            + &record(&["Music", "playing", "M", ""])
            + "\n";
        assert_eq!(
            parse_players(&out),
            vec![
                (MediaStatus::Paused, np("S", "SA")),
                (MediaStatus::Playing, np("M", "")),
            ]
        );
        let none = record(&["Spotify", "not-running"])
            + &record(&["Music", "error"])
            + &record(&["Music", "stopped", "", ""])
            + "\n";
        assert!(parse_players(&none).is_empty());
        assert!(parse_players("").is_empty());
    }

    #[test]
    fn fallback_prefers_playing_and_never_claims_stopped() {
        let players = [
            (MediaStatus::Paused, np("S", "SA")),
            (MediaStatus::Playing, np("M", "MA")),
        ];
        assert_eq!(
            fallback_view(&players),
            (Some(MediaStatus::Playing), np("M", "MA"))
        );
        assert_eq!(
            fallback_view(&players[..1]),
            (Some(MediaStatus::Paused), np("S", "SA"))
        );
        // Spotify and Music idle or absent: a browser may still be playing.
        assert_eq!(fallback_view(&[]), UNKNOWN);
    }
}
