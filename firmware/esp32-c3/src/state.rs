//! The device lifecycle FSM (data-model §1, FR-014).
//!
//! The device owns the `booting`/`offline` axis; the desktop owns the sendable states. This type
//! turns lifecycle events into the single companion state the device displays, and signals when that
//! state changed so the caller can emit a `StateReport`.

use kivori_model::{CompanionState, SendableState};

/// An event that may change the device's displayed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceEvent {
    /// Boot / self-test finished; the device is ready to show content.
    BootComplete,
    /// A host session became active.
    LinkUp,
    /// The host session was lost (unplug, timeout, or `Bye`).
    LinkDown,
    /// The host said it is going to sleep (`Bye(HostSleeping)`): show `Sleeping` and keep showing it
    /// across the link loss that follows, until a new host session starts.
    HostSleep,
    /// The host commanded a sendable state.
    SetState(SendableState),
}

/// The device lifecycle state machine. Starts in [`CompanionState::Booting`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceState {
    current: CompanionState,
    /// Set by `HostSleep`: a link loss keeps `Sleeping` instead of dropping to `Offline`. Cleared
    /// by `LinkUp` (a new host session).
    sleep_latched: bool,
}

impl DeviceState {
    /// A freshly-powered device, showing the boot scene.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            current: CompanionState::Booting,
            sleep_latched: false,
        }
    }

    /// The companion state the device is currently displaying.
    #[must_use]
    pub const fn current(&self) -> CompanionState {
        self.current
    }

    /// Applies `event`. Returns `Some(new_state)` iff the displayed state changed (the caller should
    /// then emit a `StateReport`); `None` if the event left the state unchanged.
    pub fn apply(&mut self, event: DeviceEvent) -> Option<CompanionState> {
        let next = match event {
            // Boot ends into `offline`: no host session exists yet, so nothing drives a sendable
            // state. A later host `SetState` moves it to idle/happy/busy/sleeping.
            DeviceEvent::BootComplete => {
                if matches!(self.current, CompanionState::Booting) {
                    CompanionState::Offline
                } else {
                    self.current
                }
            }
            // Losing the link drops to the device-owned `offline` state, unless the host announced
            // it is sleeping: then the buddy keeps sleeping until the host is back.
            DeviceEvent::LinkDown => {
                if self.sleep_latched {
                    CompanionState::Sleeping
                } else {
                    CompanionState::Offline
                }
            }
            // A new link changes nothing on its own (the host follows with a `SetState`), except
            // that it ends any sleep announced by the previous session.
            DeviceEvent::LinkUp => {
                self.sleep_latched = false;
                self.current
            }
            DeviceEvent::HostSleep => {
                self.sleep_latched = true;
                CompanionState::Sleeping
            }
            DeviceEvent::SetState(desired) => desired.to_companion(),
        };
        if next != self.current {
            self.current = next;
            Some(next)
        } else {
            None
        }
    }
}

impl Default for DeviceState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn booted() -> DeviceState {
        let mut d = DeviceState::new();
        d.apply(DeviceEvent::BootComplete);
        d
    }

    #[test]
    fn link_down_without_a_sleep_announcement_goes_offline() {
        let mut d = booted();
        d.apply(DeviceEvent::SetState(SendableState::Idle));
        assert_eq!(
            d.apply(DeviceEvent::LinkDown),
            Some(CompanionState::Offline)
        );
    }

    #[test]
    fn host_sleep_latches_sleeping_across_link_down() {
        let mut d = booted();
        d.apply(DeviceEvent::SetState(SendableState::Busy));
        assert_eq!(
            d.apply(DeviceEvent::HostSleep),
            Some(CompanionState::Sleeping)
        );
        assert_eq!(d.apply(DeviceEvent::LinkDown), None);
        assert_eq!(d.current(), CompanionState::Sleeping);
    }

    #[test]
    fn link_up_clears_the_latch() {
        let mut d = booted();
        d.apply(DeviceEvent::HostSleep);
        d.apply(DeviceEvent::LinkDown);
        assert_eq!(d.apply(DeviceEvent::LinkUp), None);
        assert_eq!(d.current(), CompanionState::Sleeping);
        assert_eq!(
            d.apply(DeviceEvent::LinkDown),
            Some(CompanionState::Offline)
        );
    }
}
