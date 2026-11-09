//! GNOME Shell's opt-in, title-free session D-Bus bridge. All I/O stays off the device thread.
//!
//! One shared session-bus connection serves everything here. A single worker waits (without
//! polling) for the extension's bus name to gain an owner, checks that the owner is GNOME Shell
//! running a compatible extension version, Subscribes, and then re-reads once a second so a hung
//! Shell expires the cache. A second thread receives the unicast `FocusChanged` signals and
//! accepts only those sent by that verified owner. Failures back off exponentially up to a minute.
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use zbus::blocking::fdo::{DBusProxy, PropertiesProxy};
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::message::Type;
use zbus::names::{BusName, InterfaceName};
use zbus::proxy::CacheProperties;
use zbus::MatchRule;

use super::apps::DesktopEntry;
use crate::platform::foreground::classify_linux;
use crate::platform::Foreground;

pub(super) const BUS: &str = "id.cunilab.KivoriFocus";
const PATH: &str = "/id/cunilab/KivoriFocus";
/// The extension version this build embeds (`metadata.json` "version", `bridge.js` VERSION).
/// A running copy at least this new is trusted; an older one is never talked to.
pub(super) const VERSION: u32 = 2;
const TTL: Duration = Duration::from_secs(3);
const HEARTBEAT: Duration = Duration::from_secs(1);
const BACKOFF_FIRST: Duration = Duration::from_secs(2);
const BACKOFF_MAX: Duration = Duration::from_secs(60);
pub(super) type Reply = (String, String, u32, bool);

/// Use the executable first, exactly as X11 and the running-app picker do. Desktop ids and
/// WM_CLASS cover sandboxed or already-exited processes without ever reading a window title.
/// A Chromium app window (a PWA such as Teams) is its own app, never the browser behind it. The
/// extension reports `protected` for a lock screen and for any Shell-modal prompt (polkit,
/// keyring, network secrets), which is Protected whatever app sits behind it.
fn map_reply(reply: Reply) -> Foreground {
    map_reply_in(&super::apps::entries(), reply)
}

fn map_reply_in(entries: &[DesktopEntry], (app_id, wm_class, pid, protected): Reply) -> Foreground {
    if protected {
        return Foreground::Protected;
    }
    if let Some(app) = super::apps::pwa_foreground(entries, &app_id, &wm_class) {
        return app;
    }
    let exe = super::proc::exe_name(pid).or_else(|| {
        super::apps::focus_program_in(entries, &app_id)
            .or_else(|| super::apps::focus_program_in(entries, &wm_class))
    });
    classify_linux(exe.as_deref(), false)
}

static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

/// The one session-bus connection of this module, created on first use.
pub(super) fn connection() -> zbus::Result<Connection> {
    let mut slot = CONNECTION.lock().expect("session bus");
    if let Some(conn) = slot.as_ref() {
        return Ok(conn.clone());
    }
    let conn = zbus::blocking::connection::Builder::session()?
        .method_timeout(Duration::from_secs(2))
        .build()?;
    *slot = Some(conn.clone());
    Ok(conn)
}

fn reset_connection() {
    *CONNECTION.lock().expect("session bus") = None;
}

/// Who owns the extension's bus name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Peer {
    Absent,
    /// Some program other than GNOME Shell owns the name.
    Foreign,
    /// GNOME Shell owns it; `version` is `None` for a copy without a `Version` property.
    Shell {
        owner: String,
        version: Option<u32>,
    },
}

/// Whether `version` is new enough to be trusted by this build.
pub(super) fn compatible(version: Option<u32>) -> bool {
    version.is_some_and(|v| v >= VERSION)
}

fn is_shell_exe(exe: Option<&str>) -> bool {
    exe.is_some_and(|name| name.contains("gnome-shell"))
}

pub(super) fn peer(conn: &Connection) -> zbus::Result<Peer> {
    let dbus = DBusProxy::new(conn)?;
    let name = BusName::try_from(BUS)?;
    if !dbus.name_has_owner(name.clone())? {
        return Ok(Peer::Absent);
    }
    let Ok(owner) = dbus.get_name_owner(name) else {
        return Ok(Peer::Absent);
    };
    let pid = dbus.get_connection_unix_process_id(BusName::Unique(owner.inner().clone()))?;
    if !is_shell_exe(super::proc::exe_name(pid).as_deref()) {
        return Ok(Peer::Foreign);
    }
    let version = PropertiesProxy::builder(conn)
        .destination(BUS)?
        .path(PATH)?
        .cache_properties(CacheProperties::No)
        .build()
        .ok()
        .and_then(|props| {
            props
                .get(InterfaceName::try_from(BUS).ok()?, "Version")
                .ok()
        })
        .and_then(|value| u32::try_from(value).ok());
    Ok(Peer::Shell {
        owner: owner.to_string(),
        version,
    })
}

/// The pid of GNOME Shell itself, to tell whether it restarted after the extension was written.
pub(super) fn shell_pid(conn: &Connection) -> Option<u32> {
    DBusProxy::new(conn)
        .ok()?
        .get_connection_unix_process_id(BusName::try_from("org.gnome.Shell").ok()?)
        .ok()
}

