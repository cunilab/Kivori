//! Media playback observation and media keys through MPRIS (`org.mpris.MediaPlayer2.*`) over the
//! session bus. It works on X11 and Wayland alike, and talks to the player directly, so a key sent
//! here needs no synthesized input.
//!
//! WHICH PLAYER: every `org.mpris.MediaPlayer2.*` name on the bus is a candidate. [`pick_player`]
//! prefers one that is Playing (the one that played most recently, when several are), then a
//! paused one (again the most recent), then any other. The observer polls once a second on its own
//! `kivori-media` thread and remembers which player last played; keys use the same memory.
//!
//! WHAT IS KNOWN: no player registered on the bus is `Stopped` (nothing is playing), a session bus
//! that cannot be reached is `None` (unknown). A player whose status Kivori does not recognise
//! ranks last, never guessed. A player that is registered but did not answer (hung, or too slow) is
//! NOT "no player": unless another player is seen Playing, the status is unknown (`None`), so an
//! unreadable player that is really playing never reads as Stopped. Titles and artists are never
//! logged (ADR-0005).
//!
//! DEADLINES: players are queried in parallel and the WHOLE discovery has one deadline (the key
//! request's own deadline for a key, [`OBSERVE_BUDGET`] for the observer), not one per player. A
//! key is only sent after the request's guard is checked again, immediately before the one
//! mutating D-Bus call.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kivori_model::desk::MediaStatus;
use zbus::blocking::fdo::{DBusProxy, PropertiesProxy};
use zbus::blocking::{Connection, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedValue, Value};

use crate::platform::{InputGuard, MediaKey, MediaObserver, NowPlaying};

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
const POLL_SLICES: u32 = 10;
const SLICE: Duration = Duration::from_millis(100);
/// A player that does not answer a call within this is skipped for the poll.
const CALL_TIMEOUT: Duration = Duration::from_millis(750);
/// The whole observer poll (discovery of every player) is bounded by this.
const OBSERVE_BUDGET: Duration = Duration::from_millis(1500);
/// The most a key request spends finding a player when its guard has no earlier deadline.
const SEND_BUDGET: Duration = Duration::from_secs(2);
/// More registered players than this are not queried (and count as unreadable).
const MAX_QUERIED: usize = 8;

/// What one player says, after mapping. `status` is `None` for a status Kivori does not know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlayerView {
    pub name: String,
    pub status: Option<MediaStatus>,
    pub title: String,
    pub artist: String,
}

/// `PlaybackStatus` -> ours. Anything else is unknown (never guessed as Stopped).
pub(super) fn map_status(status: &str) -> Option<MediaStatus> {
    match status {
        "Playing" => Some(MediaStatus::Playing),
        "Paused" => Some(MediaStatus::Paused),
        "Stopped" => Some(MediaStatus::Stopped),
        _ => None,
    }
}

/// Which player to observe or send a key to: an index into `players`.
///
/// Playing beats paused beats the rest; within a rank the player named `last_active` wins, else the
/// first listed. Players with an unknown status rank last of all.
pub(super) fn pick_player(players: &[PlayerView], last_active: Option<&str>) -> Option<usize> {
    let rank = |view: &PlayerView| match view.status {
        Some(MediaStatus::Playing) => 0,
        Some(MediaStatus::Paused) => 1,
        Some(MediaStatus::Stopped) => 2,
        None => 3,
    };
    players
        .iter()
        .enumerate()
        .min_by_key(|(_, view)| (rank(view), last_active != Some(view.name.as_str())))
        .map(|(index, _)| index)
}

/// What is playing, kept consistent with the status: nothing when the status is unknown or Stopped,
/// or when the player gives neither a title nor an artist (unknown, not "untitled").
pub(super) fn now_playing(
    status: Option<MediaStatus>,
    title: &str,
    artist: &str,
) -> Option<NowPlaying> {
    let active = matches!(status, Some(MediaStatus::Playing | MediaStatus::Paused));
    (active && (!title.is_empty() || !artist.is_empty())).then(|| NowPlaying {
        title: title.to_string(),
        artist: artist.to_string(),
    })
}

