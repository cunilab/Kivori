//! Keyboard shortcuts as data: parsed from text like `Ctrl+Shift+M`, validated once at the trust
//! boundary, and sent by an [`super::InputSynth`].

use std::fmt;

/// A non-modifier key a shortcut may end in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutKey {
    /// An ASCII letter (stored lowercase) or digit.
    Char(char),
    /// F1..=F24.
    Function(u8),
    Enter,
    Tab,
    Space,
    Escape,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
}

/// Modifiers plus exactly one key. `meta` is Command on macOS and the Windows key on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: ShortcutKey,
}

/// Why a shortcut string was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutError {
    /// Nothing but modifiers (or nothing at all).
    MissingKey,
    /// More than one non-modifier key.
    TooManyKeys,
    /// The same modifier twice.
    DuplicateModifier(String),
    /// A token that is neither a known modifier nor a supported key.
    UnknownToken(String),
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShortcutError::MissingKey => write!(f, "a shortcut needs one key besides modifiers"),
            ShortcutError::TooManyKeys => write!(f, "a shortcut takes only one non-modifier key"),
            ShortcutError::DuplicateModifier(m) => write!(f, "modifier {m} appears twice"),
            ShortcutError::UnknownToken(t) => write!(f, "unknown key {t:?}"),
        }
    }
}

const NAMED: [(&str, ShortcutKey); 17] = [
    ("enter", ShortcutKey::Enter),
    ("return", ShortcutKey::Enter),
    ("tab", ShortcutKey::Tab),
    ("space", ShortcutKey::Space),
    ("escape", ShortcutKey::Escape),
    ("esc", ShortcutKey::Escape),
    ("backspace", ShortcutKey::Backspace),
    ("delete", ShortcutKey::Delete),
    ("del", ShortcutKey::Delete),
    ("home", ShortcutKey::Home),
    ("end", ShortcutKey::End),
    ("pageup", ShortcutKey::PageUp),
    ("pagedown", ShortcutKey::PageDown),
    ("up", ShortcutKey::Up),
    ("down", ShortcutKey::Down),
    ("left", ShortcutKey::Left),
    ("right", ShortcutKey::Right),
];

fn parse_key(token: &str) -> Option<ShortcutKey> {
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c
            .is_ascii_alphanumeric()
            .then(|| ShortcutKey::Char(c.to_ascii_lowercase()));
    }
    if let Some(n) = token.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
        return (1..=24).contains(&n).then_some(ShortcutKey::Function(n));
    }
    NAMED
        .iter()
        .find(|(name, _)| *name == token)
        .map(|(_, key)| *key)
}

impl std::str::FromStr for Shortcut {
    type Err = ShortcutError;

    /// Parses `+`-separated tokens, case-insensitively, ignoring surrounding spaces:
    /// `ctrl`/`control`, `alt`/`option`/`opt`, `shift`, `meta`/`cmd`/`command`/`win`/`super`,
    /// then one key: a letter, a digit, `F1`..`F24`, or a named key (`Enter`, `Space`, `Up`...).
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (mut ctrl, mut alt, mut shift, mut meta) = (false, false, false, false);
        let mut key = None;
        for raw in text.split('+') {
            let token = raw.trim().to_ascii_lowercase();
            let modifier = match token.as_str() {
                "ctrl" | "control" => Some(&mut ctrl),
                "alt" | "option" | "opt" => Some(&mut alt),
                "shift" => Some(&mut shift),
                "meta" | "cmd" | "command" | "win" | "super" => Some(&mut meta),
                _ => None,
            };
            if let Some(flag) = modifier {
                if *flag {
                    return Err(ShortcutError::DuplicateModifier(raw.trim().to_string()));
                }
                *flag = true;
                continue;
            }
            if token.is_empty() {
                return Err(ShortcutError::MissingKey);
            }
            let parsed = parse_key(&token)
                .ok_or_else(|| ShortcutError::UnknownToken(raw.trim().to_string()))?;
            if key.replace(parsed).is_some() {
                return Err(ShortcutError::TooManyKeys);
            }
        }
        Ok(Shortcut {
            ctrl,
            alt,
            shift,
            meta,
            key: key.ok_or(ShortcutError::MissingKey)?,
        })
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
            (self.meta, "Meta+"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        match self.key {
            ShortcutKey::Char(c) => write!(f, "{}", c.to_ascii_uppercase()),
            ShortcutKey::Function(n) => write!(f, "F{n}"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Shortcut, ShortcutError> {
        text.parse()
    }

    #[test]
    fn modifiers_and_one_key_parse_case_insensitively() {
        let s = parse(" ctrl + Shift+m ").unwrap();
        assert!(s.ctrl && s.shift && !s.alt && !s.meta);
        assert_eq!(s.key, ShortcutKey::Char('m'));
        assert_eq!(s.to_string(), "Ctrl+Shift+M");
        assert_eq!(parse("Cmd+Option+F12").unwrap().to_string(), "Alt+Meta+F12");
        assert_eq!(parse("win+Up").unwrap().key, ShortcutKey::Up);
        assert_eq!(parse("Space").unwrap().key, ShortcutKey::Space);
    }

    #[test]
    fn the_display_form_parses_back_to_the_same_shortcut() {
        for text in ["Ctrl+Alt+Shift+Meta+Z", "F1", "Shift+PageDown", "Alt+7"] {
            let shortcut = parse(text).unwrap();
            assert_eq!(parse(&shortcut.to_string()).unwrap(), shortcut);
        }
    }

    #[test]
    fn invalid_shortcuts_are_rejected_with_a_reason() {
        assert_eq!(parse(""), Err(ShortcutError::MissingKey));
        assert_eq!(parse("Ctrl+Shift"), Err(ShortcutError::MissingKey));
        assert_eq!(parse("Ctrl++"), Err(ShortcutError::MissingKey));
        assert_eq!(parse("A+B"), Err(ShortcutError::TooManyKeys));
        assert_eq!(
            parse("Ctrl+ctrl+A"),
            Err(ShortcutError::DuplicateModifier("ctrl".into()))
        );
        assert_eq!(parse("F25"), Err(ShortcutError::UnknownToken("F25".into())));
        assert_eq!(parse("F0"), Err(ShortcutError::UnknownToken("F0".into())));
        assert_eq!(
            parse("Ctrl+é"),
            Err(ShortcutError::UnknownToken("é".into()))
        );
        assert_eq!(
            parse("Ctrl+Hyper"),
            Err(ShortcutError::UnknownToken("Hyper".into()))
        );
    }
}