fn subscribe(conn: &Connection) -> zbus::Result<Reply> {
    Proxy::new(conn, BUS, PATH, BUS)?.call("Subscribe", &())
}

/// Doubles up to a minute; a success starts it over.
struct Backoff(Duration);

impl Backoff {
    fn new() -> Self {
        Self(BACKOFF_FIRST)
    }

    fn reset(&mut self) {
        self.0 = BACKOFF_FIRST;
    }

    fn wait(&mut self) {
        std::thread::sleep(self.0);
        self.0 = (self.0 * 2).min(BACKOFF_MAX);
    }
}

#[derive(Default)]
pub(super) struct GnomeFocus {
    sample: Mutex<Option<(Instant, Foreground)>>,
    /// The unique name of the verified GNOME Shell; only its signals are believed.
    owner: Mutex<Option<String>>,
}

impl GnomeFocus {
    /// Only ever called for a GNOME Wayland session (see `LinuxForeground`).
    pub(super) fn shared() -> Arc<Self> {
        static SHARED: OnceLock<Arc<GnomeFocus>> = OnceLock::new();
        Arc::clone(SHARED.get_or_init(|| {
            let watch = Arc::new(Self::default());
            let worker = Arc::clone(&watch);
            std::thread::spawn(move || worker.watch());
            let listener = Arc::clone(&watch);
            std::thread::spawn(move || listener.listen());
            watch
        }))
    }

    fn clear(&self) {
        *self.sample.lock().expect("GNOME focus") = None;
        *self.owner.lock().expect("GNOME focus") = None;
    }

    fn store(&self, reply: Reply) {
        let foreground = map_reply(reply);
        *self.sample.lock().expect("GNOME focus") = Some((Instant::now(), foreground));
    }

    fn watch(&self) {
        let mut backoff = Backoff::new();
        loop {
            if let Err(error) = self.attend(&mut backoff) {
                tracing::warn!(%error, "GNOME focus observer disconnected");
                reset_connection();
            }
            self.clear();
            backoff.wait();
        }
    }

    /// Follow the extension's name on one connection until the bus itself fails.
    fn attend(&self, backoff: &mut Backoff) -> zbus::Result<()> {
        let conn = connection()?;
        let dbus = DBusProxy::new(&conn)?;
        // Created before the first look, so no owner change falls between the two.
        let mut changes = dbus.receive_name_owner_changed_with_args(&[(0, BUS)])?;
        loop {
            match peer(&conn)? {
                Peer::Shell { owner, version } if compatible(version) => {
                    *self.owner.lock().expect("GNOME focus") = Some(owner);
                    if let Err(error) = self.beat(&conn, backoff) {
                        tracing::warn!(%error, "GNOME focus subscription failed");
                        self.clear();
                        backoff.wait();
                    }
                }
                _ => {
                    // No extension (or a stale one): wait for the name to change hands.
                    self.clear();
                    if changes.next().is_none() {
                        return Err(zbus::Error::Failure("bus name watch ended".into()));
                    }
                }
            }
        }
    }

    /// Subscribe (which returns the current state) every second until a call fails. The repeat
    /// re-subscribes after an extension reload and keeps the cache fresh while Shell answers.
    fn beat(&self, conn: &Connection, backoff: &mut Backoff) -> zbus::Result<()> {
        let mut subscribed = false;
        loop {
            let reply = subscribe(conn)?;
            if !subscribed {
                tracing::info!("GNOME focus observer subscribed");
                subscribed = true;
            }
            self.store(reply);
            backoff.reset();
            std::thread::sleep(HEARTBEAT);
        }
    }

    fn listen(&self) {
        let mut backoff = Backoff::new();
        loop {
            let _ = self.receive(&mut backoff);
            backoff.wait();
        }
    }

