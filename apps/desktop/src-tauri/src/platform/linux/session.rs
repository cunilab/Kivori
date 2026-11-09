//! Which kind of desktop session Kivori runs in. Key synthesis and focus reading depend on it:
//! X11 allows both, Wayland allows neither without a portal Kivori does not use.

/// The graphical session this process belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    X11,
    Wayland,
    /// No display at all (a console or SSH session).
    Headless,
}

/// Classifies a session from the environment, read through `var` so tests need no real one.
///
/// `XDG_SESSION_TYPE` (set by the display manager) wins when it says `x11` or `wayland`; otherwise
/// `WAYLAND_DISPLAY` and then `DISPLAY` decide. An X11 session with no `DISPLAY` is headless: there
/// is no server to connect to.
pub(super) fn classify(var: impl Fn(&str) -> Option<String>) -> SessionKind {
    let set = |name: &str| var(name).is_some_and(|v| !v.is_empty());
    let x11_or_headless = || {
        if set("DISPLAY") {
            SessionKind::X11
        } else {
            SessionKind::Headless
        }
    };
    match var("XDG_SESSION_TYPE").as_deref() {
        Some("wayland") => SessionKind::Wayland,
        Some("x11") => x11_or_headless(),
        _ if set("WAYLAND_DISPLAY") => SessionKind::Wayland,
        _ => x11_or_headless(),
    }
}

/// The session type of this process, from its environment.
#[must_use]
pub fn session_kind() -> SessionKind {
    classify(|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(pairs: &[(&str, &str)]) -> SessionKind {
        classify(|name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_string())
        })
    }

    #[test]
    fn the_session_type_wins_over_the_display_variables() {
        // GNOME on Wayland sets DISPLAY for XWayland too.
        let both = [("DISPLAY", ":0"), ("WAYLAND_DISPLAY", "wayland-0")];
        assert_eq!(
            kind(&[("XDG_SESSION_TYPE", "wayland"), both[0], both[1]]),
            SessionKind::Wayland
        );
        assert_eq!(
            kind(&[("XDG_SESSION_TYPE", "x11"), both[0], both[1]]),
            SessionKind::X11
        );
    }

    #[test]
    fn without_a_session_type_the_display_variables_decide() {
        assert_eq!(
            kind(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")]),
            SessionKind::Wayland
        );
        assert_eq!(kind(&[("DISPLAY", ":1")]), SessionKind::X11);
        assert_eq!(kind(&[("XDG_SESSION_TYPE", "tty")]), SessionKind::Headless);
        assert_eq!(kind(&[]), SessionKind::Headless);
    }

    #[test]
    fn an_x11_session_without_a_display_is_headless() {
        assert_eq!(kind(&[("XDG_SESSION_TYPE", "x11")]), SessionKind::Headless);
        assert_eq!(
            kind(&[("XDG_SESSION_TYPE", "x11"), ("DISPLAY", "")]),
            SessionKind::Headless
        );
    }
}