/// `xesam:title` and `xesam:artist` from a Metadata dictionary. The spec makes the artist a list of
/// strings, but some players send one string; both are accepted, joined with ", ".
pub(super) fn title_and_artist(metadata: &HashMap<String, OwnedValue>) -> (String, String) {
    let text = |value: &Value<'_>| match value {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
    };
    let title = metadata
        .get("xesam:title")
        .and_then(|v| text(v))
        .unwrap_or_default();
    let artist = match metadata.get("xesam:artist").map(|v| &**v) {
        Some(Value::Array(artists)) => artists
            .iter()
            .filter_map(&text)
            .filter(|a| !a.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        Some(other) => text(other).unwrap_or_default(),
        None => String::new(),
    };
    (title.trim().to_string(), artist.trim().to_string())
}

fn player_proxy<'a>(conn: &Connection, name: &'a str) -> zbus::Result<Proxy<'a>> {
    zbus::blocking::proxy::Builder::new(conn)
        .destination(name)?
        .path(MPRIS_PATH)?
        .interface(PLAYER_IFACE)?
        .cache_properties(CacheProperties::No)
        .build()
}

/// How sending a media key to a player ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Sent {
    Yes,
    /// The session bus is unreachable or no player is registered on it: nothing was sent, so
    /// another way (the X11 key event) may be tried.
    NoPlayer,
    /// A player was found but refused the call: nothing the key event could fix.
    Failed(String),
    /// A player was found and did not answer in time: it may yet act, so nothing else is tried.
    Unknown,
    /// The request's guard or deadline ran out before the call: nothing was sent.
    Expired,
}

/// What talks to the session bus. A trait so tests drive discovery and sending with fakes (slow,
/// hung, unreadable players) instead of a real bus.
pub(super) trait Bus: Send + Sync + 'static {
    /// The registered MPRIS player names. `Err`: the bus cannot be talked to.
    fn names(&self) -> Result<Vec<String>, ()>;
    /// One player's state, `None` when it does not answer (gone, hung, or not a real player).
    /// Blocking is bounded by [`CALL_TIMEOUT`].
    fn view(&self, name: &str) -> Option<PlayerView>;
    /// Calls a no-argument `Player` method (`PlayPause`, `Next`, `Previous`).
    fn call(&self, name: &str, method: &str) -> Sent;
}

/// The real session bus: one connection, opened on first use and rebuilt after an error.
struct SessionBus {
    conn: Mutex<Option<Connection>>,
}

impl SessionBus {
    fn connection(&self) -> Option<Connection> {
        let mut slot = self.conn.lock().expect("mpris connection mutex");
        if slot.is_none() {
            *slot = zbus::blocking::connection::Builder::session()
                .and_then(|builder| builder.method_timeout(CALL_TIMEOUT).build())
                .ok();
        }
        slot.clone()
    }

    fn reset(&self) {
        self.conn.lock().expect("mpris connection mutex").take();
    }
}

impl Bus for SessionBus {
    fn names(&self) -> Result<Vec<String>, ()> {
        let conn = self.connection().ok_or(())?;
        let names = DBusProxy::new(&conn)
            .and_then(|dbus| dbus.list_names().map_err(zbus::Error::from))
            .map_err(|_| self.reset())?;
        Ok(names
            .iter()
            .map(|n| n.as_str().to_string())
            .filter(|n| n.starts_with(MPRIS_PREFIX))
            .collect())
    }

