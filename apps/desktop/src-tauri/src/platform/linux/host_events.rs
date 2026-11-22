//! Linux sleep/wake and lock/unlock, as [`HostEvent`]s, from systemd-logind over the system bus.
//!
//! SLEEP: `Manager.PrepareForSleep(true)` is sent before suspend and `(false)` after resume. A
//! `delay` inhibitor lock (`Manager.Inhibit`) holds the suspend until the device thread has written
//! its goodbye: on `PrepareForSleep(true)` this sends `Suspending` with an ack channel, waits at most
//! [`SUSPEND_ACK_BUDGET`], then releases the lock so the machine sleeps. The lock is taken again
//! after the resume. Without the lock (logind refused it) the events still flow, only the ack is
//! not honoured.
//!
//! LOCK: the current session's `Lock` / `Unlock` signals (`loginctl lock-session`) and its
//! `LockedHint` property (what GNOME sets when the user locks the screen, and what other desktops
//! set too). The tracker drops the duplicates the two sources produce. The session is found through
//! `XDG_SESSION_ID`, else the caller's pid, else the user's display session.
//!
//! Each blocking signal loop runs on its own thread. [`HostEventsGuard`] stops them by closing the
//! bus connection, which ends both iterators; D-Bus here is a local unix socket, never the network.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zbus::blocking::proxy::Builder as ProxyBuilder;
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue};
use zbus::MatchRule;

use crate::platform::host_events::SUSPEND_ACK_BUDGET;
use crate::platform::host_events::{HostEvent, HostEventsError, HostEventsGuard, HostSignal};

const LOGIN1: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER_IFACE: &str = "org.freedesktop.login1.Manager";
const SESSION_IFACE: &str = "org.freedesktop.login1.Session";
/// A logind call that has not answered by now is treated as failed.
const CALL_TIMEOUT: Duration = Duration::from_secs(2);

/// Whether the session is locked, as last reported by logind. The focus observer reads it: a locked
/// screen is a protected surface whatever window was focused before.
static LOCKED: AtomicBool = AtomicBool::new(false);

/// Whether the screen is locked right now, as far as logind has said (false before it has).
#[must_use]
pub fn screen_locked() -> bool {
    LOCKED.load(Ordering::SeqCst)
}

/// The host event for one logind session message. `locked_hint` is the new `LockedHint` carried by
/// a `PropertiesChanged` signal for the session interface, if it carries one.
fn session_event(interface: &str, member: &str, locked_hint: Option<bool>) -> Option<HostEvent> {
    match (interface, member) {
        (SESSION_IFACE, "Lock") => Some(HostEvent::Locked),
        (SESSION_IFACE, "Unlock") => Some(HostEvent::Unlocked),
        ("org.freedesktop.DBus.Properties", "PropertiesChanged") => locked_hint.map(|locked| {
            if locked {
                HostEvent::Locked
            } else {
                HostEvent::Unlocked
            }
        }),
        _ => None,
    }
}

/// The host event for `Manager.PrepareForSleep(going_to_sleep)`.
const fn sleep_event(going_to_sleep: bool) -> HostEvent {
    if going_to_sleep {
        HostEvent::Suspending
    } else {
        HostEvent::Resumed
    }
}

fn unavailable(error: impl std::fmt::Display) -> HostEventsError {
    HostEventsError::Unavailable(error.to_string())
}

fn proxy<'a>(conn: &Connection, path: &'a str, interface: &'a str) -> zbus::Result<Proxy<'a>> {
    ProxyBuilder::new(conn)
        .destination(LOGIN1)?
        .path(path)?
        .interface(interface)?
        .cache_properties(CacheProperties::No)
        .build()
}

