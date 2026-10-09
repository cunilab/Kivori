//! The desktop orchestrator's desired-state ownership (data-model §4; clarified).
//!
//! The orchestrator is the single source of truth for the state the desktop wants the device to show.
//! It defaults to `Idle`, resets to `Idle` on every cold start (no persistence), and is re-transmitted
//! to the device on each (re)connect (FR-009 within-process restoration).

use kivori_model::SendableState;

/// Owns the desktop's desired companion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Orchestrator {
    desired: SendableState,
    /// A state the host imposes while it is away (a locked screen). It hides `desired` from the
    /// device without losing it.
    host_override: Option<SendableState>,
}

impl Orchestrator {
    /// A cold-start orchestrator: desired state is `Idle` (no persistence across launches).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            desired: SendableState::Idle,
            host_override: None,
        }
    }

    /// The state the device should show: the host override if one is set, else the user's.
    #[must_use]
    pub const fn desired(&self) -> SendableState {
        match self.host_override {
            Some(state) => state,
            None => self.desired,
        }
    }

    /// The state the user (or an integration) asked for, whatever the host is imposing.
    #[must_use]
    pub const fn user_desired(&self) -> SendableState {
        self.desired
    }

    /// The imposed state, if any.
    #[must_use]
    pub const fn host_override(&self) -> Option<SendableState> {
        self.host_override
    }

    /// Imposes `state` on the device (`None` lifts it). The user's desired state is untouched, so
    /// lifting restores it exactly.
    pub fn set_host_override(&mut self, state: Option<SendableState>) {
        self.host_override = state;
    }

    /// Sets the user's desired state (the production path — a future integration calls the same
    /// method). Returns the state the device should now show, which a host override still wins.
    pub fn set_desired(&mut self, state: SendableState) -> SendableState {
        self.desired = state;
        self.desired()
    }

    /// The state to (re)transmit to the device on (re)connect to resynchronize it (FR-009).
    #[must_use]
    pub const fn resync_state(&self) -> SendableState {
        self.desired()
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_override_hides_the_users_state_and_lifting_it_restores_it() {
        let mut orchestrator = Orchestrator::new();
        orchestrator.set_desired(SendableState::Busy);
        orchestrator.set_host_override(Some(SendableState::Sleeping));
        assert_eq!(orchestrator.desired(), SendableState::Sleeping);
        assert_eq!(orchestrator.resync_state(), SendableState::Sleeping);
        assert_eq!(orchestrator.user_desired(), SendableState::Busy);
        orchestrator.set_host_override(None);
        assert_eq!(orchestrator.desired(), SendableState::Busy);
        assert_eq!(orchestrator.host_override(), None);
    }

    #[test]
    fn a_change_made_under_an_override_is_kept_for_after() {
        let mut orchestrator = Orchestrator::new();
        orchestrator.set_host_override(Some(SendableState::Sleeping));
        assert_eq!(
            orchestrator.set_desired(SendableState::Happy),
            SendableState::Sleeping
        );
        orchestrator.set_host_override(None);
        assert_eq!(orchestrator.desired(), SendableState::Happy);
    }
}
