//! Linux master volume/mute and per-app volume through the PulseAudio protocol (PipeWire serves it
//! through `pipewire-pulse`).
//!
//! THREADING: like the Windows backend, a dedicated `kivori-audio` OS thread owns the connection
//! (see `pulse`). The public, `Sync` API sends it a [`Command`] with a one-shot reply channel and
//! waits. The subscribe callback is INGRESS ONLY: it posts `Command::Event` and the thread does the
//! reading.
//!
//! ENDPOINT: the default output device is a MOVING TARGET. A server or sink event makes the thread
//! re-read the default sink; when its name changed it publishes the NEW device's confirmed values
//! as `EndpointRebind`.
//!
//! ECHO: pulse events carry no origin. After a write the thread remembers the state it read back;
//! the change it then observes is `Kivori` when it is that state (within [`ECHO_WINDOW`]), else
//! `External` (the GNOME panel, a media key, `pavucontrol`). A change that leaves the state as last
//! reported publishes nothing.
//!
//! APPS: an app's streams are the sink inputs on the default sink whose `application.process.binary`
//! or `application.name` is the app id (the lowercase executable name the focus observer reports).
//! A write goes to every stream, and is only confirmed if every read-back agrees.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::pulse::{
    channels_percent, join_within, with_percent, Conn, Event, Link, Sink, SinkInput, CALL_DEADLINE,
    JOIN_DEADLINE, UNREACHABLE,
};
use crate::platform::{
    ActionAvailability, AppVolumeBackend, AppVolumeError, BackendError, ChangeOrigin,
    ConfirmationClass, VolumeBackend, VolumeChange,
};

/// A change that matches Kivori's own last write this soon after it is the echo of that write.
const ECHO_WINDOW: Duration = Duration::from_secs(2);
/// Events that arrive within this of each other are read once (a slider drag is dozens).
const SETTLE: Duration = Duration::from_millis(30);
/// The longest a burst of events is held before it is read anyway.
const MAX_COALESCE: Duration = Duration::from_millis(200);
/// How often the thread wakes to retry a lost connection.
const RECONNECT: Duration = Duration::from_secs(3);
/// How long `new()` waits for the first connection attempt.
const READY_WAIT: Duration = Duration::from_secs(3);

const NO_SINK: &str = "no default output device";

/// Whether the thread has a usable endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    NoServer = 0,
    NoSink = 1,
    Ok = 2,
}

impl Health {
    fn from_u8(value: u8) -> Self {
        match value {
            2 => Self::Ok,
            1 => Self::NoSink,
            _ => Self::NoServer,
        }
    }
}

/// Requests handled on the owning `kivori-audio` thread.
enum Command {
    Read(Sender<Result<u8, BackendError>>),
    Set(u8, Sender<Result<u8, BackendError>>),
    ReadMute(Sender<Result<bool, BackendError>>),
    SetMute(bool, Sender<Result<bool, BackendError>>),
    AppRead(String, Sender<Result<u8, AppVolumeError>>),
    AppSet(String, u8, Sender<Result<u8, AppVolumeError>>),
    AppReadMute(String, Sender<Result<bool, AppVolumeError>>),
    AppSetMute(String, bool, Sender<Result<bool, AppVolumeError>>),
    /// Raised by the subscribe callback.
    Event,
    Shutdown,
}

pub struct LinuxVolumeBackend {
    commands: Sender<Command>,
    changes: Mutex<Receiver<VolumeChange>>,
    health: Arc<AtomicU8>,
    thread: Option<JoinHandle<()>>,
}

impl LinuxVolumeBackend {
    #[must_use]
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<Command>();
        let (change_tx, change_rx) = mpsc::channel::<VolumeChange>();
        let health = Arc::new(AtomicU8::new(Health::NoServer as u8));
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let thread_health = Arc::clone(&health);
        let self_commands = command_tx.clone();
        let thread = std::thread::Builder::new()
            .name("kivori-audio".to_string())
            .spawn(move || {
                audio_thread(
                    &command_rx,
                    &change_tx,
                    &thread_health,
                    &self_commands,
                    &ready_tx,
                )
            })
            .ok();