/// The logind session object path of this process.
fn session_path(
    conn: &Connection,
    manager: &Proxy<'_>,
) -> Result<OwnedObjectPath, HostEventsError> {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        if let Ok(path) = manager.call::<_, _, OwnedObjectPath>("GetSession", &(id.as_str(),)) {
            return Ok(path);
        }
    }
    if let Ok(path) =
        manager.call::<_, _, OwnedObjectPath>("GetSessionByPID", &(std::process::id(),))
    {
        return Ok(path);
    }
    // A process started by the user manager (an autostart entry) is in no session scope: take the
    // user's display session instead.
    // SAFETY: `getuid` has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    let user: OwnedObjectPath = manager.call("GetUser", &(uid,)).map_err(unavailable)?;
    let user = proxy(conn, user.as_str(), "org.freedesktop.login1.User").map_err(unavailable)?;
    let (_, session): (String, OwnedObjectPath) =
        user.get_property("Display").map_err(unavailable)?;
    Ok(session)
}

/// Takes the delay inhibitor that holds a suspend until the goodbye is written.
fn inhibit(manager: &Proxy<'_>) -> Option<OwnedFd> {
    manager
        .call::<_, _, OwnedFd>(
            "Inhibit",
            &(
                "sleep",
                "Kivori",
                "Tell the device the computer is going to sleep",
                "delay",
            ),
        )
        .ok()
}

/// Starts the logind sources. Signals flow to the device thread over `tx`; dropping the guard stops
/// them.
///
/// # Errors
/// [`HostEventsError::Unavailable`] when the system bus or logind cannot be reached.
pub fn start(tx: Sender<HostSignal>) -> Result<HostEventsGuard, HostEventsError> {
    let conn = zbus::blocking::connection::Builder::system()
        .and_then(|builder| builder.method_timeout(CALL_TIMEOUT).build())
        .map_err(unavailable)?;
    let manager = proxy(&conn, MANAGER_PATH, MANAGER_IFACE).map_err(unavailable)?;

    // Subscribe before anything is read, so no signal between the read and the loop is lost.
    let sleep_signals = manager
        .receive_signal("PrepareForSleep")
        .map_err(unavailable)?;
    let session = session_path(&conn, &manager)?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(LOGIN1)
        .and_then(|rule| rule.path(session.as_str()))
        .map(|rule| rule.build())
        .map_err(unavailable)?;
    let session_signals =
        MessageIterator::for_match_rule(rule, &conn, None).map_err(unavailable)?;

    // The lock that is already on when Kivori starts.
    if let Ok(session_proxy) = proxy(&conn, session.as_str(), SESSION_IFACE) {
        if session_proxy
            .get_property::<bool>("LockedHint")
            .unwrap_or(false)
        {
            LOCKED.store(true, Ordering::SeqCst);
            let _ = tx.send(HostSignal::new(HostEvent::Locked));
        }
    }

    let inhibitor: Arc<Mutex<Option<OwnedFd>>> = Arc::new(Mutex::new(inhibit(&manager)));
    let stopped = Arc::new(AtomicBool::new(false));

    let sleep_thread = {
        let (tx, inhibitor, stopped) = (tx.clone(), Arc::clone(&inhibitor), Arc::clone(&stopped));
        std::thread::Builder::new()
            .name("kivori-power".to_string())
            .spawn(move || sleep_loop(sleep_signals, &manager, &tx, &inhibitor, &stopped))
            .map_err(unavailable)?
    };
    let lock_thread = {
        let stopped = Arc::clone(&stopped);
        std::thread::Builder::new()
            .name("kivori-lock".to_string())
            .spawn(move || lock_loop(session_signals, &tx, &stopped))
            .map_err(unavailable)?
    };

    Ok(HostEventsGuard::new(move || {
        stopped.store(true, Ordering::SeqCst);
        // Never leave a suspend delayed by a lock nobody will release.
        inhibitor.lock().expect("inhibitor mutex").take();
        // Closing the connection ends both blocking iterators.
        let _ = conn.close();
        let _ = sleep_thread.join();
        let _ = lock_thread.join();
        LOCKED.store(false, Ordering::SeqCst);
    }))
}

