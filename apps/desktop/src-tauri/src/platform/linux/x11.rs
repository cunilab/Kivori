//! The X11 side of Linux: ONE background worker that owns the X connection and caches what it last
//! saw, so nothing on the device thread (or an action worker) ever waits on an X server.
//!
//! WHY A WORKER: connecting to X and every property reply are blocking round trips. A hung or
//! suspended X server (a frozen compositor, a login on another VT) would stall the device thread
//! that pumps the serial port. The worker takes that wait; readers only look at the cache.
//!
//! WHAT IS CACHED: the executable behind the focused window and whether the server is reachable with
//! the XTEST extension (the key-injection path). Each reading carries the time it was taken; one
//! older than [`STALE`] is no reading at all, so a worker stuck in a call makes the focus `Unknown`
//! and the shortcuts unavailable, never a stale guess. The worker keeps one connection and drops it
//! on any error, so a restarted X server is picked up on the next poll.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

use super::proc::exe_name;

/// How often the worker looks (the device thread asks for focus a few times a second).
const POLL: Duration = Duration::from_millis(250);
/// A reading older than this is not trusted.
pub(super) const STALE: Duration = Duration::from_secs(2);
/// How long the first reading is waited for when the worker starts, so availability is right the
/// moment the app is up without a hung server being able to hold startup for long.
const FIRST_READING_WAIT: Duration = Duration::from_millis(750);
/// How long dropping a watch waits for its worker before detaching it.
const JOIN: Duration = Duration::from_millis(500);

/// One look at the X server.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Sample {
    /// The executable behind the focused window; `None` when nothing is focused, the owner is
    /// unknown, or the server is unreachable.
    pub exe: Option<String>,
    /// The server answered AND has the XTEST extension: keys can be injected.
    pub xtest: bool,
}

/// What looks at the X server. Blocking is allowed: it only ever runs on the worker.
pub(super) trait Probe: Send + 'static {
    fn poll(&mut self) -> Sample;
}

/// Whether X11 key injection works right now. Injected so tests need no X server.
pub(super) trait XHealth: Send + Sync {
    /// `Some(true)`: reachable with XTEST. `Some(false)`: not. `None`: no recent answer (the
    /// server is not responding, or has not been asked yet).
    fn xtest(&self) -> Option<bool>;
}

/// The latest sample and when it was taken.
type Reading = Option<(Sample, Instant)>;

/// The background worker and its cache.
pub(super) struct X11Watch {
    latest: Arc<Mutex<Reading>>,
    stale: Duration,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl X11Watch {
    /// The watch of this process's X server, started on first use and kept for the process life.
    pub fn shared() -> Arc<Self> {
        static SHARED: OnceLock<Arc<X11Watch>> = OnceLock::new();
        Arc::clone(SHARED.get_or_init(|| {
            let watch = Arc::new(Self::start(XProbe::default(), POLL, STALE));
            watch.wait_first(FIRST_READING_WAIT);
            watch
        }))
    }

    /// Starts a worker polling `probe` every `every`; readings older than `stale` are ignored.
    pub fn start(mut probe: impl Probe, every: Duration, stale: Duration) -> Self {
        let latest: Arc<Mutex<Reading>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (worker_latest, worker_stop) = (Arc::clone(&latest), Arc::clone(&stop));
        let thread = std::thread::Builder::new()
            .name("kivori-x11".to_string())
            .spawn(move || {
                while !worker_stop.load(Ordering::SeqCst) {
                    let sample = probe.poll();
                    *worker_latest.lock().unwrap_or_else(PoisonError::into_inner) =
                        Some((sample, Instant::now()));
                    let until = Instant::now() + every;
                    while Instant::now() < until && !worker_stop.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(10).min(every));
                    }
                }
            })
            .ok();
        Self {
            latest,
            stale,
            stop,
            thread: Mutex::new(thread),
        }
    }

    fn wait_first(&self, within: Duration) {
        let deadline = Instant::now() + within;
        while self.fresh().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The latest sample if it is recent enough to trust. Never blocks on the server.
    fn fresh(&self) -> Option<Sample> {
        let reading = self.latest.lock().unwrap_or_else(PoisonError::into_inner);
        reading
            .as_ref()
            .filter(|(_, at)| at.elapsed() <= self.stale)
            .map(|(sample, _)| sample.clone())
    }

    /// The focused executable: `None` when there is no recent reading (unknown), `Some(None)` when
    /// the reading says nothing is focused or the owner is unknown.
    pub fn focused_exe(&self) -> Option<Option<String>> {
        self.fresh().map(|sample| sample.exe)
    }
}

impl XHealth for X11Watch {
    fn xtest(&self) -> Option<bool> {
        self.fresh().map(|sample| sample.xtest)
    }
}

impl Drop for X11Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let thread = self
            .thread
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            // A worker stuck inside a blocking X call is detached rather than waited on.
            super::pulse::join_within(thread, JOIN);
        }
    }
}

struct X11 {
    conn: RustConnection,
    root: Window,
    active_window: Atom,
    wm_pid: Atom,
    xtest: bool,
}