    fn view(&self, name: &str) -> Option<PlayerView> {
        let conn = self.connection()?;
        let properties = PropertiesProxy::builder(&conn)
            .destination(name)
            .ok()?
            .path(MPRIS_PATH)
            .ok()?
            .cache_properties(CacheProperties::No)
            .build()
            .ok()?;
        let all = properties.get_all(PLAYER_IFACE.try_into().ok()?).ok()?;
        let status = all
            .get("PlaybackStatus")
            .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
            .and_then(|s| map_status(&s));
        let metadata = all
            .get("Metadata")
            .and_then(|v| HashMap::<String, OwnedValue>::try_from(v.try_clone().ok()?).ok())
            .unwrap_or_default();
        let (title, artist) = title_and_artist(&metadata);
        Some(PlayerView {
            name: name.to_string(),
            status,
            title,
            artist,
        })
    }

    fn call(&self, name: &str, method: &str) -> Sent {
        let Some(conn) = self.connection() else {
            return Sent::NoPlayer;
        };
        let result = player_proxy(&conn, name).and_then(|player| player.call_method(method, &()));
        match result {
            Ok(_) => Sent::Yes,
            Err(zbus::Error::InputOutput(_)) => {
                self.reset();
                Sent::Unknown
            }
            // A timeout surfaces as a D-Bus "NoReply" error from zbus's method timeout.
            Err(zbus::Error::MethodError(name, _, _)) if name.as_str().ends_with(".NoReply") => {
                Sent::Unknown
            }
            Err(error) => Sent::Failed(error.to_string()),
        }
    }
}

/// What looking at the bus found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Discovery {
    /// The bus cannot be talked to: unknown.
    BusDown,
    /// The bus answered and no player is registered on it.
    NoPlayers,
    /// Players are registered. `unreadable` more did not answer in time (or at all).
    Found {
        players: Vec<PlayerView>,
        unreadable: usize,
    },
}

/// The observer's answer for what [`Discovery`] found. A playing player is Playing whatever else is
/// on the bus. Otherwise an unreadable player might be the one playing, so the status is unknown,
/// not that of the readable ones.
pub(super) fn observed(
    players: &[PlayerView],
    unreadable: usize,
    last_active: Option<&str>,
) -> (Option<MediaStatus>, Option<NowPlaying>, Option<String>) {
    let picked = pick_player(players, last_active).map(|index| &players[index]);
    match picked {
        Some(view) if view.status == Some(MediaStatus::Playing) => (
            view.status,
            now_playing(view.status, &view.title, &view.artist),
            Some(view.name.clone()),
        ),
        _ if unreadable > 0 => (None, None, None),
        Some(view) => (
            view.status,
            now_playing(view.status, &view.title, &view.artist),
            None,
        ),
        None => (None, None, None),
    }
}

/// A MPRIS client: the bus, and the memory of which player played last. Shared by the observer
/// and the media keys.
pub struct Mpris {
    bus: Arc<dyn Bus>,
    last_active: Mutex<Option<String>>,
}

impl Mpris {
    #[must_use]
    pub fn new() -> Self {
        Self::with_bus(Arc::new(SessionBus {
            conn: Mutex::new(None),
        }))
    }

    pub(super) fn with_bus(bus: Arc<dyn Bus>) -> Self {
        Self {
            bus,
            last_active: Mutex::new(None),
        }
    }

