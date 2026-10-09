//! Host presence: sleep/wake, lock/unlock and "someone else owns the console".
//!
//! Each OS has a small source (`platform::windows::host_events`, `platform::macos::host_events`)
//! that turns its notifications into [`HostEvent`]s on a channel the device thread drains. The
//! decisions live here and are pure: [`HostPresenceTracker`] folds events into the current
//! [`HostPresence`] and says what the device thread must do ([`PresenceAction`]). Duplicate events
//! (Windows sends several resume notices per wake) change nothing.
//!
//! Lock deliberately does not touch the link: the Sleeping buddy is an orchestrator override and
//! the knob keeps working. Suspend and console loss release the serial port.

use std::sync::mpsc::SyncSender;
use std::time::Duration;

use kivori_model::SendableState;

/// How long the OS callback waits for the device thread to write its goodbye before it lets the
/// machine suspend anyway.
pub const SUSPEND_ACK_BUDGET: Duration = Duration::from_millis(500);

/// One thing the OS told us about the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostEvent {
    /// The machine is about to sleep or hibernate.
    Suspending,
    /// The machine woke up.
    Resumed,
    /// The screen locked.
    Locked,
    /// The screen unlocked.
    Unlocked,
    /// This user's session lost the console (fast user switching, remote session taking over).
    ConsoleDisconnected,
    /// This user's session has the console back.
    ConsoleConnected,
}

/// A [`HostEvent`] as it crosses to the device thread.
#[derive(Debug)]
pub struct HostSignal {
    pub event: HostEvent,
    /// Set only for [`HostEvent::Suspending`]: the OS callback blocks (for at most
    /// [`SUSPEND_ACK_BUDGET`]) until the device thread has written its goodbye and sends here.
    pub ack: Option<SyncSender<()>>,
}

impl HostSignal {
    #[must_use]
    pub const fn new(event: HostEvent) -> Self {
        Self { event, ack: None }
    }
}

/// What the machine is doing, as shown to the UI. Sleeping wins over locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostPresence {
    #[default]
    Active,
    Locked,
    Sleeping,
}

impl HostPresence {
    /// The token in `ConnectionStatusDto.host` (shared with the webview).
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Locked => "locked",
            Self::Sleeping => "sleeping",
        }
    }
}

/// The closed token list, mirrored by `HOST_PRESENCES` in the webview.
pub const HOST_PRESENCE_TOKENS: [&str; 3] = ["active", "locked", "sleeping"];

/// One step the device thread performs for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceAction {
    /// Tell the device the host is going away (`Bye`), if a session is up.
    SendSleepNotice,
    /// Drop the serial link and stop discovering until [`PresenceAction::RediscoverNow`].
    ReleaseLink,
    /// Forget any backoff and look for the device on the next tick.
    RediscoverNow,
    /// Show this state instead of the user's desired one (`None` restores it).
    StateOverride(Option<SendableState>),
    /// Stop (`true`) or resume (`false`) the buddy's ambient actions.
    PauseSelfPlay(bool),
}

/// Folds [`HostEvent`]s into the current presence.
#[derive(Debug, Clone, Copy, Default)]
pub struct HostPresenceTracker {
    sleeping: bool,
    locked: bool,
    console_lost: bool,
}

impl HostPresenceTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sleeping: false,
            locked: false,
            console_lost: false,
        }
    }

    /// Applies `event` and returns what to do. An event that changes nothing returns nothing.
    pub fn on_event(&mut self, event: HostEvent) -> Vec<PresenceAction> {
        match event {
            HostEvent::Suspending if !self.sleeping => {
                self.sleeping = true;
                vec![PresenceAction::SendSleepNotice, PresenceAction::ReleaseLink]
            }
            // A wake with no suspend seen is left to the heartbeat; dropping a live link on the
            // second of Windows' two resume notices would only force a needless reconnect.
            HostEvent::Resumed if self.sleeping => {
                self.sleeping = false;
                // A lock that outlasts the sleep stays: the override is still set.
                vec![PresenceAction::RediscoverNow]
            }
            HostEvent::Locked if !self.locked => {
                self.locked = true;
                vec![
                    PresenceAction::StateOverride(Some(SendableState::Sleeping)),
                    PresenceAction::PauseSelfPlay(true),
                ]
            }
            HostEvent::Unlocked if self.locked => {
                self.locked = false;
                vec![
                    PresenceAction::StateOverride(None),
                    PresenceAction::PauseSelfPlay(false),
                ]
            }
            HostEvent::ConsoleDisconnected if !self.console_lost => {
                self.console_lost = true;
                vec![PresenceAction::ReleaseLink]
            }
            HostEvent::ConsoleConnected if self.console_lost => {
                self.console_lost = false;
                vec![PresenceAction::RediscoverNow]
            }
            _ => Vec::new(),
        }
    }

    #[must_use]
    pub const fn presence(&self) -> HostPresence {
        if self.sleeping {
            HostPresence::Sleeping
        } else if self.locked {
            HostPresence::Locked
        } else {
            HostPresence::Active
        }
    }

    /// Whether the device thread may open the serial port. False while asleep or while another
    /// user session has the console, so the port stays free for that session's Kivori.
    #[must_use]
    pub const fn may_use_port(&self) -> bool {
        !self.sleeping && !self.console_lost
    }
}

