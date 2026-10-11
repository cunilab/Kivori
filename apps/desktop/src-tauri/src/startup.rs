//! Pure startup policy for the splash screen (M2.6).
//!
//! The main window stays hidden until the frontend reports ready. A slow start shows a small splash
//! meanwhile; a fast start never flashes one; a login launch shows neither. `runtime::splash` binds
//! these decisions to real windows and the clock.

use std::time::Duration;

/// A start that is ready sooner than this never shows the splash.
pub const SPLASH_DELAY: Duration = Duration::from_millis(300);

/// How long the main window may stay hidden before it is shown anyway. A broken frontend (a script
/// error before it reports ready, a webview that never loads) must not leave the user with only a
/// splash, or with nothing at all. Well above any real start, so it never fires on a healthy one.
pub const READY_FALLBACK: Duration = Duration::from_secs(10);

/// Whether the splash should be on screen now: still starting, not a login launch, past the delay.
#[must_use]
pub fn splash_due(hidden_start: bool, ready: bool, elapsed: Duration) -> bool {
    !hidden_start && !ready && elapsed >= SPLASH_DELAY
}

/// Whether to give up waiting for the frontend and show the main window regardless.
#[must_use]
pub fn fallback_due(hidden_start: bool, ready: bool, elapsed: Duration) -> bool {
    !hidden_start && !ready && elapsed >= READY_FALLBACK
}

/// The one status line the splash shows. Real phases only, never a percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPhase {
    /// The app core and window are still coming up.
    Starting,
    /// The core is up and has begun looking for a Kivori.
    LookingForDevice,
}

impl StartupPhase {
    /// The token the splash page maps to its text.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::LookingForDevice => "looking",
        }
    }
}

/// The phase for the latest connection snapshot: "looking" once the device task has left its
/// initial `disconnected` state or counted an attempt, "starting" before that.
#[must_use]
pub fn phase_for(connection: &str, retry_count: u32) -> StartupPhase {
    if connection == "disconnected" && retry_count == 0 {
        StartupPhase::Starting
    } else {
        StartupPhase::LookingForDevice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn a_fast_start_never_shows_the_splash() {
        assert!(!splash_due(false, true, MS(250)));
        assert!(!splash_due(false, true, MS(2_000)));
        assert!(!splash_due(false, false, MS(299)));
    }

    #[test]
    fn a_slow_start_shows_it_after_the_delay() {
        assert!(splash_due(false, false, SPLASH_DELAY));
        assert!(splash_due(false, false, MS(5_000)));
    }

    #[test]
    fn a_login_launch_never_shows_splash_or_forces_the_window() {
        assert!(!splash_due(true, false, MS(5_000)));
        assert!(!fallback_due(true, false, READY_FALLBACK * 3));
    }

    #[test]
    fn the_fallback_fires_only_when_not_ready_and_late() {
        assert!(!fallback_due(false, false, READY_FALLBACK - MS(1)));
        assert!(fallback_due(false, false, READY_FALLBACK));
        assert!(!fallback_due(false, true, READY_FALLBACK * 2));
    }

    #[test]
    fn the_fallback_comes_after_the_splash() {
        assert!(READY_FALLBACK > SPLASH_DELAY);
    }

    #[test]
    fn phase_follows_the_connection_snapshot() {
        assert_eq!(phase_for("disconnected", 0), StartupPhase::Starting);
        assert_eq!(phase_for("connecting", 0), StartupPhase::LookingForDevice);
        assert_eq!(phase_for("disconnected", 2), StartupPhase::LookingForDevice);
        assert_eq!(StartupPhase::Starting.token(), "starting");
        assert_eq!(StartupPhase::LookingForDevice.token(), "looking");
    }
}