    /// Every registered player with its mapped state, all queried in parallel under ONE `deadline`:
    /// a player that has not answered by then is counted as unreadable (its query ends on its own
    /// by [`CALL_TIMEOUT`]; nothing waits for it).
    pub(super) fn discover(&self, deadline: Instant) -> Discovery {
        let Ok(names) = self.bus.names() else {
            return Discovery::BusDown;
        };
        if names.is_empty() {
            return Discovery::NoPlayers;
        }
        let total = names.len();
        let (tx, rx) = mpsc::channel::<(usize, Option<PlayerView>)>();
        let mut asked = 0;
        for (index, name) in names.into_iter().take(MAX_QUERIED).enumerate() {
            let (bus, tx) = (Arc::clone(&self.bus), tx.clone());
            let spawned = std::thread::Builder::new()
                .name("kivori-mpris-query".to_string())
                .spawn(move || {
                    let _ = tx.send((index, bus.view(&name)));
                });
            if spawned.is_ok() {
                asked += 1;
            }
        }
        drop(tx);
        let mut answers: Vec<(usize, PlayerView)> = Vec::new();
        for _ in 0..asked {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok((index, Some(view))) => answers.push((index, view)),
                Ok((_, None)) => {}
                Err(_) => break,
            }
        }
        answers.sort_by_key(|(index, _)| *index);
        let players: Vec<PlayerView> = answers.into_iter().map(|(_, view)| view).collect();
        let unreadable = total - players.len();
        Discovery::Found {
            players,
            unreadable,
        }
    }

    /// The observer's view: `(status, now playing)`. `(None, None)` when the bus cannot be reached
    /// or players are present but unreadable.
    fn observe(&self) -> (Option<MediaStatus>, Option<NowPlaying>) {
        let discovery = self.discover(Instant::now() + OBSERVE_BUDGET);
        let (status, now, playing) = match discovery {
            Discovery::BusDown => return (None, None),
            // The bus is fine and no player is registered: nothing is playing.
            Discovery::NoPlayers => return (Some(MediaStatus::Stopped), None),
            Discovery::Found {
                players,
                unreadable,
            } => {
                let last = self.last_active.lock().expect("mpris last-active mutex");
                observed(&players, unreadable, last.as_deref())
            }
        };
        if let Some(name) = playing {
            *self.last_active.lock().expect("mpris last-active mutex") = Some(name);
        }
        (status, now)
    }

    /// Sends `key` to the player [`pick_player`] chooses. The whole search shares the request's
    /// deadline, and the guard is checked again immediately before the call.
    pub(super) fn send_key(&self, key: MediaKey, guard: &InputGuard) -> Sent {
        let ceiling = Instant::now() + SEND_BUDGET;
        let deadline = guard.deadline.map_or(ceiling, |d| d.min(ceiling));
        let players = match self.discover(deadline) {
            Discovery::BusDown | Discovery::NoPlayers => return Sent::NoPlayer,
            Discovery::Found { players, .. } => players,
        };
        let last = self
            .last_active
            .lock()
            .expect("mpris last-active mutex")
            .clone();
        let Some(index) = pick_player(&players, last.as_deref()) else {
            return Sent::Failed("no media player answered".to_string());
        };
        let method = match key {
            MediaKey::PlayPause => "PlayPause",
            MediaKey::Next => "Next",
            MediaKey::Previous => "Previous",
        };
        // The last look before the one mutating call: a request that has expired (or was replaced)
        // while the players were being found must not act.
        if guard.check().is_err() || Instant::now() >= deadline {
            return Sent::Expired;
        }
        self.bus.call(&players[index].name, method)
    }
}

impl Default for Mpris {
    fn default() -> Self {
        Self::new()
    }
}

type View = (Option<MediaStatus>, Option<NowPlaying>);
type Shared = Arc<Mutex<View>>;

/// Observes playback through MPRIS on a `kivori-media` thread that stops when this is dropped.
pub struct LinuxMediaObserver {
    view: Shared,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl LinuxMediaObserver {
    #[must_use]
    pub fn new(mpris: Arc<Mpris>) -> Self {
        let view: Shared = Arc::new(Mutex::new((None, None)));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_view, thread_stop) = (Arc::clone(&view), Arc::clone(&stop));
        let thread = std::thread::Builder::new()
            .name("kivori-media".to_string())
            .spawn(move || poll_loop(&mpris, &thread_view, &thread_stop))
            .ok();
        // A failed spawn leaves the status at `None`: unknown, not a panic.
        Self { view, stop, thread }
    }
}

impl Drop for LinuxMediaObserver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl MediaObserver for LinuxMediaObserver {
    fn status(&self) -> Option<MediaStatus> {
        self.view.lock().expect("media view mutex").0
    }

