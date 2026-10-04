//! Which face the buddy wears for what the desktop actually observed.
//!
//! Pure and allocation-free, shared by firmware and desktop preview. Every override comes from an
//! observed fact (invariant 2): a confirmation face only follows a confirmed outcome (gate 1, so
//! `Unverified` and `Processing` never celebrate), and the connection/sleep truth states are never
//! overridden (invariant 21).

use crate::desk::{ContextMood, DeskView, FeedbackKind, MediaStatus};
use crate::mascot::HAPPY_PRESS_Q8;
use crate::{CompanionState, MascotExpression};

/// A face (and cap press) that replaces the state's own for as long as the cause holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceOverride {
    /// The face to wear.
    pub expression: MascotExpression,
    /// Minimum cap press in Q8 pixels; apply as `max` with the pose's own press.
    pub press_q8: i32,
}

const fn face(expression: MascotExpression) -> Option<FaceOverride> {
    Some(FaceOverride {
        expression,
        press_q8: 0,
    })
}

/// The face override for `state` and `view`, or `None` to keep the state's own face.
///
/// First match wins: feedback error, confirmed-outcome celebration (both in Idle, Happy and Busy),
/// then, in Idle only: high load, muted, media playing, meeting profile.
#[must_use]
pub fn buddy_face(state: CompanionState, view: &DeskView) -> Option<FaceOverride> {
    if !matches!(
        state,
        CompanionState::Idle | CompanionState::Happy | CompanionState::Busy
    ) {
        return None;
    }
    match view.feedback.map(|f| f.kind) {
        Some(FeedbackKind::Error) => return face(MascotExpression::Error),
        Some(FeedbackKind::StateConfirmed | FeedbackKind::ExecutionConfirmed) => {
            return Some(FaceOverride {
                expression: MascotExpression::Happy,
                press_q8: HAPPY_PRESS_Q8,
            })
        }
        Some(FeedbackKind::Processing | FeedbackKind::Unverified) | None => {}
    }
    if state != CompanionState::Idle {
        return None;
    }
    let status = &view.status;
    if status.high_load {
        face(MascotExpression::Strained)
    } else if status.muted == Some(true) {
        face(MascotExpression::Muted)
    } else if status.media == Some(MediaStatus::Playing) {
        face(MascotExpression::Listening)
    } else if view
        .controls
        .is_some_and(|c| c.mood == ContextMood::Meeting)
    {
        face(MascotExpression::Attentive)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk::{ActionFeedback, ActionKind, ControlLabels, DeskStatus};

    fn feedback(kind: FeedbackKind) -> Option<ActionFeedback> {
        Some(ActionFeedback {
            action: ActionKind::Mute,
            kind,
        })
    }

    fn with(f: impl FnOnce(&mut DeskView)) -> DeskView {
        let mut view = DeskView::default();
        f(&mut view);
        view
    }

    fn expr(state: CompanionState, view: &DeskView) -> Option<MascotExpression> {
        buddy_face(state, view).map(|o| o.expression)
    }

    fn status(f: impl FnOnce(&mut DeskStatus)) -> DeskView {
        with(|v| f(&mut v.status))
    }

    #[test]
    fn nothing_observed_changes_no_face() {
        for state in CompanionState::ALL {
            assert_eq!(buddy_face(state, &DeskView::default()), None);
        }
    }

    #[test]
    fn error_feedback_shows_the_error_face() {
        let view = with(|v| v.feedback = feedback(FeedbackKind::Error));
        for state in [
            CompanionState::Idle,
            CompanionState::Happy,
            CompanionState::Busy,
        ] {
            let o = buddy_face(state, &view).unwrap();
            assert_eq!(o.expression, MascotExpression::Error);
            assert_eq!(o.press_q8, 0);
        }
    }

    #[test]
    fn confirmed_outcomes_celebrate_with_the_deep_press() {
        for kind in [
            FeedbackKind::StateConfirmed,
            FeedbackKind::ExecutionConfirmed,
        ] {
            let view = with(|v| v.feedback = feedback(kind));
            for state in [
                CompanionState::Idle,
                CompanionState::Happy,
                CompanionState::Busy,
            ] {
                assert_eq!(
                    buddy_face(state, &view),
                    Some(FaceOverride {
                        expression: MascotExpression::Happy,
                        press_q8: HAPPY_PRESS_Q8
                    })
                );
            }
        }
    }

    #[test]
    fn unverified_and_processing_never_celebrate() {
        for kind in [FeedbackKind::Unverified, FeedbackKind::Processing] {
            let view = with(|v| v.feedback = feedback(kind));
            for state in CompanionState::ALL {
                assert_eq!(buddy_face(state, &view), None, "{kind:?} in {state:?}");
            }
        }
    }

    #[test]
    fn unconfirmed_feedback_does_not_hide_a_real_status_face() {
        let view = with(|v| {
            v.feedback = feedback(FeedbackKind::Unverified);
            v.status.muted = Some(true);
        });
        assert_eq!(
            expr(CompanionState::Idle, &view),
            Some(MascotExpression::Muted)
        );
    }

    #[test]
    fn each_status_cue_has_its_own_face() {
        let idle = CompanionState::Idle;
        assert_eq!(
            expr(idle, &status(|s| s.high_load = true)),
            Some(MascotExpression::Strained)
        );
        assert_eq!(
            expr(idle, &status(|s| s.muted = Some(true))),
            Some(MascotExpression::Muted)
        );
        assert_eq!(expr(idle, &status(|s| s.muted = Some(false))), None);
        assert_eq!(expr(idle, &status(|s| s.muted = None)), None);
        assert_eq!(
            expr(idle, &status(|s| s.media = Some(MediaStatus::Playing))),
            Some(MascotExpression::Listening)
        );
        assert_eq!(
            expr(idle, &status(|s| s.media = Some(MediaStatus::Paused))),
            None
        );
        let meeting = with(|v| {
            v.controls = Some(ControlLabels {
                mood: ContextMood::Meeting,
                ..ControlLabels::default()
            })
        });
        assert_eq!(expr(idle, &meeting), Some(MascotExpression::Attentive));
        let neutral = with(|v| v.controls = Some(ControlLabels::default()));
        assert_eq!(expr(idle, &neutral), None);
    }

    #[test]
    fn priority_is_error_celebrate_load_mute_media_meeting() {
        let all = with(|v| {
            v.status.high_load = true;
            v.status.muted = Some(true);
            v.status.media = Some(MediaStatus::Playing);
            v.controls = Some(ControlLabels {
                mood: ContextMood::Meeting,
                ..ControlLabels::default()
            });
        });
        let idle = CompanionState::Idle;
        assert_eq!(expr(idle, &all), Some(MascotExpression::Strained));
        let mut v = all;
        v.status.high_load = false;
        assert_eq!(expr(idle, &v), Some(MascotExpression::Muted));
        v.status.muted = Some(false);
        assert_eq!(expr(idle, &v), Some(MascotExpression::Listening));
        v.status.media = None;
        assert_eq!(expr(idle, &v), Some(MascotExpression::Attentive));
        v.feedback = feedback(FeedbackKind::StateConfirmed);
        assert_eq!(expr(idle, &v), Some(MascotExpression::Happy));
        v.feedback = feedback(FeedbackKind::Error);
        assert_eq!(expr(idle, &v), Some(MascotExpression::Error));
    }

    #[test]
    fn status_cues_apply_only_in_idle() {
        let view = with(|v| {
            v.status.high_load = true;
            v.status.muted = Some(true);
            v.status.media = Some(MediaStatus::Playing);
        });
        for state in [CompanionState::Happy, CompanionState::Busy] {
            assert_eq!(buddy_face(state, &view), None, "{state:?}");
        }
    }

    #[test]
    fn booting_offline_and_sleeping_are_never_overridden() {
        let everything = with(|v| {
            v.feedback = feedback(FeedbackKind::Error);
            v.status.high_load = true;
            v.status.muted = Some(true);
            v.status.media = Some(MediaStatus::Playing);
        });
        let celebrate = with(|v| v.feedback = feedback(FeedbackKind::StateConfirmed));
        for state in [
            CompanionState::Booting,
            CompanionState::Offline,
            CompanionState::Sleeping,
        ] {
            assert_eq!(buddy_face(state, &everything), None, "{state:?}");
            assert_eq!(buddy_face(state, &celebrate), None, "{state:?}");
        }
    }

    #[test]
    fn override_faces_differ_from_every_state_and_each_other() {
        use MascotExpression::*;
        let pair = |e: MascotExpression| (e.eye_frame(), e.mouth_frame());
        let new = [Error, Strained, Muted, Listening, Attentive];
        for (i, a) in new.iter().enumerate() {
            for b in &new[i + 1..] {
                assert_ne!(pair(*a), pair(*b), "{a:?} vs {b:?}");
            }
            for state in CompanionState::ALL {
                assert_ne!(pair(*a), pair(MascotExpression::for_state(state)), "{a:?}");
            }
            for old in [Delighted, Affectionate, Laughing, Surprised, Reassuring] {
                assert_ne!(pair(*a), pair(old), "{a:?} vs {old:?}");
            }
        }
    }
}
