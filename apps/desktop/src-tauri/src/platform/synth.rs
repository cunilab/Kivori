//! [`InputSynth`] over `enigo`, shared by Windows (`SendInput`) and macOS (Quartz events).
//!
//! macOS only delivers synthesized input from processes the user trusted under
//! Privacy & Security > Accessibility. Without it this reports
//! [`ActionError::PermissionRequired`] (and asks the OS to show its prompt), and sends nothing:
//! a missing permission is a capability state, never a reason to try another mechanism.

use enigo::{Direction, Enigo, Key, Keyboard, NewConError, Settings};

use super::{ActionError, InputSynth, Shortcut, ShortcutKey};

/// Key synthesis for the current desktop session.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnigoInputSynth;

fn connect() -> Result<Enigo, ActionError> {
    let settings = Settings {
        // Only reached from a deliberate user action, which is when the OS prompt belongs.
        open_prompt_to_get_permissions: true,
        release_keys_when_dropped: true,
        ..Settings::default()
    };
    Enigo::new(&settings).map_err(|error| match error {
        NewConError::NoPermission => ActionError::PermissionRequired,
        other => ActionError::Failed(other.to_string()),
    })
}

fn failed(error: enigo::InputError) -> ActionError {
    ActionError::Failed(error.to_string())
}

fn key_of(key: ShortcutKey) -> Result<Key, ActionError> {
    Ok(match key {
        ShortcutKey::Char(c) => Key::Unicode(c),
        ShortcutKey::Function(n) => match n {
            1 => Key::F1,
            2 => Key::F2,
            3 => Key::F3,
            4 => Key::F4,
            5 => Key::F5,
            6 => Key::F6,
            7 => Key::F7,
            8 => Key::F8,
            9 => Key::F9,
            10 => Key::F10,
            11 => Key::F11,
            12 => Key::F12,
            13 => Key::F13,
            14 => Key::F14,
            15 => Key::F15,
            16 => Key::F16,
            17 => Key::F17,
            18 => Key::F18,
            19 => Key::F19,
            20 => Key::F20,
            _ => return Err(ActionError::Failed(format!("F{n} cannot be sent here"))),
        },
        ShortcutKey::Enter => Key::Return,
        ShortcutKey::Tab => Key::Tab,
        ShortcutKey::Space => Key::Space,
        ShortcutKey::Escape => Key::Escape,
        ShortcutKey::Backspace => Key::Backspace,
        ShortcutKey::Delete => Key::Delete,
        ShortcutKey::Home => Key::Home,
        ShortcutKey::End => Key::End,
        ShortcutKey::PageUp => Key::PageUp,
        ShortcutKey::PageDown => Key::PageDown,
        ShortcutKey::Up => Key::UpArrow,
        ShortcutKey::Down => Key::DownArrow,
        ShortcutKey::Left => Key::LeftArrow,
        ShortcutKey::Right => Key::RightArrow,
    })
}

impl InputSynth for EnigoInputSynth {
    fn send_media_play_pause(&self) -> Result<(), ActionError> {
        connect()?
            .key(Key::MediaPlayPause, Direction::Click)
            .map_err(failed)
    }

    fn send_shortcut(&self, shortcut: &Shortcut) -> Result<(), ActionError> {
        let key = key_of(shortcut.key)?;
        let mut enigo = connect()?;
        let modifiers: Vec<Key> = [
            (shortcut.ctrl, Key::Control),
            (shortcut.alt, Key::Alt),
            (shortcut.shift, Key::Shift),
            (shortcut.meta, Key::Meta),
        ]
        .into_iter()
        .filter_map(|(on, key)| on.then_some(key))
        .collect();
        let mut pressed = 0;
        let mut result = Ok(());
        for modifier in &modifiers {
            result = enigo.key(*modifier, Direction::Press).map_err(failed);
            if result.is_err() {
                break;
            }
            pressed += 1;
        }
        if result.is_ok() {
            result = enigo.key(key, Direction::Click).map_err(failed);
        }
        // Release whatever went down, in reverse, even after a failure: a stuck modifier would
        // corrupt the user's next keystrokes.
        for modifier in modifiers[..pressed].iter().rev() {
            let released = enigo.key(*modifier, Direction::Release).map_err(failed);
            result = result.and(released);
        }
        result
    }
}
