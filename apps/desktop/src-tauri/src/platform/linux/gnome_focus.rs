//! GNOME Shell's opt-in, title-free session D-Bus bridge. All I/O stays off the device thread.
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy};

use crate::platform::foreground::classify_linux;
use crate::platform::Foreground;

pub(super) const BUS: &str = "id.cunilab.KivoriFocus";
const PATH: &str = "/id/cunilab/KivoriFocus";
const TTL: Duration = Duration::from_secs(3);
pub(super) type Reply = (String, String, u32, bool);

/// Use the executable first, exactly as X11 and the running-app picker do. Desktop ids and
/// WM_CLASS cover sandboxed or already-exited processes without ever reading a window title.
fn map_reply((app_id, wm_class, pid, locked): Reply) -> Foreground {
    if locked {
        return Foreground::Protected;
    }
    let exe = super::proc::exe_name(pid).or_else(|| {
        super::apps::focus_program(&app_id).or_else(|| super::apps::focus_program(&wm_class))
    });
    classify_linux(exe.as_deref(), false)
}

pub(super) fn connection() -> zbus::Result<Connection> {
    zbus::blocking::connection::Builder::session()?
        .method_timeout(Duration::from_secs(2))
        .build()
}

fn proxy(conn: &Connection) -> zbus::Result<Proxy<'_>> {
    Proxy::new(conn, BUS, PATH, BUS)
}

pub(super) fn read(conn: &Connection) -> zbus::Result<Reply> {
    proxy(conn)?.call("GetFocusedApp", &())
}

#[derive(Default)]
pub(super) struct GnomeFocus {
    sample: Mutex<Option<(Instant, Foreground)>>,
}

impl GnomeFocus {
    pub(super) fn shared() -> Arc<Self> {
        static SHARED: OnceLock<Arc<GnomeFocus>> = OnceLock::new();
        Arc::clone(SHARED.get_or_init(|| {
            let watch = Arc::new(Self::default());
            let poll = Arc::clone(&watch);
            // Heartbeat invalidates focus on disable, Shell restart or an unresponsive bus and
            // reconnects automatically after login. A signal alone cannot prove cache freshness.
            std::thread::spawn(move || loop {
                if let Ok(conn) = connection() {
                    loop {
                        match read(&conn) {
                            Ok(reply) => poll.store(reply),
                            Err(_) => {
                                *poll.sample.lock().expect("GNOME focus") = None;
                                break;
                            }
                        }
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
                std::thread::sleep(Duration::from_secs(2));
            });
            let signals = Arc::clone(&watch);
            std::thread::spawn(move || loop {
                let _ = signals.subscribe();
                std::thread::sleep(Duration::from_secs(2));
            });
            watch
        }))
    }

    fn store(&self, reply: Reply) {
        let foreground = map_reply(reply);
        *self.sample.lock().expect("GNOME focus") = Some((Instant::now(), foreground));
    }

    fn subscribe(&self) -> zbus::Result<()> {
        let conn = connection()?;
        let proxy = proxy(&conn)?;
        // Subscribe before the initial read: no missed change between the call and subscription.
        let signals = proxy.receive_signal("FocusChanged")?;
        self.store(proxy.call("GetFocusedApp", &())?);
        for message in signals {
            if let Ok(reply) = message.body().deserialize::<Reply>() {
                self.store(reply);
            }
        }
        Ok(())
    }

    pub(super) fn foreground(&self) -> Foreground {
        self.sample
            .lock()
            .expect("GNOME focus")
            .as_ref()
            .filter(|(at, _)| at.elapsed() < TTL)
            .map_or(Foreground::Unknown, |(_, value)| value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbus_reply_maps_to_existing_builtin_profiles() {
        for (desktop, expected) in [
            ("brave-browser.desktop", "Browser"),
            ("google-chrome.desktop", "Browser"),
            ("chromium.desktop", "Browser"),
            ("firefox.desktop", "Browser"),
            ("code.desktop", "Code"),
            ("Zoom.desktop", "Zoom"),
            ("teams.desktop", "Teams"),
        ] {
            let Foreground::App { id, .. } = map_reply((desktop.into(), "".into(), 0, false))
            else {
                panic!("no app for {desktop}");
            };
            assert!(
                crate::desk::profile::builtins_for(false)
                    .iter()
                    .any(|profile| profile.name == expected && profile.ids.contains(&id)),
                "{desktop}: {id}"
            );
        }
    }

    #[test]
    fn no_window_unknown_lock_protected_and_stale_cache_expires() {
        assert_eq!(
            map_reply(("".into(), "".into(), 0, false)),
            Foreground::Unknown
        );
        assert_eq!(
            map_reply(("brave.desktop".into(), "".into(), 0, true)),
            Foreground::Protected
        );
        assert_eq!(
            map_reply(("".into(), "pinentry-gnome3".into(), 0, false)),
            Foreground::Protected
        );
        let watch = GnomeFocus {
            sample: Mutex::new(Some((Instant::now() - TTL, Foreground::Protected))),
        };
        assert_eq!(watch.foreground(), Foreground::Unknown);
    }
}