fn sleep_loop(
    signals: zbus::blocking::proxy::SignalIterator<'_>,
    manager: &Proxy<'_>,
    tx: &Sender<HostSignal>,
    inhibitor: &Mutex<Option<OwnedFd>>,
    stopped: &AtomicBool,
) {
    for message in signals {
        if stopped.load(Ordering::SeqCst) {
            break;
        }
        let Ok(going_to_sleep) = message.body().deserialize::<bool>() else {
            continue;
        };
        if going_to_sleep {
            let (ack_tx, ack_rx) = mpsc::sync_channel(1);
            let sent = tx
                .send(HostSignal {
                    event: sleep_event(true),
                    ack: Some(ack_tx),
                })
                .is_ok();
            if sent {
                // Timeout or a gone device thread both fall through: sleep must never be blocked.
                let _ = ack_rx.recv_timeout(SUSPEND_ACK_BUDGET);
            }
            // Let the machine sleep.
            inhibitor.lock().expect("inhibitor mutex").take();
        } else {
            let _ = tx.send(HostSignal::new(sleep_event(false)));
            if !stopped.load(Ordering::SeqCst) {
                *inhibitor.lock().expect("inhibitor mutex") = inhibit(manager);
            }
        }
    }
}

fn lock_loop(signals: MessageIterator, tx: &Sender<HostSignal>, stopped: &AtomicBool) {
    for message in signals {
        if stopped.load(Ordering::SeqCst) {
            break;
        }
        let Ok(message) = message else { continue };
        let header = message.header();
        let (Some(interface), Some(member)) = (header.interface(), header.member()) else {
            continue;
        };
        let locked_hint = if member.as_str() == "PropertiesChanged" {
            locked_hint_of(&message)
        } else {
            None
        };
        if let Some(event) = session_event(interface.as_str(), member.as_str(), locked_hint) {
            LOCKED.store(event == HostEvent::Locked, Ordering::SeqCst);
            if tx.send(HostSignal::new(event)).is_err() {
                break;
            }
        }
    }
}

/// The new `LockedHint` in a `PropertiesChanged` for the session interface, if it changed one.
fn locked_hint_of(message: &zbus::Message) -> Option<bool> {
    let (interface, changed, _): (String, HashMap<String, OwnedValue>, Vec<String>) =
        message.body().deserialize().ok()?;
    if interface != SESSION_IFACE {
        return None;
    }
    bool::try_from(changed.get("LockedHint")?.try_clone().ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lock_and_unlock_signals_map_to_host_events() {
        assert_eq!(
            session_event(SESSION_IFACE, "Lock", None),
            Some(HostEvent::Locked)
        );
        assert_eq!(
            session_event(SESSION_IFACE, "Unlock", None),
            Some(HostEvent::Unlocked)
        );
    }

    #[test]
    fn a_locked_hint_change_maps_and_other_property_changes_do_not() {
        let props = "org.freedesktop.DBus.Properties";
        assert_eq!(
            session_event(props, "PropertiesChanged", Some(true)),
            Some(HostEvent::Locked)
        );
        assert_eq!(
            session_event(props, "PropertiesChanged", Some(false)),
            Some(HostEvent::Unlocked)
        );
        // Another property (Active, IdleHint) changed: no lock edge.
        assert_eq!(session_event(props, "PropertiesChanged", None), None);
    }

    #[test]
    fn unrelated_messages_are_ignored() {
        assert_eq!(session_event(SESSION_IFACE, "Something", None), None);
        assert_eq!(session_event("org.example.Other", "Lock", None), None);
    }

    #[test]
    fn prepare_for_sleep_maps_to_suspend_and_resume() {
        assert_eq!(sleep_event(true), HostEvent::Suspending);
        assert_eq!(sleep_event(false), HostEvent::Resumed);
    }

    /// Real logind: `cargo test -p kivori-desktop -- --ignored --nocapture logind_host_events_now`.
    #[test]
    #[ignore = "talks to the live system bus"]
    fn logind_host_events_now() {
        let (tx, _rx) = mpsc::channel();
        let guard = start(tx).expect("logind reachable");
        println!("locked now: {}", screen_locked());
        drop(guard);
    }
}