        // Block until the owning thread has attempted its first connection, so `availability()` is
        // right the instant `new()` returns. A server that accepts the socket and never answers
        // must not hang startup: after READY_WAIT the backend is simply unavailable until it does.
        let ready = ready_rx.recv_timeout(READY_WAIT).is_ok();
        Self {
            commands: command_tx,
            changes: Mutex::new(change_rx),
            health,
            // A thread stuck in its first connect is detached rather than joined on drop.
            thread: thread.filter(|_| ready),
        }
    }

    fn health(&self) -> Health {
        Health::from_u8(self.health.load(Ordering::SeqCst))
    }

    /// Sends a command and waits for its reply, for at most [`CALL_DEADLINE`]: the owning thread
    /// bounds its own requests, so this is the backstop that keeps a caller (the device thread)
    /// from ever sleeping on a stuck audio server.
    fn call<T, E>(
        &self,
        make: impl FnOnce(Sender<Result<T, E>>) -> Command,
        fail: impl Fn(&str) -> E,
    ) -> Result<T, E> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(make(reply_tx))
            .map_err(|_| fail("the audio thread is gone"))?;
        reply_rx
            .recv_timeout(CALL_DEADLINE)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => fail("the audio server did not answer in time"),
                RecvTimeoutError::Disconnected => fail("the audio thread is gone"),
            })?
    }

    fn master<T>(
        &self,
        make: impl FnOnce(Sender<Result<T, BackendError>>) -> Command,
    ) -> Result<T, BackendError> {
        self.call(make, |why| BackendError::Os(why.to_string()))
    }

    fn app<T>(
        &self,
        make: impl FnOnce(Sender<Result<T, AppVolumeError>>) -> Command,
    ) -> Result<T, AppVolumeError> {
        self.call(make, |why| AppVolumeError::Os(why.to_string()))
    }
}

impl Default for LinuxVolumeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LinuxVolumeBackend {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            // Bounded: a thread inside a request ends it by its own deadline; past that it is
            // detached rather than waited on.
            join_within(thread, JOIN_DEADLINE);
        }
    }
}

impl VolumeBackend for LinuxVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        match self.health() {
            Health::Ok => ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            // Linux IS implemented; a missing server or device is a runtime fact, never
            // `NotImplementedYet`.
            Health::NoServer => ActionAvailability::RuntimeUnavailable {
                reason: UNREACHABLE.to_string(),
            },
            Health::NoSink => ActionAvailability::RuntimeUnavailable {
                reason: NO_SINK.to_string(),
            },
        }
    }

    fn read(&self) -> Result<u8, BackendError> {
        self.master(Command::Read)
    }

    fn set(&self, percent: u8) -> Result<u8, BackendError> {
        self.master(|reply| Command::Set(percent, reply))
    }

    fn read_mute(&self) -> Result<bool, BackendError> {
        self.master(Command::ReadMute)
    }

    fn set_mute(&self, muted: bool) -> Result<bool, BackendError> {
        self.master(|reply| Command::SetMute(muted, reply))
    }

    fn try_recv_change(&self) -> Option<VolumeChange> {
        self.changes
            .lock()
            .expect("changes channel mutex")
            .try_recv()
            .ok()
    }
}

impl AppVolumeBackend for LinuxVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        VolumeBackend::availability(self)
    }

    fn read(&self, app: &str) -> Result<u8, AppVolumeError> {
        self.app(|reply| Command::AppRead(app.to_string(), reply))
    }

    fn set(&self, app: &str, percent: u8) -> Result<u8, AppVolumeError> {
        self.app(|reply| Command::AppSet(app.to_string(), percent, reply))
    }

    fn read_mute(&self, app: &str) -> Result<bool, AppVolumeError> {
        self.app(|reply| Command::AppReadMute(app.to_string(), reply))
    }

    fn set_mute(&self, app: &str, muted: bool) -> Result<bool, AppVolumeError> {
        self.app(|reply| Command::AppSetMute(app.to_string(), muted, reply))
    }
}

