//! Preview ownership and reconciliation for the continuous volume value.
//!
//! While a gesture is open the local target owns the display (Preview). External
//! changes are recorded but never rendered over an active gesture. On gesture end
//! the value reconciles to what the backend reports (Confirmed) — desktop truth
//! wins (product invariants 1 and 30, US3, US4).

use super::volume::apply_step;
use super::{execute_volume, Outcome};
use crate::input::LogicalInput;
use crate::platform::VolumeBackend;
use kivori_model::input::Direction;
use kivori_model::presentation::ValueConfidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueUpdate {
    pub percent: u8,
    pub confidence: ValueConfidence,
    pub at_boundary: bool,
    /// The action was attempted and is known to have failed.
    ///
    /// Known failure, known success and unknown outcome MUST stay distinct
    /// (product invariant 4), so this is carried out of `GestureValue`
    /// separately from `confidence` — `Unverified` means "we could not observe the
    /// result", never "it did not happen". When set, `percent` is the last value the
    /// desktop actually knows about and MUST NOT be displayed as a volume: the
    /// snapshot resolves to `PrimaryState::Error` with no overlay instead.
    pub failed: bool,
}

#[derive(Debug, Default)]
pub struct GestureValue {
    /// `Some(gesture_id)` while a gesture owns the preview.
    active: Option<u16>,
    /// Gesture-local target, seeded from the backend when the gesture opens.
    target: u8,
    /// Set when an endpoint rebind invalidated the in-flight gesture.
    abandoned: bool,
    /// Detents staged since the last [`Self::flush`]: the stepped target and whether the last
    /// step was absorbed by the clamp. Not yet written to the backend.
    staged: Option<(u8, bool)>,
}

impl GestureValue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_input(
        &mut self,
        input: LogicalInput,
        backend: &dyn VolumeBackend,
    ) -> Option<ValueUpdate> {
        match input {
            LogicalInput::GestureStarted { gesture_id } => {
                self.active = Some(gesture_id);
                self.abandoned = false;
                self.target = backend.read().unwrap_or(self.target);
                None
            }
            LogicalInput::Detent {
                gesture_id,
                direction,
            } => {
                self.stage_detent(gesture_id, direction);
                self.flush(backend)
            }
            // The push switch never drives the continuous volume value.
            LogicalInput::Press { .. }
            | LogicalInput::Hold { .. }
            | LogicalInput::DoublePress { .. }
            | LogicalInput::ButtonPress { .. }
            | LogicalInput::ButtonHold { .. } => None,
            LogicalInput::GestureEnded { gesture_id } => {
                if self.active != Some(gesture_id) {
                    return None;
                }
                self.active = None;
                if self.abandoned {
                    self.abandoned = false;
                    return None;
                }
                let observed = backend.read().ok()?;
                self.target = observed;
                Some(ValueUpdate {
                    percent: observed,
                    confidence: ValueConfidence::Confirmed,
                    at_boundary: false,
                    failed: false,
                })
            }
        }
    }

    /// Steps the gesture-local target by one detent without touching the backend. Step math is
    /// applied per detent (`apply_step`), so clamping and direction reversal net exactly as if
    /// each detent had been written. Detents for another or abandoned gesture are ignored.
    pub fn stage_detent(&mut self, gesture_id: u16, direction: Direction) {
        if self.active != Some(gesture_id) || self.abandoned {
            return;
        }
        let from = self.staged.map_or(self.target, |(next, _)| next);
        self.staged = Some(apply_step(from, direction));
    }

    /// Writes the staged target with ONE backend round trip and reports it as one update, however
    /// many detents were staged. `None` when nothing is staged.
    pub fn flush(&mut self, backend: &dyn VolumeBackend) -> Option<ValueUpdate> {
        let (next, at_boundary) = self.staged.take()?;
        let confidence = match execute_volume(backend, next) {
            // A confirmed write still displays as Preview while the gesture
            // owns the surface; it is promoted at gesture end.
            Outcome::StateConfirmed { .. } => ValueConfidence::Preview,
            // Nothing happened — an unimplemented or unavailable backend is never
            // even written to. Do not advance the target and do not paint a percent
            // the desktop cannot observe (invariants 2 and 19); report the failure
            // as a failure (invariant 4).
            Outcome::Failed { .. } => {
                return Some(ValueUpdate {
                    percent: self.target,
                    confidence: ValueConfidence::Unverified,
                    at_boundary: false,
                    failed: true,
                })
            }
            Outcome::TriggeredUnverified => ValueConfidence::Unverified,
            _ => ValueConfidence::Unverified,
        };
        self.target = next;

        Some(ValueUpdate {
            percent: next,
            confidence,
            at_boundary,
            failed: false,
        })
    }

    /// The session ended. The device drops an open gesture on a session boundary without sending
    /// `GestureEnded`, so release the preview here or external changes would stay suppressed.
    pub fn end_session(&mut self) {
        self.active = None;
        self.staged = None;
        self.abandoned = false;
    }

    /// A change Kivori did not originate. Ignored while a gesture owns the preview.
    pub fn on_external_change(&mut self, percent: u8) -> Option<ValueUpdate> {
        if self.active.is_some() {
            return None;
        }
        self.target = percent;
        Some(ValueUpdate {
            percent,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
            failed: false,
        })
    }

    /// The default render endpoint changed. The value being controlled is now a
    /// different thing, so an in-flight gesture is abandoned rather than retargeted.
    pub fn on_endpoint_rebind(&mut self, percent: u8) -> Option<ValueUpdate> {
        if self.active.is_some() {
            self.abandoned = true;
        }
        self.staged = None;
        self.target = percent;
        Some(ValueUpdate {
            percent,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
            failed: false,
        })
    }
}
