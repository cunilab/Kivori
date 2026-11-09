//! Linux focused app, on X11: the root window's `_NET_ACTIVE_WINDOW`, that window's `_NET_WM_PID`,
//! and the executable behind it. The decision itself is `platform::foreground::classify_linux`.
//!
//! WAYLAND: there is no protocol for a client to ask which window has focus (that is the point of
//! Wayland), and XWayland's `_NET_ACTIVE_WINDOW` only follows X11 clients, so it would show the
//! last XWayland app while a native one has focus. That is a wrong answer, not an unknown one, so
//! on a Wayland session the foreground is always `Unknown` and only the General profile applies.
//!
//! The X work is done by the background worker in `x11` (one connection, a cache with a freshness
//! limit), never on the caller: `foreground()` is called from the device thread and the action
//! workers, and an unresponsive X server must not stall them. It returns the cached value, and
//! `Unknown` when the cache is stale (the server stopped answering).

use std::sync::Arc;

use super::host_events::screen_locked;
use super::session::{session_kind, SessionKind};
use super::x11::X11Watch;
use crate::platform::foreground::classify_linux;
use crate::platform::{Foreground, ForegroundObserver};

/// Foreground observer for Linux.
pub struct LinuxForeground {
    /// The X worker's cache; `None` outside an X11 session (nothing to ask).
    watch: Option<Arc<X11Watch>>,
}

impl LinuxForeground {
    #[must_use]
    pub fn new() -> Self {
        Self::for_session(session_kind())
    }

    fn for_session(session: SessionKind) -> Self {
        Self {
            watch: (session == SessionKind::X11).then(X11Watch::shared),
        }
    }

    #[cfg(test)]
    fn with_watch(watch: Arc<X11Watch>) -> Self {
        Self { watch: Some(watch) }
    }
}

impl Default for LinuxForeground {
    fn default() -> Self {
        Self::new()
    }
}

impl ForegroundObserver for LinuxForeground {
    fn foreground(&self) -> Foreground {
        let Some(watch) = &self.watch else {
            return Foreground::Unknown;
        };
        // A locked screen is Protected whatever was focused, and needs no cache.
        let locked = screen_locked();
        // No recent reading is no executable: `Unknown`.
        let exe = if locked {
            None
        } else {
            watch.focused_exe().flatten()
        };
        classify_linux(exe.as_deref(), locked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::linux::x11::{Probe, Sample};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    /// Answers from a script, then hangs forever like an unresponsive X server.
    struct Scripted(mpsc::Receiver<Sample>);

    impl Probe for Scripted {
        fn poll(&mut self) -> Sample {
            self.0.recv().unwrap_or_else(|_| {
                std::thread::sleep(Duration::from_secs(3600));
                Sample::default()
            })
        }
    }

    #[test]
    fn wayland_and_headless_sessions_never_guess_a_focus() {
        for session in [SessionKind::Wayland, SessionKind::Headless] {
            let observer = LinuxForeground::for_session(session);
            assert_eq!(observer.foreground(), Foreground::Unknown, "{session:?}");
            assert!(observer.watch.is_none(), "no X worker is started");
        }
    }

    #[test]
    fn the_cached_focus_is_served_and_a_stale_cache_is_unknown() {
        let (tx, rx) = mpsc::channel();
        let watch = Arc::new(X11Watch::start(
            Scripted(rx),
            Duration::from_millis(10),
            Duration::from_millis(200),
        ));
        let observer = LinuxForeground::with_watch(watch);
        assert_eq!(
            observer.foreground(),
            Foreground::Unknown,
            "nothing read yet"
        );

        tx.send(Sample {
            exe: Some("Firefox".to_string()),
            xtest: true,
        })
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while observer.foreground() == Foreground::Unknown {
            assert!(Instant::now() < deadline, "the reading never arrived");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            observer.foreground(),
            Foreground::App {
                id: "firefox".to_string(),
                name: "Firefox".to_string()
            }
        );

        // The worker is now stuck in a call (the script is spent): the answer must go stale.
        std::thread::sleep(Duration::from_millis(350));
        let t = Instant::now();
        assert_eq!(observer.foreground(), Foreground::Unknown);
        assert!(t.elapsed() < Duration::from_millis(50), "never blocks");
    }

    /// Real X server: `cargo test -p kivori-desktop -- --ignored --nocapture focused_app_now`.
    #[test]
    #[ignore = "reads the live focused app (needs an X11 session)"]
    fn focused_app_now() {
        let seen = LinuxForeground::new().foreground();
        println!("session: {:?}, foreground: {seen:?}", session_kind());
        if let Foreground::App { id, .. } = &seen {
            assert_eq!(id, &id.to_lowercase());
        }
    }
}
