//! Rotary gesture formation: identity plus the inactivity boundary.
//!
//! A gesture opens on the first validated detent and closes after
//! `gesture_end_ms` with no new detent (user-story-contract section 5). A
//! direction reversal mid-gesture does not split it: one physical twiddle back
//! and forth is one gesture. Gesture identity is session-unique only —
//! [`RotaryGesture::reset`] restarts numbering, which is what a host session
//! ending looks like.

use kivori_model::input::Direction;

/// One gesture-lifecycle event emitted by [`RotaryGesture`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotaryEvent {
    /// A new gesture opened. Emitted once, on the first detent of the gesture.
    GestureStarted {
        /// Session-unique identifier of the gesture that just opened.
        gesture_id: u16,
    },
    /// One validated detent belonging to a gesture.
    Detent {
        /// Identifier of the gesture this detent belongs to.
        gesture_id: u16,
        /// Rotational direction of the detent.
        direction: Direction,
    },
    /// A gesture closed after its inactivity window elapsed with no new detent.
    /// Emitted exactly once per gesture.
    GestureEnded {
        /// Identifier of the gesture that just closed.
        gesture_id: u16,
    },
}

/// Groups validated logical detents (from the quadrature decoder) into gestures.
///
/// A gesture opens on the first detent and stays open, regardless of direction
/// reversals, until [`poll`](Self::poll) observes that `gesture_end_ms` has
/// elapsed since the last detent. The inactivity window is a tuning target
/// (user-story-contract section 5), so it is a constructor parameter rather
/// than a hardcoded constant.
#[derive(Debug)]
pub struct RotaryGesture {
    /// Inactivity window, in milliseconds, after which an open gesture ends.
    gesture_end_ms: u32,
    /// Identifier of the most recently opened gesture. Session-unique.
    last_id: u16,
    /// `Some((id, last_detent_ms))` while a gesture is open.
    open: Option<(u16, u32)>,
}

impl RotaryGesture {
    /// Creates a gesture former with no open gesture and the given inactivity
    /// window, in milliseconds.
    pub const fn new(gesture_end_ms: u32) -> Self {
        Self {
            gesture_end_ms,
            last_id: 0,
            open: None,
        }
    }

    /// Records one validated detent at `now_ms`.
    ///
    /// Returns `(Some(GestureStarted), Detent)` when this detent opened a new
    /// gesture, and `(None, Detent)` when it continued the currently open one.
    /// A direction reversal never opens a new gesture on its own.
    pub fn on_detent(
        &mut self,
        direction: Direction,
        now_ms: u32,
    ) -> (Option<RotaryEvent>, RotaryEvent) {
        let mut started = None;

        let gesture_id = match self.open {
            Some((id, _)) => id,
            None => {
                self.last_id = self.last_id.wrapping_add(1);
                if self.last_id == 0 {
                    // Never hand out id 0; it reads as "no gesture".
                    self.last_id = 1;
                }
                started = Some(RotaryEvent::GestureStarted {
                    gesture_id: self.last_id,
                });
                self.last_id
            }
        };

        self.open = Some((gesture_id, now_ms));
        (
            started,
            RotaryEvent::Detent {
                gesture_id,
                direction,
            },
        )
    }

    /// Closes the open gesture once the inactivity window has elapsed since its
    /// last detent, as of `now_ms`.
    ///
    /// Returns `Some(GestureEnded)` exactly once per gesture — the first poll
    /// at or after the boundary — and `None` otherwise, including when no
    /// gesture is open.
    pub fn poll(&mut self, now_ms: u32) -> Option<RotaryEvent> {
        let (id, last_ms) = self.open?;
        if now_ms.wrapping_sub(last_ms) >= self.gesture_end_ms {
            self.open = None;
            return Some(RotaryEvent::GestureEnded { gesture_id: id });
        }
        None
    }

    /// Drops any open gesture without emitting `GestureEnded`, and restarts
    /// gesture numbering.
    ///
    /// Used when a host session ends: a gesture from the old session must not
    /// be completable in a new one.
    pub fn reset(&mut self) {
        self.open = None;
        self.last_id = 0;
    }
}
