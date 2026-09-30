//! Pure presentation resolution: product snapshot -> wire `Presentation`.
//!
//! `revision` is strictly increasing WITHIN a session and resets with it, so a
//! restarted desktop at revision 1 is never mistaken for stale traffic
//! (design spec section 4.1).

use crate::action::gesture_value::ValueUpdate;
use kivori_model::presentation::{PrimaryState, ValueDisplay, ValueKind};
use kivori_protocol::message::Presentation;

/// How long a volume overlay stays up. Contract section 5 success-transient target.
const VALUE_TRANSIENT_MS: u16 = 800;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProductSnapshot {
    pub value: Option<ValueUpdate>,
    pub failed: bool,
}

impl ProductSnapshot {
    pub const fn idle() -> Self {
        Self {
            value: None,
            failed: false,
        }
    }
    /// A snapshot carrying one value update.
    ///
    /// A known failure carries no observable value, so it becomes [`Self::failed`] here rather
    /// than an overlay painting a percent nothing ever set: known failure and unknown outcome MUST
    /// stay distinct (user-story-contract invariant 4), and unavailability MUST be communicated
    /// rather than silently substituted (invariant 19). Routing it at this single choke point
    /// means every producer of a `ValueUpdate` is covered, not just the rotary path.
    pub const fn with_value(value: ValueUpdate) -> Self {
        if value.failed {
            return Self::failed();
        }
        Self {
            value: Some(value),
            failed: false,
        }
    }
    pub const fn failed() -> Self {
        Self {
            value: None,
            failed: true,
        }
    }
}

#[derive(Debug)]
pub struct PresentationResolver {
    session: u32,
    revision: u32,
}

impl PresentationResolver {
    pub const fn new(session: u32) -> Self {
        Self {
            session,
            revision: 0,
        }
    }

    /// Begin a new host session: rebind identity and restart revisions.
    pub fn begin_session(&mut self, session: u32) {
        self.session = session;
        self.revision = 0;
    }

    pub fn resolve(&mut self, snapshot: &ProductSnapshot) -> Presentation {
        self.revision = self.revision.saturating_add(1);

        let primary = if snapshot.failed {
            PrimaryState::Error
        } else {
            PrimaryState::Idle
        };

        let value = snapshot.value.map(|u| ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: u.percent,
            confidence: u.confidence,
            at_boundary: u.at_boundary,
        });

        Presentation {
            session: self.session,
            revision: self.revision,
            primary,
            value,
            transient_ms: if value.is_some() {
                VALUE_TRANSIENT_MS
            } else {
                0
            },
        }
    }
}
