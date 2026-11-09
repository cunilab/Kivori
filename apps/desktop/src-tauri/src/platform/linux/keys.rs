//! Key synthesis for Linux.
//!
//! MEDIA KEYS go to the media player over MPRIS first (see `media`): that works on X11 and Wayland
//! and needs no synthesized input. Only when no player is reachable on the bus does an X11 session
//! fall back to the key event (`XF86AudioPlay` and friends through `platform::synth`). A player that
//! was found and refused, or that did not answer in time, is never retried as a key event, so one
//! press is never delivered twice.
//!
//! SHORTCUTS need synthesized keyboard input, which exists on X11 only (XTest, through enigo's
//! pure-Rust `x11rb` backend). On Wayland the only X server there is XWayland, and events injected
//! into it reach XWayland clients only: a shortcut would silently miss every native Wayland window,
//! which is most of a GNOME desktop. That is a wrong outcome reported as success, so a Wayland
//! session reports shortcuts as `RuntimeUnavailable` and sends nothing (invariant 19).

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::media::{Mpris, Sent};
use super::session::{session_kind, SessionKind};
use super::x11::{X11Watch, XHealth};
use crate::platform::synth::EnigoInputSynth;
use crate::platform::{
    ActionAvailability, ActionError, ConfirmationClass, InputGuard, InputSynth, MediaKey, Shortcut,
};

/// Why shortcuts are unavailable on a Wayland session. Shown in the UI.
pub const WAYLAND_SHORTCUTS_REASON: &str =
    "Keyboard shortcuts can't be sent from Wayland yet. Log in with an X11 session to use them.";
/// Why shortcuts are unavailable with no graphical session.
pub const HEADLESS_SHORTCUTS_REASON: &str = "Keyboard shortcuts need a graphical session";

/// Why shortcuts are unavailable in an X11 session whose server cannot take injected keys.
pub const X11_UNREACHABLE_REASON: &str =
    "Keyboard shortcuts need the X server, which isn't reachable or lacks the XTEST extension";
/// Why shortcuts are unavailable while the X server is not answering.
pub const X11_UNRESPONSIVE_REASON: &str =
    "Keyboard shortcuts are paused: the X server isn't responding";

/// An MPRIS call is bounded by its own method timeout and the discovery by the request's deadline;
/// this caps the whole send.
const SEND_CEILING: Duration = Duration::from_secs(2);

/// What the session allows for shortcuts. On X11 the answer follows the backend's health (a recent
/// look at the X server: reachable, with XTEST), not just a `DISPLAY` variable.
fn shortcut_availability(session: SessionKind, x11: &dyn XHealth) -> ActionAvailability {
    match session {
        SessionKind::X11 => match x11.xtest() {
            Some(true) => ActionAvailability::Available {
                confirmation: ConfirmationClass::TriggeredUnverified,
            },
            Some(false) => ActionAvailability::RuntimeUnavailable {
                reason: X11_UNREACHABLE_REASON.to_string(),
            },
            None => ActionAvailability::RuntimeUnavailable {
                reason: X11_UNRESPONSIVE_REASON.to_string(),
            },
        },
        SessionKind::Wayland => ActionAvailability::RuntimeUnavailable {
            reason: WAYLAND_SHORTCUTS_REASON.to_string(),
        },
        SessionKind::Headless => ActionAvailability::RuntimeUnavailable {
            reason: HEADLESS_SHORTCUTS_REASON.to_string(),
        },
    }
}

/// Sends one media command to a player. Injected so tests never touch the session bus.
pub(super) trait MediaSend: Send + Sync {
    fn send(&self, key: MediaKey, guard: &InputGuard) -> Sent;
}

impl MediaSend for Mpris {
    fn send(&self, key: MediaKey, guard: &InputGuard) -> Sent {
        self.send_key(key, guard)
    }
}

/// An X11 session's health is the shared X worker's; any other session has no X to ask.
struct NoX;

impl XHealth for NoX {
    fn xtest(&self) -> Option<bool> {
        Some(false)
    }
}

/// Key synthesis for Linux: MPRIS for media keys, X11 key events for the rest.
pub struct LinuxInputSynth {
    session: SessionKind,
    player: Arc<dyn MediaSend>,
    x11: Arc<dyn InputSynth>,
    health: Arc<dyn XHealth>,
}

impl LinuxInputSynth {
    #[must_use]
    pub fn new(mpris: Arc<Mpris>) -> Self {
        // No main-thread hop: an X connection is fine on any thread.
        let session = session_kind();
        let health: Arc<dyn XHealth> = if session == SessionKind::X11 {
            X11Watch::shared()
        } else {
            Arc::new(NoX)
        };
        Self::with_parts(session, mpris, Arc::new(EnigoInputSynth::new(None)), health)
    }

