//! Input ingress: turns wire `InputEvent`s into validated `LogicalInput`.
//!
//! Two independent freshness rules (design spec section 4.1):
//!   1. the event's session MUST be the current session nonce;
//!   2. a gesture MUST have been observed to start in THIS session.
//!
//! Rule 1 alone is sufficient against a complete stale pair; rule 2 is retained as
//! defence in depth against intra-session reordering.

use kivori_model::input::Direction;
use kivori_protocol::message::{InputEvent, InputKind};
use std::collections::HashSet;

/// A validated, session-fresh input, ready for later tasks to bind to an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalInput {
    /// A gesture has begun.
    GestureStarted {
        /// The gesture identifier assigned by the device for this gesture.
        gesture_id: u16,
    },
    /// One completed, validated logical detent within an open gesture.
    Detent {
        /// The gesture this detent belongs to.
        gesture_id: u16,
        /// The direction of the detent.
        direction: Direction,
    },
    /// A gesture has ended.
    GestureEnded {
        /// The gesture identifier that ended.
        gesture_id: u16,
    },
}

/// Why an `InputEvent` was rejected by [`InputIngress::accept`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// No session is currently established.
    NoSession,
    /// The event belongs to a different session than the current one.
    StaleSession,
    /// No `GestureStarted` for this gesture was observed in this session.
    UnknownGesture,
}

/// A pure, self-contained state machine that admits only fresh, in-session input.
///
/// `InputIngress` does not observe connection state itself; it is told when
/// sessions begin and end via [`InputIngress::begin_session`] and
/// [`InputIngress::end_session`] (wired from `ConnectionManager` state
/// transitions in a later task).
#[derive(Debug, Default)]
pub struct InputIngress {
    session: Option<u32>,
    open_gestures: HashSet<u16>,
}

impl InputIngress {
    /// Creates an ingress with no active session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a new session, discarding any gestures left open by a previous one.
    pub fn begin_session(&mut self, session: u32) {
        self.session = Some(session);
        self.open_gestures.clear();
    }

    /// Ends the current session. Called on every exit from `Connected`.
    pub fn end_session(&mut self) {
        self.session = None;
        self.open_gestures.clear();
    }

    /// Validates one wire `InputEvent` against the current session and open-gesture state.
    pub fn accept(&mut self, event: &InputEvent) -> Result<Option<LogicalInput>, RejectReason> {
        let Some(current) = self.session else {
            return Err(RejectReason::NoSession);
        };
        if event.session != current {
            return Err(RejectReason::StaleSession);
        }

        match event.kind {
            InputKind::GestureStarted => {
                self.open_gestures.insert(event.gesture_id);
                Ok(Some(LogicalInput::GestureStarted {
                    gesture_id: event.gesture_id,
                }))
            }
            InputKind::Detent(direction) => {
                if !self.open_gestures.contains(&event.gesture_id) {
                    return Err(RejectReason::UnknownGesture);
                }
                Ok(Some(LogicalInput::Detent {
                    gesture_id: event.gesture_id,
                    direction,
                }))
            }
            InputKind::GestureEnded => {
                if !self.open_gestures.remove(&event.gesture_id) {
                    return Err(RejectReason::UnknownGesture);
                }
                Ok(Some(LogicalInput::GestureEnded {
                    gesture_id: event.gesture_id,
                }))
            }
        }
    }
}