    fn now_playing(&self) -> Option<NowPlaying> {
        self.view.lock().expect("media view mutex").1.clone()
    }
}

fn poll_loop(mpris: &Mpris, shared: &Shared, stop: &AtomicBool) {
    while !stop.load(Ordering::SeqCst) {
        let next = mpris.observe();
        *shared.lock().expect("media view mutex") = next;
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

    fn view(name: &str, status: Option<MediaStatus>) -> PlayerView {
        PlayerView {
            name: name.to_string(),
            status,
            title: String::new(),
            artist: String::new(),
        }
    }

    fn dict(entries: &[(&str, Value<'_>)]) -> HashMap<String, OwnedValue> {
        entries
            .iter()
            .map(|(k, v)| {
                (
                    (*k).to_string(),
                    OwnedValue::try_from(v.try_clone().unwrap()).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn the_three_mpris_statuses_map_and_others_are_unknown() {
        assert_eq!(map_status("Playing"), Some(MediaStatus::Playing));
        assert_eq!(map_status("Paused"), Some(MediaStatus::Paused));
        assert_eq!(map_status("Stopped"), Some(MediaStatus::Stopped));
        assert_eq!(map_status("Buffering"), None, "never guessed");
        assert_eq!(map_status(""), None);
    }

    #[test]
    fn metadata_gives_title_and_a_joined_artist_list() {
        let metadata = dict(&[
            ("xesam:title", Value::from("Song")),
            (
                "xesam:artist",
                Value::from(vec!["A".to_string(), "B".to_string()]),
            ),
            ("xesam:album", Value::from("Ignored")),
        ]);
        assert_eq!(
            title_and_artist(&metadata),
            ("Song".to_string(), "A, B".to_string())
        );
    }

    #[test]
    fn a_player_that_sends_one_artist_string_is_accepted() {
        let metadata = dict(&[
            ("xesam:title", Value::from("  Song ")),
            ("xesam:artist", Value::from("Solo")),
        ]);
        assert_eq!(
            title_and_artist(&metadata),
            ("Song".to_string(), "Solo".to_string())
        );
    }

    #[test]
    fn missing_or_wrongly_typed_fields_are_empty_not_invented() {
        assert_eq!(
            title_and_artist(&HashMap::new()),
            (String::new(), String::new())
        );
        let metadata = dict(&[
            ("xesam:title", Value::from(7_u32)),
            ("xesam:artist", Value::from(vec![String::new()])),
        ]);
        assert_eq!(title_and_artist(&metadata), (String::new(), String::new()));
    }

    #[test]
    fn now_playing_follows_the_status() {
        let both = Some(NowPlaying {
            title: "T".to_string(),
            artist: "A".to_string(),
        });
        for active in [MediaStatus::Playing, MediaStatus::Paused] {
            assert_eq!(now_playing(Some(active), "T", "A"), both);
            assert_eq!(
                now_playing(Some(active), "", ""),
                None,
                "unknown, not untitled"
            );
        }
        assert_eq!(now_playing(Some(MediaStatus::Stopped), "T", "A"), None);
        assert_eq!(now_playing(None, "T", "A"), None);
    }

    #[test]
    fn a_playing_player_beats_a_paused_one() {
        let players = [
            view("a", Some(MediaStatus::Paused)),
            view("b", Some(MediaStatus::Playing)),
            view("c", Some(MediaStatus::Stopped)),
        ];
        assert_eq!(pick_player(&players, Some("a")), Some(1));
        assert_eq!(pick_player(&players, None), Some(1));
    }

    #[test]
    fn among_equals_the_last_active_player_wins_then_the_first_listed() {
        let players = [
            view("a", Some(MediaStatus::Playing)),
            view("b", Some(MediaStatus::Playing)),
        ];
        assert_eq!(pick_player(&players, Some("b")), Some(1));
        assert_eq!(pick_player(&players, Some("gone")), Some(0));
        assert_eq!(pick_player(&players, None), Some(0));
        let paused = [
            view("a", Some(MediaStatus::Paused)),
            view("b", Some(MediaStatus::Paused)),
        ];
        assert_eq!(pick_player(&paused, Some("b")), Some(1));
    }

    #[test]
    fn unknown_status_ranks_last_and_no_players_picks_none() {
        let players = [view("a", None), view("b", Some(MediaStatus::Stopped))];
        assert_eq!(pick_player(&players, Some("a")), Some(1));
        assert_eq!(pick_player(&[], None), None);
    }

    /// A bus whose players answer after a delay (or never), recording every call.
    struct FakeBus {
        names: Result<Vec<String>, ()>,
        /// `(delay, view)` per player name; `None` view is an unreadable player.
        answers: HashMap<String, (Duration, Option<MediaStatus>, bool)>,
        calls: Mutex<Vec<(String, String)>>,
    }

    impl FakeBus {
        fn new(players: &[(&str, u64, Option<MediaStatus>, bool)]) -> Arc<Self> {
            Arc::new(Self {
                names: Ok(players.iter().map(|p| p.0.to_string()).collect()),
                answers: players
                    .iter()
                    .map(|(n, ms, status, readable)| {
                        (
                            (*n).to_string(),
                            (Duration::from_millis(*ms), *status, *readable),
                        )
                    })
                    .collect(),
                calls: Mutex::default(),
            })
        }
    }

    impl Bus for FakeBus {
        fn names(&self) -> Result<Vec<String>, ()> {
            self.names.clone()
        }

        fn view(&self, name: &str) -> Option<PlayerView> {
            let (delay, status, readable) = *self.answers.get(name)?;
            std::thread::sleep(delay);
            readable.then(|| view(name, status))
        }

        fn call(&self, name: &str, method: &str) -> Sent {
            self.calls
                .lock()
                .unwrap()
                .push((name.to_string(), method.to_string()));
            Sent::Yes
        }
    }

    fn guard_until(deadline: Option<Instant>, current: bool) -> InputGuard {
        InputGuard {
            deadline,
            current: Arc::new(move || current),
        }
    }

    #[test]
    fn discovery_has_one_deadline_for_all_players_not_one_each() {
        // Four players at 400 ms each would take 1.6 s one after another.
        let bus = FakeBus::new(&[
            (
                "org.mpris.MediaPlayer2.a",
                400,
                Some(MediaStatus::Paused),
                true,
            ),
            (
                "org.mpris.MediaPlayer2.b",
                400,
                Some(MediaStatus::Paused),
                true,
            ),
            (
                "org.mpris.MediaPlayer2.c",
                400,
                Some(MediaStatus::Paused),
                true,
            ),
            (
                "org.mpris.MediaPlayer2.d",
                400,
                Some(MediaStatus::Paused),
                true,
            ),
        ]);
        let mpris = Mpris::with_bus(bus);
        let started = Instant::now();
        let found = mpris.discover(Instant::now() + Duration::from_millis(150));
        assert!(
            started.elapsed() < Duration::from_millis(350),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(
            found,
            Discovery::Found {
                players: vec![],
                unreadable: 4
            },
            "players that missed the deadline are unreadable"
        );
    }

    #[test]
    fn a_key_whose_deadline_passes_during_discovery_sends_nothing() {
        let bus = FakeBus::new(&[(
            "org.mpris.MediaPlayer2.slow",
            300,
            Some(MediaStatus::Playing),
            true,
        )]);
        let mpris = Mpris::with_bus(bus.clone());
        let started = Instant::now();
        let guard = guard_until(Some(Instant::now() + Duration::from_millis(100)), true);
        let sent = mpris.send_key(MediaKey::PlayPause, &guard);
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "{:?}",
            started.elapsed()
        );
        // Nothing answered in time, so no player was chosen and nothing was called.
        assert!(matches!(sent, Sent::Failed(_) | Sent::Expired), "{sent:?}");
        assert!(bus.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_player_that_answers_just_before_the_deadline_is_still_not_called_after_it() {
        let bus = FakeBus::new(&[(
            "org.mpris.MediaPlayer2.p",
            60,
            Some(MediaStatus::Playing),
            true,
        )]);
        let mpris = Mpris::with_bus(bus.clone());
        // The guard's deadline is later than discovery but the request was replaced meanwhile.
        let replaced = guard_until(Some(Instant::now() + Duration::from_secs(5)), false);
        assert_eq!(mpris.send_key(MediaKey::Next, &replaced), Sent::Expired);
        assert!(bus.calls.lock().unwrap().is_empty());

        let current = guard_until(Some(Instant::now() + Duration::from_secs(5)), true);
        assert_eq!(mpris.send_key(MediaKey::Next, &current), Sent::Yes);
        assert_eq!(
            *bus.calls.lock().unwrap(),
            [("org.mpris.MediaPlayer2.p".to_string(), "Next".to_string())]
        );
    }

    #[test]
    fn no_registered_player_is_stopped_and_the_key_may_fall_back() {
        let mpris = Mpris::with_bus(FakeBus::new(&[]));
        assert_eq!(mpris.observe(), (Some(MediaStatus::Stopped), None));
        assert_eq!(
            mpris.send_key(MediaKey::Next, &guard_until(None, true)),
            Sent::NoPlayer
        );
    }

    #[test]
    fn an_unreachable_bus_is_unknown_not_stopped() {
        let bus = Arc::new(FakeBus {
            names: Err(()),
            answers: HashMap::new(),
            calls: Mutex::default(),
        });
        let mpris = Mpris::with_bus(bus);
        assert_eq!(mpris.observe(), (None, None));
    }

    #[test]
    fn a_registered_but_unreadable_player_is_unknown_not_stopped() {
        let mpris = Mpris::with_bus(FakeBus::new(&[(
            "org.mpris.MediaPlayer2.hung",
            0,
            None,
            false,
        )]));
        assert_eq!(mpris.observe(), (None, None), "failed discovery is unknown");
        // And a key is never sent to a bus whose only player could not be read, nor falls back.
        let sent = mpris.send_key(MediaKey::PlayPause, &guard_until(None, true));
        assert!(matches!(sent, Sent::Failed(_)), "{sent:?}");
    }

    #[test]
    fn a_readable_playing_player_wins_over_an_unreadable_one() {
        let mpris = Mpris::with_bus(FakeBus::new(&[
            ("org.mpris.MediaPlayer2.hung", 0, None, false),
            (
                "org.mpris.MediaPlayer2.music",
                0,
                Some(MediaStatus::Playing),
                true,
            ),
        ]));
        assert_eq!(mpris.observe().0, Some(MediaStatus::Playing));
    }

    #[test]
    fn readable_non_playing_players_do_not_hide_an_unreadable_one() {
        let paused = [view("a", Some(MediaStatus::Paused))];
        assert_eq!(
            observed(&paused, 1, None).0,
            None,
            "the hung one may be playing"
        );
        assert_eq!(observed(&paused, 0, None).0, Some(MediaStatus::Paused));
        assert_eq!(observed(&[], 0, None).0, None);
    }

    /// Real session bus: `cargo test -p kivori-desktop -- --ignored --nocapture mpris_now`.
    #[test]
    #[ignore = "reads the live MPRIS players"]
    fn mpris_now() {
        let mpris = Mpris::new();
        match mpris.discover(Instant::now() + OBSERVE_BUDGET) {
            Discovery::Found {
                players,
                unreadable,
            } => {
                for p in &players {
                    println!("player {} status {:?}", p.name, p.status);
                }
                println!("{unreadable} unreadable");
            }
            other => println!("{other:?}"),
        }
        println!("observe: {:?}", mpris.observe().0);
    }
}