impl X11 {
    fn connect() -> Option<Self> {
        let (conn, screen) = RustConnection::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let atom = |name: &[u8]| -> Option<Atom> {
            Some(conn.intern_atom(false, name).ok()?.reply().ok()?.atom)
        };
        let active_window = atom(b"_NET_ACTIVE_WINDOW")?;
        let wm_pid = atom(b"_NET_WM_PID")?;
        let xtest = conn.query_extension(b"XTEST").ok()?.reply().ok()?.present;
        Some(Self {
            conn,
            root,
            active_window,
            wm_pid,
            xtest,
        })
    }

    /// The first 32-bit value of a property, `None` when absent or not that format.
    fn cardinal(&self, window: Window, property: Atom, kind: AtomEnum) -> Result<Option<u32>, ()> {
        let reply = self
            .conn
            .get_property(false, window, property, kind, 0, 1)
            .map_err(drop)?
            .reply()
            .map_err(drop)?;
        Ok(reply.value32().and_then(|mut values| values.next()))
    }

    /// The pid of the focused window's owner. `Ok(None)`: nothing is focused, or the window does not
    /// say which process owns it. `Err`: the connection failed and must be rebuilt.
    fn focused_pid(&self) -> Result<Option<u32>, ()> {
        let Some(window) = self
            .cardinal(self.root, self.active_window, AtomEnum::WINDOW)?
            .filter(|window| *window != 0)
        else {
            return Ok(None);
        };
        self.cardinal(window, self.wm_pid, AtomEnum::CARDINAL)
    }
}

/// The real probe: one X connection, dropped on any error.
#[derive(Default)]
struct XProbe {
    x11: Option<X11>,
}

impl Probe for XProbe {
    fn poll(&mut self) -> Sample {
        if self.x11.is_none() {
            self.x11 = X11::connect();
        }
        let Some(x11) = self.x11.as_ref() else {
            return Sample::default();
        };
        let xtest = x11.xtest;
        match x11.focused_pid() {
            Ok(pid) => Sample {
                exe: pid.and_then(exe_name),
                xtest,
            },
            Err(()) => {
                self.x11 = None;
                Sample::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Answers from a script; once it runs out it blocks forever, like a hung X server.
    struct Scripted(mpsc::Receiver<Sample>);

    impl Probe for Scripted {
        fn poll(&mut self) -> Sample {
            // Blocks until the test sends a sample (or the test drops the sender: then forever).
            self.0.recv().unwrap_or_else(|_| {
                std::thread::sleep(Duration::from_secs(3600));
                Sample::default()
            })
        }
    }

    fn app(exe: &str, xtest: bool) -> Sample {
        Sample {
            exe: Some(exe.to_string()),
            xtest,
        }
    }

    fn wait_for(mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !done() {
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn before_any_reading_focus_and_health_are_unknown_without_blocking() {
        let (_tx, rx) = mpsc::channel();
        let watch = X11Watch::start(Scripted(rx), Duration::from_millis(10), STALE);
        let t = Instant::now();
        assert_eq!(watch.focused_exe(), None);
        assert_eq!(watch.xtest(), None);
        assert!(t.elapsed() < Duration::from_millis(50), "the probe is hung");
    }

    #[test]
    fn a_fresh_reading_is_served_from_the_cache() {
        let (tx, rx) = mpsc::channel();
        let watch = X11Watch::start(Scripted(rx), Duration::from_millis(10), STALE);
        tx.send(app("firefox", true)).unwrap();
        wait_for(|| watch.focused_exe().is_some());
        assert_eq!(watch.focused_exe(), Some(Some("firefox".to_string())));
        assert_eq!(watch.xtest(), Some(true));
    }

    #[test]
    fn a_reading_that_ages_out_while_the_server_hangs_is_unknown_again() {
        let (tx, rx) = mpsc::channel();
        let watch = X11Watch::start(
            Scripted(rx),
            Duration::from_millis(10),
            Duration::from_millis(150),
        );
        tx.send(app("firefox", true)).unwrap();
        wait_for(|| watch.focused_exe().is_some());
        // The probe now hangs on its next poll: nothing refreshes the cache.
        std::thread::sleep(Duration::from_millis(300));
        let t = Instant::now();
        assert_eq!(watch.focused_exe(), None, "stale is unknown, not firefox");
        assert_eq!(watch.xtest(), None, "and shortcuts are not vouched for");
        assert!(
            t.elapsed() < Duration::from_millis(50),
            "readers never wait"
        );
    }

    #[test]
    fn an_unreachable_server_is_a_fresh_negative_not_unknown() {
        let (tx, rx) = mpsc::channel();
        let watch = X11Watch::start(Scripted(rx), Duration::from_millis(10), STALE);
        tx.send(Sample::default()).unwrap();
        wait_for(|| watch.xtest().is_some());
        assert_eq!(watch.xtest(), Some(false));
        assert_eq!(watch.focused_exe(), Some(None));
    }

    #[test]
    fn dropping_a_watch_whose_worker_is_hung_does_not_hang() {
        let (_tx, rx) = mpsc::channel();
        let watch = X11Watch::start(Scripted(rx), Duration::from_millis(10), STALE);
        std::thread::sleep(Duration::from_millis(50));
        let t = Instant::now();
        drop(watch);
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
    }
}
