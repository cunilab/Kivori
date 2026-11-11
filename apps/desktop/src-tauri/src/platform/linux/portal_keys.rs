//! Keyboard-only RemoteDesktop portal session. Starting it is exclusively a user action.
use crate::platform::{ActionError, InputGuard, Shortcut, ShortcutKey};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

const DEST: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE: &str = "org.freedesktop.portal.RemoteDesktop";

#[derive(Default)]
struct State {
    status: &'static str,
    session: Option<Session>,
    generation: u64,
}
struct Session {
    conn: Connection,
    path: OwnedObjectPath,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static SEND: Mutex<()> = Mutex::new(());
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        Mutex::new(State {
            status: "disabled",
            ..State::default()
        })
    })
}

pub fn status() -> &'static str {
    state().lock().unwrap().status
}
pub fn enable() {
    let generation = {
        let mut s = state().lock().unwrap();
        if s.status == "starting" || s.status == "enabled" {
            return;
        }
        s.generation += 1;
        s.status = "starting";
        s.generation
    };
    std::thread::spawn(move || {
        if let Err(error) = start(generation) {
            tracing::warn!(%error, "keyboard permission setup failed");
            let mut s = state().lock().unwrap();
            if s.generation == generation {
                s.session = None;
                s.status = if error == "keyboard access was declined or cancelled"
                    || error == "keyboard access was not granted"
                {
                    "denied"
                } else {
                    "disconnected"
                };
            }
        }
    });
}
fn token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(format!(
        "kivori{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
}
fn request<F>(conn: &Connection, call: F) -> Result<HashMap<String, OwnedValue>, String>
where
    F: FnOnce(&str) -> zbus::Result<OwnedObjectPath>,
{
    let tok = token()?;
    let sender = conn
        .unique_name()
        .ok_or("missing bus name")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{tok}");
    let proxy = Proxy::new(conn, DEST, path.as_str(), "org.freedesktop.portal.Request")
        .map_err(|e| e.to_string())?;
    let mut signals = proxy
        .receive_signal("Response")
        .map_err(|e| e.to_string())?;
    let returned = call(&tok).map_err(|e| e.to_string())?;
    if returned.as_str() != path {
        return Err("unexpected portal request path".into());
    }
    let message = signals.next().ok_or("portal disconnected")?;
    let (code, values): (u32, HashMap<String, OwnedValue>) =
        message.body().deserialize().map_err(|e| e.to_string())?;
    response(code, values)
}
fn response(
    code: u32,
    values: HashMap<String, OwnedValue>,
) -> Result<HashMap<String, OwnedValue>, String> {
    match code {
        0 => Ok(values),
        1 => Err("keyboard access was declined or cancelled".into()),
        _ => Err("keyboard permission request ended".into()),
    }
}
fn start(generation: u64) -> Result<(), String> {
    let conn = zbus::blocking::connection::Builder::session()
        .map_err(|e| e.to_string())?
        .method_timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    let bus = Proxy::new(
        &conn,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .map_err(|e| e.to_string())?;
    let portal = Proxy::new(&conn, DEST, PATH, IFACE).map_err(|e| e.to_string())?;
    let available: u32 = portal
        .get_property("AvailableDeviceTypes")
        .map_err(|e| e.to_string())?;
    if available & 1 == 0 {
        return Err("keyboard access is unavailable".into());
    }
    let owner: String = bus
        .call("GetNameOwner", &(DEST,))
        .map_err(|e| e.to_string())?;
    let session_token = token()?;
    let created = request(&conn, |tok| {
        let options = HashMap::from([
            ("handle_token", Value::from(tok)),
            ("session_handle_token", Value::from(session_token.as_str())),
        ]);
        portal.call("CreateSession", &(options,))
    })?;
    let handle = created
        .get("session_handle")
        .ok_or("missing session handle")?;
    let path: OwnedObjectPath = if let Ok(value) = <&str>::try_from(handle) {
        OwnedObjectPath::try_from(value).map_err(|e| e.to_string())?
    } else {
        handle
            .try_clone()
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|e: zbus::zvariant::Error| e.to_string())?
    };
    let session_proxy = Proxy::new(&conn, DEST, path.as_str(), "org.freedesktop.portal.Session")
        .map_err(|e| e.to_string())?;
    let mut closed = session_proxy
        .receive_signal("Closed")
        .map_err(|e| e.to_string())?;
    let setup = (|| {
        request(&conn, |tok| {
            let opts = HashMap::from([
                ("handle_token", Value::from(tok)),
                ("types", Value::from(1u32)),
            ]);
            portal.call("SelectDevices", &(&path, opts))
        })?;
        let result = request(&conn, |tok| {
            let opts = HashMap::from([("handle_token", Value::from(tok))]);
            portal.call("Start", &(&path, "", opts))
        })?;
        let devices: u32 = result
            .get("devices")
            .ok_or("missing granted devices")?
            .try_clone()
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|e: zbus::zvariant::Error| e.to_string())?;
        if devices & 1 == 0 {
            return Err("keyboard access was not granted".into());
        }
        Ok(())
    })();
    if let Err(e) = setup {
        let _ = session_proxy.call::<_, _, ()>("Close", &());
        return Err(e);
    }
    {
        let mut s = state().lock().unwrap();
        s.status = "enabled";
        s.session = Some(Session {
            conn: conn.clone(),
            path: path.clone(),
        });
    }
    // A portal restart may drop the session without a Session.Closed signal.
    let watch_conn = conn.clone();
    let owner_before = owner;
    std::thread::spawn(move || {
        let Ok(bus) = Proxy::new(
            &watch_conn,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        ) else {
            invalidate(generation);
            return;
        };
        let Ok(mut changes) = bus.receive_signal("NameOwnerChanged") else {
            invalidate(generation);
            return;
        };
        let owner: zbus::Result<String> = bus.call("GetNameOwner", &(DEST,));
        if owner.as_ref() != Ok(&owner_before) {
            invalidate(generation);
            let _ = watch_conn.close();
            return;
        }
        let owner = owner_before;
        for message in &mut changes {
            if let Ok((name, old, _new)) = message.body().deserialize::<(String, String, String)>()
            {
                if name == DEST && old == owner {
                    break;
                }
            }
        }
        invalidate(generation);
        let _ = watch_conn.close();
    });
    // A closed session or broken bus invalidates availability; never reconnect automatically.
    let _ = closed.next();
    invalidate(generation);
    let _ = conn.clone().close();
    Ok(())
}
fn invalidate(generation: u64) {
    invalidate_state(&mut state().lock().unwrap(), generation);
}
fn invalidate_state(s: &mut State, generation: u64) {
    if s.generation == generation {
        s.session = None;
        s.status = "disconnected";
    }
}
fn keysym(key: ShortcutKey) -> Result<i32, ActionError> {
    Ok(match key {
        ShortcutKey::Char(c) if c.is_ascii_alphanumeric() || "`[]".contains(c) => c as i32,
        ShortcutKey::Function(n @ 1..=20) => 0xffbe + i32::from(n) - 1,
        ShortcutKey::Enter => 0xff0d,
        ShortcutKey::Tab => 0xff09,
        ShortcutKey::Space => 0x20,
        ShortcutKey::Escape => 0xff1b,
        ShortcutKey::Backspace => 0xff08,
        ShortcutKey::Delete => 0xffff,
        ShortcutKey::Home => 0xff50,
        ShortcutKey::End => 0xff57,
        ShortcutKey::PageUp => 0xff55,
        ShortcutKey::PageDown => 0xff56,
        ShortcutKey::Up => 0xff52,
        ShortcutKey::Down => 0xff54,
        ShortcutKey::Left => 0xff51,
        ShortcutKey::Right => 0xff53,
        _ => return Err(ActionError::Failed("unsupported key".into())),
    })
}
fn chord<F>(shortcut: &Shortcut, guard: &InputGuard, mut send: F) -> Result<(), ActionError>
where
    F: FnMut(i32, bool) -> Result<(), ActionError>,
{
    let key = keysym(shortcut.key)?;
    let mut pressed = Vec::new();
    let mut result = Ok(());
    for key in [
        (shortcut.ctrl, 0xffe3),
        (shortcut.alt, 0xffe9),
        (shortcut.shift, 0xffe1),
        (shortcut.meta, 0xffeb),
    ]
    .into_iter()
    .filter_map(|(on, key)| on.then_some(key))
    .chain(std::iter::once(key))
    {
        if let Err(e) = guard.check() {
            result = Err(e);
            break;
        }
        // A failed call may have reached the compositor; attempt its release too.
        pressed.push(key);
        if let Err(e) = send(key, true) {
            result = Err(e);
            break;
        }
    }
    for key in pressed.into_iter().rev() {
        if let Err(e) = send(key, false) {
            if result.is_ok() {
                result = Err(e);
            }
        }
    }
    result
}
pub fn send(shortcut: &Shortcut, guard: InputGuard) -> Result<(), ActionError> {
    guard.check()?;
    let _serial = SEND.lock().unwrap();
    guard.check()?;
    let (conn, path, generation) = {
        let s = state().lock().unwrap();
        let session = s.session.as_ref().ok_or(ActionError::PermissionRequired)?;
        (session.conn.clone(), session.path.clone(), s.generation)
    };
    let proxy = Proxy::new(&conn, DEST, PATH, IFACE).map_err(|_| ActionError::Uncertain)?;
    let mut failed = false;
    let result = chord(shortcut, &guard, |key, down| {
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        proxy
            .call::<_, _, ()>(
                "NotifyKeyboardKeysym",
                &(&path, options, key, u32::from(down)),
            )
            .map_err(|_| {
                failed = true;
                ActionError::Uncertain
            })
    });
    if failed {
        let mut s = state().lock().unwrap();
        if s.generation == generation {
            s.session = None;
            s.status = "disconnected";
        }
        drop(s);
        let _ = conn.close();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    fn guard() -> InputGuard {
        InputGuard {
            deadline: None,
            current: Arc::new(|| true),
        }
    }
    #[test]
    fn disconnection_invalidates_only_the_matching_session() {
        let mut state = State {
            status: "enabled",
            generation: 4,
            session: None,
        };
        invalidate_state(&mut state, 3);
        assert_eq!(state.status, "enabled");
        invalidate_state(&mut state, 4);
        assert_eq!(state.status, "disconnected");
    }
    #[test]
    fn denied_or_cancelled_portal_response_cannot_grant_a_session() {
        assert!(response(1, HashMap::new()).is_err());
        assert!(response(2, HashMap::new()).is_err());
        assert!(response(0, HashMap::new()).is_ok());
    }
    #[test]
    fn maps_shortcuts_to_x_keysyms() {
        assert_eq!(keysym(ShortcutKey::Tab), Ok(0xff09));
        assert_eq!(keysym(ShortcutKey::Char('a')), Ok(97));
        assert_eq!(keysym(ShortcutKey::Function(20)), Ok(0xffd1));
        assert!(keysym(ShortcutKey::Function(21)).is_err());
    }
    #[test]
    fn chord_presses_and_releases_in_reverse_order() {
        let mut events = Vec::new();
        chord(&"Ctrl+Shift+Tab".parse().unwrap(), &guard(), |key, down| {
            events.push((key, down));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            events,
            [
                (0xffe3, true),
                (0xffe1, true),
                (0xff09, true),
                (0xff09, false),
                (0xffe1, false),
                (0xffe3, false)
            ]
        );
    }
    #[test]
    fn cancellation_between_phases_releases_pressed_keys() {
        let count = Arc::new(AtomicUsize::new(0));
        let seen = count.clone();
        let guard = InputGuard {
            deadline: None,
            current: Arc::new(move || seen.fetch_add(1, Ordering::SeqCst) == 0),
        };
        let mut events = Vec::new();
        assert!(chord(&"Ctrl+Tab".parse().unwrap(), &guard, |key, down| {
            events.push((key, down));
            Ok(())
        })
        .is_err());
        assert_eq!(events, [(0xffe3, true), (0xffe3, false)]);
    }
    #[test]
    fn failed_press_is_also_released_and_other_modifiers_are_cleaned_up() {
        let mut events = Vec::new();
        let result = chord(&"Ctrl+Tab".parse().unwrap(), &guard(), |key, down| {
            events.push((key, down));
            if key == 0xff09 && down {
                Err(ActionError::Uncertain)
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(ActionError::Uncertain));
        assert_eq!(
            events,
            [
                (0xffe3, true),
                (0xff09, true),
                (0xff09, false),
                (0xffe3, false)
            ]
        );
    }
    #[test]
    fn expired_guard_sends_nothing() {
        let guard = InputGuard {
            deadline: Some(std::time::Instant::now()),
            current: Arc::new(|| true),
        };
        let mut events = Vec::new();
        assert!(chord(&"Ctrl+Tab".parse().unwrap(), &guard, |key, down| {
            events.push((key, down));
            Ok(())
        })
        .is_err());
        assert!(events.is_empty());
    }
}