/// The lock edge between two polls (macOS has no lock notification Kivori can use).
#[must_use]
pub const fn lock_edge(was_locked: bool, locked: bool) -> Option<HostEvent> {
    match (was_locked, locked) {
        (false, true) => Some(HostEvent::Locked),
        (true, false) => Some(HostEvent::Unlocked),
        _ => None,
    }
}

/// Why a host-event source could not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEventsError {
    /// The OS refused the registration. Kivori still works; it just cannot say goodnight.
    Unavailable(String),
}

impl std::fmt::Display for HostEventsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "host events unavailable: {reason}"),
        }
    }
}

impl std::error::Error for HostEventsError {}

/// Keeps a host-event source alive; dropping it stops the source.
pub struct HostEventsGuard {
    stop: Option<Box<dyn FnOnce() + Send>>,
}

impl HostEventsGuard {
    #[must_use]
    pub fn new(stop: impl FnOnce() + Send + 'static) -> Self {
        Self {
            stop: Some(Box::new(stop)),
        }
    }

    /// A source that emits nothing (targets without a backend).
    #[must_use]
    pub const fn inert() -> Self {
        Self { stop: None }
    }
}

impl Drop for HostEventsGuard {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use HostEvent::*;
    use PresenceAction::*;

    fn run(events: &[HostEvent]) -> (HostPresenceTracker, Vec<Vec<PresenceAction>>) {
        let mut tracker = HostPresenceTracker::new();
        let out = events.iter().map(|e| tracker.on_event(*e)).collect();
        (tracker, out)
    }

    #[test]
    fn lock_then_unlock_restores_the_users_state() {
        let (tracker, out) = run(&[Locked, Unlocked]);
        assert_eq!(
            out[0],
            [
                StateOverride(Some(SendableState::Sleeping)),
                PauseSelfPlay(true)
            ]
        );
        assert_eq!(out[1], [StateOverride(None), PauseSelfPlay(false)]);
        assert_eq!(tracker.presence(), HostPresence::Active);
    }

    #[test]
    fn suspend_while_locked_resumes_into_locked() {
        let (tracker, out) = run(&[Locked, Suspending]);
        assert_eq!(out[1], [SendSleepNotice, ReleaseLink]);
        assert_eq!(tracker.presence(), HostPresence::Sleeping);
        let mut tracker = tracker;
        assert_eq!(tracker.on_event(Resumed), [RediscoverNow]);
        assert_eq!(tracker.presence(), HostPresence::Locked);
        assert_eq!(tracker.on_event(Unlocked)[0], StateOverride(None));
    }

    #[test]
    fn resume_with_no_device_only_asks_for_rediscovery() {
        // No backoff action exists: the device thread clears any, and creates none.
        let (tracker, out) = run(&[Suspending, Resumed]);
        assert_eq!(out[1], [RediscoverNow]);
        assert_eq!(tracker.presence(), HostPresence::Active);
        assert!(tracker.may_use_port());
    }

    #[test]
    fn duplicate_events_change_nothing() {
        let (_, out) = run(&[
            Suspending,
            Suspending,
            Resumed,
            Resumed,
            Locked,
            Locked,
            Unlocked,
            Unlocked,
            ConsoleDisconnected,
            ConsoleDisconnected,
            ConsoleConnected,
            ConsoleConnected,
        ]);
        for index in [1, 3, 5, 7, 9, 11] {
            assert!(out[index].is_empty(), "event {index} should be a no-op");
        }
        // And an event for a state we were never in.
        let (_, out) = run(&[Resumed, Unlocked, ConsoleConnected]);
        assert!(out.iter().all(Vec::is_empty));
    }

    #[test]
    fn the_port_is_off_limits_while_asleep_or_away() {
        let mut tracker = HostPresenceTracker::new();
        assert!(tracker.may_use_port());
        tracker.on_event(Suspending);
        assert!(!tracker.may_use_port());
        tracker.on_event(ConsoleDisconnected);
        tracker.on_event(Resumed);
        assert!(!tracker.may_use_port(), "still another user's console");
        assert_eq!(tracker.on_event(ConsoleConnected), [RediscoverNow]);
        assert!(tracker.may_use_port());
        // Lock keeps the port: the knob still works under the lock screen.
        tracker.on_event(Locked);
        assert!(tracker.may_use_port());
    }

    #[test]
    fn lock_edges_come_from_polling() {
        assert_eq!(lock_edge(false, true), Some(Locked));
        assert_eq!(lock_edge(true, false), Some(Unlocked));
        assert_eq!(lock_edge(true, true), None);
        assert_eq!(lock_edge(false, false), None);
    }

    #[test]
    fn presence_tokens_match_the_shared_list() {
        let tokens = [
            HostPresence::Active,
            HostPresence::Locked,
            HostPresence::Sleeping,
        ]
        .map(HostPresence::token);
        assert_eq!(tokens, HOST_PRESENCE_TOKENS);
    }
}