/// Whether a sink input is `app`'s: `app` is the lowercase executable file name the focus observer
/// reports. Both the stream's `application.process.binary` (by file name, so `/usr/bin/vlc` matches
/// `vlc`) and its `application.name` are compared case-insensitively and exactly.
pub(super) fn matches_app(app: &str, binary: Option<&str>, app_name: Option<&str>) -> bool {
    let app = app.trim().to_lowercase();
    if app.is_empty() {
        return false;
    }
    let by_binary = binary
        .and_then(|b| b.rsplit('/').next())
        .is_some_and(|b| b.to_lowercase() == app);
    by_binary || app_name.is_some_and(|n| n.to_lowercase() == app)
}

/// The one value every session reports, or `None` if there are none or they disagree.
fn agreed<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

/// What the thread knows about the master endpoint it publishes changes for.
struct Master {
    /// The default sink last bound.
    bound: Option<String>,
    /// The state last published (or observed at bind).
    reported: Option<(u8, bool)>,
    /// The state Kivori's last write read back, and when.
    own: Option<((u8, bool), Instant)>,
}

/// The origin of an observed master state: `Kivori` when it is what Kivori's own recent write read
/// back, else `External`. `None` when it is the state already reported.
fn classify_master(
    reported: Option<(u8, bool)>,
    own: Option<((u8, bool), Instant)>,
    seen: (u8, bool),
    now: Instant,
) -> Option<ChangeOrigin> {
    if reported == Some(seen) {
        return None;
    }
    let echo =
        own.is_some_and(|(state, at)| state == seen && now.duration_since(at) <= ECHO_WINDOW);
    Some(if echo {
        ChangeOrigin::Kivori
    } else {
        ChangeOrigin::External
    })
}

struct Audio {
    link: Link,
    master: Master,
    health: Arc<AtomicU8>,
    changes: Sender<VolumeChange>,
}

impl Audio {
    fn set_health(&self, health: Health) {
        self.health.store(health as u8, Ordering::SeqCst);
    }

    /// A usable connection, reconnecting (at most every few seconds) if it was lost.
    fn conn(&mut self) -> Option<&mut Conn> {
        if self.link.ensure() {
            // A fresh connection rebinds: publish the endpoint as found.
            self.master.bound = None;
            self.master.reported = None;
            self.refresh();
        }
        if self.link.is_down() {
            self.set_health(Health::NoServer);
        }
        self.link.conn()
    }

    /// The default sink, updating health. `Err` says why there is none.
    fn sink(&mut self) -> Result<Sink, BackendError> {
        let Some(conn) = self.conn() else {
            return Err(BackendError::Os(UNREACHABLE.to_string()));
        };
        let found = conn.default_sink();
        let ready = conn.is_ready();
        match found {
            Some(sink) => {
                self.set_health(Health::Ok);
                Ok(sink)
            }
            None if ready => {
                self.set_health(Health::NoSink);
                Err(BackendError::NoEndpoint)
            }
            None => {
                self.set_health(Health::NoServer);
                Err(BackendError::Os(UNREACHABLE.to_string()))
            }
        }
    }

    fn read_state(&mut self) -> Result<(u8, bool), BackendError> {
        let sink = self.sink()?;
        Ok((channels_percent(&sink.volume), sink.mute))
    }

    fn set(&mut self, percent: u8) -> Result<u8, BackendError> {
        let sink = self.sink()?;
        let volume = with_percent(&sink.volume, percent);
        let conn = self.link.conn().ok_or(BackendError::NoEndpoint)?;
        if !conn.set_sink_volume(&sink.name, &volume) {
            return Err(BackendError::Os(
                "the audio server refused the volume".to_string(),
            ));
        }
        // Product rule: write, then re-read. A failed read-back is reported as such.
        let after = conn
            .sink(&sink.name)
            .ok_or(BackendError::ReadBackUnavailable)?;
        let state = (channels_percent(&after.volume), after.mute);
        self.master.own = Some((state, Instant::now()));
        Ok(state.0)
    }

