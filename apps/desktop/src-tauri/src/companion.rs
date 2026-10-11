//! Desktop-owned mascot behavior policy: personality settings and, for a device without idle life,
//! deterministic self-play timing.
//!
//! A device that negotiated `IDLE_LIFE_V1` schedules its own idle moments (`kivori_model::IdleLife`,
//! the same [`SelfPlay`] scheduler below plus glances) and the desktop only sends it the settings.
//! Older firmware gets this director instead, so its buddy stays lively.

use kivori_model::{CompanionState, MascotAction, MascotPersonality, SelfPlay};
use kivori_protocol::PlayMascotAction;

/// Schedules ambient social actions while leaving rendering and state semantics to shared/device code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionDirector {
    play: SelfPlay,
}

impl CompanionDirector {
    /// Creates a director and schedules its first ambient action after `now_ms`.
    #[must_use]
    pub fn new(personality: MascotPersonality, self_play: bool, seed: u32, now_ms: u32) -> Self {
        Self {
            play: SelfPlay::new(personality, self_play, seed, now_ms),
        }
    }

    /// Current user-selected personality.
    #[must_use]
    pub const fn personality(&self) -> MascotPersonality {
        self.play.personality()
    }

    /// Whether autonomous social actions are enabled.
    #[must_use]
    pub const fn self_play(&self) -> bool {
        self.play.enabled()
    }

    /// Next scheduled device-runtime millisecond, exposed for deterministic orchestration/tests.
    #[must_use]
    pub const fn next_due_ms(&self) -> u32 {
        self.play.next_due_ms()
    }

    /// Changes personality and starts a fresh cadence window.
    pub fn set_personality(&mut self, personality: MascotPersonality, now_ms: u32) {
        self.play.set_personality(personality, now_ms);
    }

    /// Enables or disables ambient actions and starts a fresh cadence window.
    pub fn set_self_play(&mut self, enabled: bool, now_ms: u32) {
        self.play.set_enabled(enabled, now_ms);
    }

    /// Returns one due autonomous action. Suppressed states consume and reschedule the opportunity.
    pub fn poll(&mut self, now_ms: u32, state: CompanionState) -> Option<PlayMascotAction> {
        let personality = self.play.personality();
        self.play
            .poll(now_ms, state)
            .map(|(action, seed)| PlayMascotAction {
                action,
                personality,
                seed,
            })
    }

    /// Builds one immediate user-requested action using current personality and a fresh seed.
    pub fn manual(&mut self, action: MascotAction) -> PlayMascotAction {
        PlayMascotAction {
            action,
            personality: self.play.personality(),
            seed: self.play.next_seed(),
        }
    }
}