    fn receive(&self, backoff: &mut Backoff) -> zbus::Result<()> {
        let conn = connection()?;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(BUS)?
            .member("FocusChanged")?
            .path(PATH)?
            .build();
        let signals = MessageIterator::for_match_rule(rule, &conn, None)?;
        backoff.reset();
        for message in signals {
            let message = message?;
            let from = message.header().sender().map(ToString::to_string);
            // The bus fills in `sender`, so a same-user impostor cannot pose as the Shell.
            if from.is_none() || from != *self.owner.lock().expect("GNOME focus") {
                continue;
            }
            if let Ok(reply) = message.body().deserialize::<Reply>() {
                tracing::info!(app_id = %reply.0, "GNOME focus change received");
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
    fn brave_desktop_and_snap_focus_commit_the_browser_profile() {
        use crate::config::{ProfileId, ProfileRef};
        use crate::desk::profile::{builtins_for, Context, STABLE_FOR};

        let entry = super::super::apps::parse_desktop_entry(
            std::path::Path::new("/var/lib/snapd/desktop/applications/brave_brave.desktop"),
            "[Desktop Entry]\nType=Application\nName=Brave Web Browser\nExec=/snap/bin/brave %U\nStartupWMClass=brave\n",
        )
        .unwrap();
        for (desktop, class) in [
            ("brave-browser.desktop", "brave"),
            ("brave_brave.desktop", "brave"),
            ("", "brave"),
        ] {
            // No readable pid: desktop identity must also work for sandboxed/exited processes.
            let foreground = map_reply_in(
                std::slice::from_ref(&entry),
                (desktop.into(), class.into(), 0, false),
            );
            let watch = GnomeFocus {
                sample: Mutex::new(Some((Instant::now(), foreground))),
                ..GnomeFocus::default()
            };
            let mut context = Context::new(builtins_for(false));
            context.observe(watch.foreground(), Duration::ZERO);
            context.observe(watch.foreground(), STABLE_FOR);
            assert_eq!(
                context.profile().id,
                ProfileRef::Builtin(ProfileId::Browser),
                "{desktop}/{class}"
            );
            assert!(!context.pinned());
        }
    }

    #[test]
    fn a_chromium_web_app_reaches_the_profile_of_its_own_app() {
        let entry = crate::platform::linux::apps::parse_desktop_entry(
            std::path::Path::new("/x/brave-cifhbcnohmdccbgoicgdjpfamggdegmo-Default.desktop"),
            "[Desktop Entry]\nType=Application\nName=Microsoft Teams\nExec=brave-browser --app-id=x\n",
        )
        .unwrap();
        // The browser's own pid is deliberately ignored: this process is whatever runs the tests.
        let Foreground::App { id, .. } = map_reply_in(
            &[entry],
            (
                "brave-cifhbcnohmdccbgoicgdjpfamggdegmo-Default.desktop".into(),
                "crx_cifhbcnohmdccbgoicgdjpfamggdegmo".into(),
                std::process::id(),
                false,
            ),
        ) else {
            panic!("no app");
        };
        let profiles = crate::desk::profile::builtins_for(false);
        assert!(profiles
            .iter()
            .any(|p| p.name == "Teams" && p.ids.contains(&id)));
    }

    #[test]
    fn a_protected_report_wins_over_whatever_app_is_behind_it() {
        // A Shell modal prompt (polkit, keyring) leaves the app behind it as the focus window.
        for app in ["code.desktop", "brave-browser.desktop", ""] {
            assert_eq!(
                map_reply_in(&[], (app.into(), "Code".into(), std::process::id(), true)),
                Foreground::Protected,
                "{app}"
            );
        }
        assert_ne!(
            map_reply_in(&[], ("code.desktop".into(), "".into(), 0, false)),
            Foreground::Protected
        );
    }

    #[test]
    fn only_gnome_shell_and_a_new_enough_version_are_trusted() {
        assert!(is_shell_exe(Some("gnome-shell")));
        assert!(is_shell_exe(Some(".gnome-shell-wrapped")));
        assert!(!is_shell_exe(Some("python3")));
        assert!(!is_shell_exe(None));
        assert!(compatible(Some(VERSION)));
        assert!(compatible(Some(VERSION + 1)));
        assert!(!compatible(Some(VERSION - 1)));
        assert!(!compatible(None), "a copy without a Version is stale");
    }

    #[test]
    fn the_embedded_extension_declares_the_version_this_build_expects() {
        let metadata: serde_json::Value = serde_json::from_str(include_str!(
            "../../../linux/gnome-extension/kivori-focus@cunilab.id/metadata.json"
        ))
        .unwrap();
        assert_eq!(metadata["version"], VERSION);
        let bridge =
            include_str!("../../../linux/gnome-extension/kivori-focus@cunilab.id/bridge.js");
        assert!(bridge.contains(&format!("export const VERSION = {VERSION};")));
        assert!(bridge.contains(&format!("export const BUS = '{BUS}';")));
        assert!(
            !bridge.contains("GetFocusedApp"),
            "focus is never queryable"
        );
    }

    #[test]
    fn retries_back_off_to_a_minute_instead_of_hammering_the_bus() {
        let mut delays = Vec::new();
        let mut backoff = Backoff::new();
        for _ in 0..7 {
            delays.push(backoff.0.as_secs());
            backoff.0 = (backoff.0 * 2).min(BACKOFF_MAX);
        }
        assert_eq!(delays, [2, 4, 8, 16, 32, 60, 60]);
        backoff.reset();
        assert_eq!(backoff.0, BACKOFF_FIRST);
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
            ..GnomeFocus::default()
        };
        assert_eq!(watch.foreground(), Foreground::Unknown);
    }

    /// Needs a process named `gnome-shell` serving the bridge on the session bus:
    /// `cargo test -p kivori-desktop -- --ignored --nocapture live_observer_now`.
    #[test]
    #[ignore = "needs the GNOME extension (or a stand-in bridge) on the session bus"]
    fn live_observer_now() {
        let watch = GnomeFocus::shared();
        let mut seen = Vec::new();
        for _ in 0..80 {
            let now = watch.foreground();
            if seen.last() != Some(&now) {
                println!("{now:?}");
                seen.push(now);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(seen.len() > 1, "never saw the bridge: {seen:?}");
    }
}