    fn with_parts(
        session: SessionKind,
        player: Arc<dyn MediaSend>,
        x11: Arc<dyn InputSynth>,
        health: Arc<dyn XHealth>,
    ) -> Self {
        Self {
            session,
            player,
            x11,
            health,
        }
    }
}

impl InputSynth for LinuxInputSynth {
    fn send_media_key(&self, key: MediaKey, guard: InputGuard) -> Result<(), ActionError> {
        guard.check()?;
        let started = Instant::now();
        match self.player.send(key, &guard) {
            Sent::Yes => Ok(()),
            Sent::Failed(reason) => Err(ActionError::Failed(reason)),
            Sent::Unknown => Err(ActionError::Uncertain),
            // Nothing was sent: the request ran out of time or was replaced while the player was
            // being found.
            Sent::Expired => {
                guard.check()?;
                Err(ActionError::Failed(
                    "the media player did not answer in time".into(),
                ))
            }
            Sent::NoPlayer => {
                // The bus call may have used up the request's time.
                guard.check()?;
                if started.elapsed() > SEND_CEILING {
                    return Err(ActionError::Uncertain);
                }
                match self.session {
                    SessionKind::X11 => self.x11.send_media_key(key, guard),
                    _ => Err(ActionError::Failed("no media player is running".into())),
                }
            }
        }
    }

    fn send_shortcut(&self, shortcut: &Shortcut, guard: InputGuard) -> Result<(), ActionError> {
        match shortcut_availability(self.session, &*self.health) {
            ActionAvailability::Available { .. } => self.x11.send_shortcut(shortcut, guard),
            ActionAvailability::RuntimeUnavailable { reason } => Err(ActionError::Failed(reason)),
            _ => Err(ActionError::Failed("shortcuts are unavailable".into())),
        }
    }