    fn set_mute(&mut self, muted: bool) -> Result<bool, BackendError> {
        let sink = self.sink()?;
        let conn = self.link.conn().ok_or(BackendError::NoEndpoint)?;
        if !conn.set_sink_mute(&sink.name, muted) {
            return Err(BackendError::Os(
                "the audio server refused the mute".to_string(),
            ));
        }
        let after = conn
            .sink(&sink.name)
            .ok_or(BackendError::ReadBackUnavailable)?;
        self.master.own = Some((
            (channels_percent(&after.volume), after.mute),
            Instant::now(),
        ));
        Ok(after.mute)
    }

    /// Re-reads the default sink and publishes what changed (see the module docs).
    fn refresh(&mut self) {
        let Ok(sink) = self.sink() else {
            self.master.bound = None;
            return;
        };
        let state = (channels_percent(&sink.volume), sink.mute);
        let rebound = self.master.bound.as_deref() != Some(sink.name.as_str());
        let origin = if rebound {
            self.master.bound = Some(sink.name.clone());
            Some(ChangeOrigin::EndpointRebind)
        } else {
            classify_master(self.master.reported, self.master.own, state, Instant::now())
        };
        if let Some(origin) = origin {
            self.master.reported = Some(state);
            let _ = self.changes.send(VolumeChange {
                percent: state.0,
                muted: Some(state.1),
                origin,
            });
        }
    }

    /// The default sink's streams of `app`, or why there are none.
    fn sessions(&mut self, app: &str) -> Result<(Vec<SinkInput>, Sink), AppVolumeError> {
        let sink = self.sink().map_err(|e| match e {
            BackendError::NoEndpoint => AppVolumeError::NoSession,
            BackendError::Os(reason) => AppVolumeError::Os(reason),
            BackendError::ReadBackUnavailable => AppVolumeError::ReadBackUnavailable,
        })?;
        let conn = self.link.conn().ok_or(AppVolumeError::NoSession)?;
        let inputs = conn
            .sink_inputs()
            .ok_or_else(|| AppVolumeError::Os(UNREACHABLE.to_string()))?;
        let found: Vec<SinkInput> = inputs
            .into_iter()
            .filter(|i| i.sink == sink.index)
            .filter(|i| matches_app(app, i.binary.as_deref(), i.app_name.as_deref()))
            .collect();
        if found.is_empty() {
            Err(AppVolumeError::NoSession)
        } else {
            Ok((found, sink))
        }
    }

    fn app_read(&mut self, app: &str) -> Result<u8, AppVolumeError> {
        let (sessions, _) = self.sessions(app)?;
        Ok(channels_percent(&sessions[0].volume))
    }

    fn app_read_mute(&mut self, app: &str) -> Result<bool, AppVolumeError> {
        let (sessions, _) = self.sessions(app)?;
        Ok(sessions[0].mute)
    }

    /// Writes to every stream of `app`, then re-lists and requires every stream to agree.
    fn app_write<T: PartialEq>(
        &mut self,
        app: &str,
        write: impl Fn(&mut Conn, &SinkInput) -> bool,
        read: impl Fn(&SinkInput) -> T,
    ) -> Result<T, AppVolumeError> {
        let (sessions, _) = self.sessions(app)?;
        let conn = self.link.conn().ok_or(AppVolumeError::NoSession)?;
        for session in &sessions {
            if !write(conn, session) {
                return Err(AppVolumeError::Os(
                    "the audio server refused the change".to_string(),
                ));
            }
        }
        let indexes: Vec<u32> = sessions.iter().map(|s| s.index).collect();
        let after = conn
            .sink_inputs()
            .ok_or(AppVolumeError::ReadBackUnavailable)?;
        // A stream that vanished or a disagreement is not a confirmation.
        let values: Vec<T> = indexes
            .iter()
            .filter_map(|index| after.iter().find(|i| i.index == *index).map(&read))
            .collect();
        if values.len() != indexes.len() {
            return Err(AppVolumeError::ReadBackUnavailable);
        }
        agreed(values.into_iter()).ok_or(AppVolumeError::ReadBackUnavailable)
    }