    fn keyboard_availability(&self) -> Option<ActionAvailability> {
        Some(shortcut_availability(self.session, &*self.health))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Player(Mutex<Vec<MediaKey>>, Sent);

    impl MediaSend for Player {
        fn send(&self, key: MediaKey, _: &InputGuard) -> Sent {
            self.0.lock().unwrap().push(key);
            self.1.clone()
        }
    }

    #[derive(Default)]
    struct Keys {
        media: Mutex<Vec<MediaKey>>,
        shortcuts: Mutex<usize>,
    }

    impl InputSynth for Keys {
        fn send_media_key(&self, key: MediaKey, _: InputGuard) -> Result<(), ActionError> {
            self.media.lock().unwrap().push(key);
            Ok(())
        }
        fn send_shortcut(&self, _: &Shortcut, _: InputGuard) -> Result<(), ActionError> {
            *self.shortcuts.lock().unwrap() += 1;
            Ok(())
        }
    }

    fn guard() -> InputGuard {
        InputGuard {
            deadline: None,
            current: Arc::new(|| true),
        }
    }

    /// A fixed X server health.
    struct Health(Option<bool>);

    impl XHealth for Health {
        fn xtest(&self) -> Option<bool> {
            self.0
        }
    }

    fn synth_with(
        session: SessionKind,
        outcome: Sent,
        health: Option<bool>,
    ) -> (LinuxInputSynth, Arc<Player>, Arc<Keys>) {
        let player = Arc::new(Player(Mutex::default(), outcome));
        let keys = Arc::new(Keys::default());
        (
            LinuxInputSynth::with_parts(
                session,
                player.clone(),
                keys.clone(),
                Arc::new(Health(health)),
            ),
            player,
            keys,
        )
    }

    fn synth(session: SessionKind, outcome: Sent) -> (LinuxInputSynth, Arc<Player>, Arc<Keys>) {
        synth_with(session, outcome, Some(true))
    }

    #[test]
    fn media_keys_go_to_the_player_and_never_the_key_event() {
        let (synth, player, keys) = synth(SessionKind::X11, Sent::Yes);
        for key in [MediaKey::PlayPause, MediaKey::Previous, MediaKey::Next] {
            assert_eq!(synth.send_media_key(key, guard()), Ok(()));
        }
        assert_eq!(
            *player.0.lock().unwrap(),
            [MediaKey::PlayPause, MediaKey::Previous, MediaKey::Next]
        );
        assert!(keys.media.lock().unwrap().is_empty());
    }

    #[test]
    fn with_no_player_an_x11_session_falls_back_to_the_key_event() {
        let (synth, _, keys) = synth(SessionKind::X11, Sent::NoPlayer);
        assert_eq!(synth.send_media_key(MediaKey::Next, guard()), Ok(()));
        assert_eq!(*keys.media.lock().unwrap(), [MediaKey::Next]);
    }

    #[test]
    fn with_no_player_wayland_reports_a_failure_instead_of_a_silent_miss() {
        for session in [SessionKind::Wayland, SessionKind::Headless] {
            let (synth, _, keys) = synth(session, Sent::NoPlayer);
            assert!(matches!(
                synth.send_media_key(MediaKey::PlayPause, guard()),
                Err(ActionError::Failed(_))
            ));
            assert!(keys.media.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn a_refusing_or_slow_player_is_never_doubled_by_the_key_event() {
        let (slow, _, slow_keys) = synth(SessionKind::X11, Sent::Unknown);
        assert_eq!(
            slow.send_media_key(MediaKey::PlayPause, guard()),
            Err(ActionError::Uncertain)
        );
        let (refusing, _, refusing_keys) = synth(SessionKind::X11, Sent::Failed("refused".into()));
        assert_eq!(
            refusing.send_media_key(MediaKey::PlayPause, guard()),
            Err(ActionError::Failed("refused".into()))
        );
        assert!(slow_keys.media.lock().unwrap().is_empty());
        assert!(refusing_keys.media.lock().unwrap().is_empty());
    }

    #[test]
    fn a_stale_request_sends_nothing() {
        let (synth, player, keys) = synth(SessionKind::X11, Sent::Yes);
        let stale = InputGuard {
            deadline: None,
            current: Arc::new(|| false),
        };
        assert!(synth.send_media_key(MediaKey::Next, stale).is_err());
        assert!(player.0.lock().unwrap().is_empty());
        assert!(keys.media.lock().unwrap().is_empty());
    }

    #[test]
    fn shortcuts_use_the_key_event_on_x11() {
        let (synth, player, keys) = synth(SessionKind::X11, Sent::Yes);
        let shortcut: Shortcut = "Ctrl+M".parse().unwrap();
        assert_eq!(synth.send_shortcut(&shortcut, guard()), Ok(()));
        assert_eq!(*keys.shortcuts.lock().unwrap(), 1);
        assert!(player.0.lock().unwrap().is_empty());
    }

    #[test]
    fn shortcuts_send_nothing_and_say_why_on_wayland() {
        let (synth, _, keys) = synth(SessionKind::Wayland, Sent::Yes);
        let shortcut: Shortcut = "Ctrl+M".parse().unwrap();
        assert_eq!(
            synth.send_shortcut(&shortcut, guard()),
            Err(ActionError::Failed(WAYLAND_SHORTCUTS_REASON.into()))
        );
        assert_eq!(*keys.shortcuts.lock().unwrap(), 0);
    }

    #[test]
    fn an_expired_media_request_reports_a_failure_and_never_falls_back() {
        let (synth, _, keys) = synth(SessionKind::X11, Sent::Expired);
        assert!(matches!(
            synth.send_media_key(MediaKey::Next, guard()),
            Err(ActionError::Failed(_))
        ));
        assert!(keys.media.lock().unwrap().is_empty());
    }

    #[test]
    fn x11_shortcuts_follow_the_health_of_the_x_server_not_just_display() {
        let availability = |health| {
            synth_with(SessionKind::X11, Sent::Yes, health)
                .0
                .keyboard_availability()
                .expect("Linux reports its own")
        };
        assert!(matches!(
            availability(Some(true)),
            ActionAvailability::Available { .. }
        ));
        assert_eq!(
            availability(Some(false)),
            ActionAvailability::RuntimeUnavailable {
                reason: X11_UNREACHABLE_REASON.into()
            },
            "DISPLAY is set but X/XTEST is not reachable"
        );
        assert_eq!(
            availability(None),
            ActionAvailability::RuntimeUnavailable {
                reason: X11_UNRESPONSIVE_REASON.into()
            }
        );
        // And an unhealthy server sends nothing.
        let (synth, _, keys) = synth_with(SessionKind::X11, Sent::Yes, Some(false));
        let shortcut: Shortcut = "Ctrl+M".parse().unwrap();
        assert!(synth.send_shortcut(&shortcut, guard()).is_err());
        assert_eq!(*keys.shortcuts.lock().unwrap(), 0);
    }

    #[test]
    fn keyboard_availability_follows_the_session() {
        let availability = |session| {
            synth(session, Sent::Yes)
                .0
                .keyboard_availability()
                .expect("Linux reports its own")
        };
        assert_eq!(
            availability(SessionKind::X11),
            ActionAvailability::Available {
                confirmation: ConfirmationClass::TriggeredUnverified
            }
        );
        assert_eq!(
            availability(SessionKind::Wayland),
            ActionAvailability::RuntimeUnavailable {
                reason: WAYLAND_SHORTCUTS_REASON.into()
            }
        );
        assert_eq!(
            availability(SessionKind::Headless),
            ActionAvailability::RuntimeUnavailable {
                reason: HEADLESS_SHORTCUTS_REASON.into()
            }
        );
    }
}