    fn app_set(&mut self, app: &str, percent: u8) -> Result<u8, AppVolumeError> {
        self.app_write(
            app,
            |conn, session| {
                conn.set_sink_input_volume(session.index, &with_percent(&session.volume, percent))
            },
            |session| channels_percent(&session.volume),
        )
    }

    fn app_set_mute(&mut self, app: &str, muted: bool) -> Result<bool, AppVolumeError> {
        self.app_write(
            app,
            |conn, session| conn.set_sink_input_mute(session.index, muted),
            |session| session.mute,
        )
    }
}

/// Body of the dedicated `kivori-audio` OS thread. Owns the connection for its whole lifetime.
fn audio_thread(
    commands: &Receiver<Command>,
    changes: &Sender<VolumeChange>,
    health: &Arc<AtomicU8>,
    self_commands: &Sender<Command>,
    ready: &Sender<()>,
) {
    let events = self_commands.clone();
    let mut audio = Audio {
        link: Link::new(
            &[Event::Sink, Event::Server],
            // INGRESS ONLY: runs on libpulse's thread.
            Arc::new(move |_| {
                let _ = events.send(Command::Event);
            }),
        ),
        master: Master {
            bound: None,
            reported: None,
            own: None,
        },
        health: Arc::clone(health),
        changes: changes.clone(),
    };
    audio.conn();
    let _ = ready.send(());

    // When the first unread event arrived. Events are read after SETTLE of quiet, or MAX_COALESCE
    // after the first, so a long slider drag still updates.
    let mut dirty_since: Option<Instant> = None;
    loop {
        let received = if dirty_since.is_some() {
            commands.recv_timeout(SETTLE)
        } else if audio.link.is_down() {
            commands.recv_timeout(RECONNECT)
        } else {
            commands.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match received {
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(Command::Event) => {
                dirty_since.get_or_insert_with(Instant::now);
            }
            Ok(Command::Read(reply)) => {
                let _ = reply.send(audio.read_state().map(|(percent, _)| percent));
            }
            Ok(Command::Set(percent, reply)) => {
                let _ = reply.send(audio.set(percent));
            }
            Ok(Command::ReadMute(reply)) => {
                let _ = reply.send(audio.read_state().map(|(_, muted)| muted));
            }
            Ok(Command::SetMute(muted, reply)) => {
                let _ = reply.send(audio.set_mute(muted));
            }
            Ok(Command::AppRead(app, reply)) => {
                let _ = reply.send(audio.app_read(&app));
            }
            Ok(Command::AppSet(app, percent, reply)) => {
                let _ = reply.send(audio.app_set(&app, percent));
            }
            Ok(Command::AppReadMute(app, reply)) => {
                let _ = reply.send(audio.app_read_mute(&app));
            }
            Ok(Command::AppSetMute(app, muted, reply)) => {
                let _ = reply.send(audio.app_set_mute(&app, muted));
            }
            // Quiet: either the events have settled or it is time to retry the connection.
            Err(RecvTimeoutError::Timeout) => {
                audio.conn();
                if dirty_since.take().is_some() {
                    audio.refresh();
                }
            }
        }
        if dirty_since.is_some_and(|since| since.elapsed() >= MAX_COALESCE) {
            dirty_since = None;
            audio.refresh();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_app_matches_by_binary_file_name_or_application_name_exactly() {
        assert!(matches_app("spotify", Some("spotify"), Some("Spotify")));
        assert!(matches_app("vlc", Some("/usr/bin/vlc"), None));
        assert!(
            matches_app("Firefox", Some("firefox"), None),
            "id case is ignored"
        );
        assert!(matches_app("spotify", None, Some("Spotify")));
        assert!(matches_app("spotify", Some("electron"), Some("SPOTIFY")));
    }

    #[test]
    fn near_misses_and_empty_ids_match_nothing() {
        assert!(!matches_app("chrome", Some("chromium"), Some("Chromium")));
        assert!(!matches_app("code", Some("codeblocks"), None));
        assert!(
            !matches_app("fire", Some("firefox"), None),
            "no prefix matching"
        );
        assert!(!matches_app("", Some(""), Some("")));
        assert!(!matches_app("  ", None, None));
        assert!(!matches_app("a", None, None));
    }

    #[test]
    fn sessions_must_agree_to_confirm() {
        assert_eq!(agreed([40u8, 40, 40].into_iter()), Some(40));
        assert_eq!(agreed([40u8, 41].into_iter()), None);
        assert_eq!(agreed(std::iter::empty::<u8>()), None);
    }

    #[test]
    fn the_state_already_reported_publishes_nothing() {
        let now = Instant::now();
        assert_eq!(
            classify_master(Some((30, false)), None, (30, false), now),
            None
        );
    }

    #[test]
    fn the_echo_of_our_own_write_is_kivori_and_anything_else_is_external() {
        let now = Instant::now();
        let own = Some(((55, false), now));
        assert_eq!(
            classify_master(Some((30, false)), own, (55, false), now),
            Some(ChangeOrigin::Kivori)
        );
        // Someone else moved it to a different value.
        assert_eq!(
            classify_master(Some((30, false)), own, (70, false), now),
            Some(ChangeOrigin::External)
        );
        // Nothing of ours is pending.
        assert_eq!(
            classify_master(Some((30, false)), None, (55, false), now),
            Some(ChangeOrigin::External)
        );
    }

    #[test]
    fn an_old_write_is_not_the_echo_of_a_later_identical_change() {
        let long_ago = Instant::now();
        let later = long_ago + ECHO_WINDOW + Duration::from_millis(1);
        assert_eq!(
            classify_master(
                Some((30, false)),
                Some(((55, false), long_ago)),
                (55, false),
                later
            ),
            Some(ChangeOrigin::External)
        );
    }

    #[test]
    fn a_mute_toggle_is_a_change_too() {
        let now = Instant::now();
        assert_eq!(
            classify_master(Some((30, false)), None, (30, true), now),
            Some(ChangeOrigin::External)
        );
    }

    /// Real audio server, opt-in (mirrors `macos-audio-test`): moves the master and one app's
    /// volume and restores them. `cargo test -p kivori-desktop -- --ignored --nocapture linux_audio_round_trip`.
    #[test]
    #[ignore = "changes the live master volume (restored afterwards)"]
    fn linux_audio_round_trip() {
        let backend = LinuxVolumeBackend::new();
        println!("availability: {:?}", VolumeBackend::availability(&backend));
        let volume = VolumeBackend::read(&backend).expect("read master volume");
        let muted = VolumeBackend::read_mute(&backend).expect("read master mute");
        println!("master: {volume} % muted {muted}");

        let target = if volume >= 50 {
            volume - 10
        } else {
            volume + 10
        };
        let after = VolumeBackend::set(&backend, target).expect("set master volume");
        assert_eq!(after, target, "the read-back is what the server holds");
        assert_eq!(VolumeBackend::read(&backend), Ok(target));
        let after_mute = VolumeBackend::set_mute(&backend, !muted).expect("set master mute");
        assert_eq!(after_mute, !muted);

        // Restore, then the echoes of our own writes must have been classified.
        VolumeBackend::set_mute(&backend, muted).expect("restore mute");
        VolumeBackend::set(&backend, volume).expect("restore volume");
        std::thread::sleep(Duration::from_millis(400));
        let mut origins = Vec::new();
        while let Some(change) = backend.try_recv_change() {
            origins.push(change.origin);
        }
        println!("observed change origins: {origins:?}");
        assert!(
            origins.contains(&ChangeOrigin::EndpointRebind),
            "the initial bind is published"
        );
        assert!(
            !origins.contains(&ChangeOrigin::External),
            "our own writes are not external"
        );

        // An app with no stream is a NoSession, not an error.
        assert_eq!(
            AppVolumeBackend::read(&backend, "kivori-no-such-app"),
            Err(AppVolumeError::NoSession)
        );
    }
}
