use kivori_model::{
    CompanionState, MascotAction, MascotAnimator, MascotExpression, MascotPersonality,
    MASCOT_TRANSITION_MS,
};

#[test]
fn interrupted_transition_starts_from_the_resolved_pose() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.set_state(CompanionState::Happy, 1_000);
    let before_interrupt = animator.pose_at(1_175);

    animator.set_state(CompanionState::Busy, 1_175);
    assert_eq!(animator.pose_at(1_175), before_interrupt);
}

#[test]
fn expression_swap_is_hidden_by_a_blink_instead_of_overlaying_faces() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.set_state(CompanionState::Happy, 1_000);

    let start = animator.pose_at(1_000);
    let before_swap = animator.pose_at(1_000 + MASCOT_TRANSITION_MS / 2 - 1);
    let halfway = animator.pose_at(1_000 + MASCOT_TRANSITION_MS / 2);
    let end = animator.pose_at(1_000 + MASCOT_TRANSITION_MS);

    assert_eq!(start.state_weights[CompanionState::Idle.index()], 255);
    assert_eq!(before_swap.state_weights[CompanionState::Idle.index()], 255);
    assert_eq!(halfway.state_weights[CompanionState::Happy.index()], 255);
    assert_eq!(
        halfway
            .state_weights
            .iter()
            .filter(|weight| **weight > 0)
            .count(),
        1,
        "two facial sprites must never be visible together"
    );
    assert!(
        halfway.eyes_scale_y_q8 <= 48,
        "the eyes close while the expression changes"
    );
    assert_eq!(end.state_weights[CompanionState::Happy.index()], 255);
}

#[test]
fn sleeping_transition_is_slower_than_the_standard_transition() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.set_state(CompanionState::Sleeping, 500);

    let standard_end = animator.pose_at(500 + MASCOT_TRANSITION_MS);
    assert!(standard_end.eyes_scale_y_q8 < 256);

    let settled = animator.pose_at(1_100);
    assert_eq!(settled.state_weights[CompanionState::Sleeping.index()], 255);
    assert_eq!(settled.eyes_scale_y_q8, 256);
}

#[test]
fn idle_life_keeps_the_silhouette_stable_while_face_moves() {
    let animator = MascotAnimator::new(CompanionState::Idle, 0);
    let resting = animator.pose_at(0);
    let mut saw_blink = false;
    let mut saw_gaze = false;
    let mut saw_expression = false;
    for now_ms in (0..36_000).step_by(20) {
        let pose = animator.pose_at(now_ms);
        assert_eq!(pose.body_offset_q8, resting.body_offset_q8);
        assert_eq!(pose.body_scale_q8, resting.body_scale_q8);
        saw_blink |= pose.eyes_scale_y_q8 < 100;
        saw_gaze |= pose.eye_offset_q8 != (0, 0);
        saw_expression |= pose.expression != MascotExpression::Idle;
    }
    assert!(saw_blink, "idle life must blink");
    assert!(saw_gaze, "idle life must glance around");
    assert!(saw_expression, "idle life must occasionally vary its face");
}

#[test]
fn idle_blink_schedule_is_seeded_varied_and_includes_double_blinks() {
    let mut gaps = Vec::new();
    for seed in [1, 7, 42, 0x4B49_564F] {
        let animator = MascotAnimator::new_seeded(CompanionState::Idle, 0, seed);
        let mut closed = false;
        let mut blink_centres = Vec::new();
        for now_ms in (0..60_000).step_by(20) {
            let is_closed = animator.pose_at(now_ms).eyes_scale_y_q8 < 100;
            if is_closed && !closed {
                blink_centres.push(now_ms);
            }
            closed = is_closed;
        }
        gaps.extend(blink_centres.windows(2).map(|pair| pair[1] - pair[0]));
    }
    assert!(gaps.iter().any(|gap| *gap < 500), "expected a double blink");
    assert!(
        gaps.windows(2).any(|pair| pair[0] != pair[1]),
        "blink spacing must vary"
    );

    let first = MascotAnimator::new_seeded(CompanionState::Idle, 0, 10);
    let same = MascotAnimator::new_seeded(CompanionState::Idle, 0, 10);
    let different = MascotAnimator::new_seeded(CompanionState::Idle, 0, 11);
    assert!((0..12_000).all(|time| first.pose_at(time) == same.pose_at(time)));
    assert!((0..12_000).any(|time| first.pose_at(time) != different.pose_at(time)));
}

#[test]
fn idle_expression_changes_happen_behind_closed_eyes() {
    let animator = MascotAnimator::new_seeded(CompanionState::Idle, 0, 1);
    let mut previous = animator.pose_at(0).expression;
    let mut swaps = 0;
    for now_ms in 1..60_000 {
        let pose = animator.pose_at(now_ms);
        if pose.expression != previous {
            swaps += 1;
            assert!(pose.eyes_scale_y_q8 <= 48);
            previous = pose.expression;
        }
    }
    assert!(swaps > 0);
}

#[test]
fn social_action_temporarily_overrides_expression_then_recovers_state() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.trigger_action(MascotAction::Pet, MascotPersonality::Cozy, 7, 1_000);

    assert_eq!(
        animator.pose_at(1_500).expression,
        MascotExpression::Affectionate
    );
    assert_eq!(animator.pose_at(3_300).expression, MascotExpression::Idle);
    assert_eq!(animator.target(), CompanionState::Idle);
}

#[test]
fn new_social_action_replaces_current_reaction_without_queueing() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.trigger_action(MascotAction::Comfort, MascotPersonality::Calm, 3, 1_000);
    animator.trigger_action(MascotAction::Surprise, MascotPersonality::Playful, 4, 1_300);

    assert_eq!(
        animator.pose_at(1_700).expression,
        MascotExpression::Surprised
    );
    assert_eq!(animator.pose_at(3_000).expression, MascotExpression::Idle);
}

#[test]
fn action_start_and_replacement_preserve_the_resolved_pose() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    let before = animator.pose_at(1_000);
    animator.trigger_action(MascotAction::Pet, MascotPersonality::Cozy, 1, 1_000);
    assert_eq!(animator.pose_at(1_000), before);

    let before_replacement = animator.pose_at(1_500);
    animator.trigger_action(MascotAction::Tickle, MascotPersonality::Playful, 2, 1_500);
    assert_eq!(animator.pose_at(1_500), before_replacement);
}

#[test]
fn action_recovery_returns_to_the_underlying_pose() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.trigger_action(MascotAction::Comfort, MascotPersonality::Calm, 3, 1_000);
    let base = MascotAnimator::new(CompanionState::Idle, 0);
    assert_eq!(animator.pose_at(3_600), base.pose_at(3_600));
}

#[test]
fn sleeping_reactions_keep_semantic_state_and_use_gentle_motion() {
    let mut animator = MascotAnimator::new(CompanionState::Sleeping, 0);
    animator.trigger_action(MascotAction::Tickle, MascotPersonality::Playful, 8, 1_000);

    let pose = animator.pose_at(1_600);
    assert_eq!(animator.target(), CompanionState::Sleeping);
    assert_eq!(pose.expression, MascotExpression::Affectionate);
    assert!(pose.body_offset_q8.0.abs() <= 256);
}

#[test]
fn gaze_and_wink_can_move_each_eye_independently() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.trigger_action(MascotAction::Greet, MascotPersonality::Playful, 11, 1_000);

    let pose = animator.pose_at(2_000);
    assert_ne!(pose.eye_offset_q8, (0, 0));
    assert_ne!(pose.left_eye_scale_y_q8, pose.right_eye_scale_y_q8);
}

/// PRD §9.5: a personality reaction must never wear a desktop state's face.
#[test]
fn no_reaction_face_matches_a_state_face() {
    let faces = |e: MascotExpression| (e.eye_frame(), e.mouth_frame());
    let actions = [
        MascotAction::Greet,
        MascotAction::Pet,
        MascotAction::Tickle,
        MascotAction::Surprise,
        MascotAction::Comfort,
    ];
    for action in actions {
        let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
        assert!(animator.trigger_action(action, MascotPersonality::Cozy, 1, 1_000));
        let face = faces(
            animator
                .pose_at(1_000 + action.duration_ms() / 2)
                .expression,
        );
        for state in CompanionState::ALL {
            assert_ne!(
                face,
                faces(MascotExpression::for_state(state)),
                "{action:?} looks like {state:?}"
            );
        }
    }
}

/// PRD §9.5: reactions give way to meaningful states.
#[test]
fn reactions_are_ignored_over_meaningful_states() {
    for state in CompanionState::ALL {
        let mut animator = MascotAnimator::new(state, 0);
        let played =
            animator.trigger_action(MascotAction::Greet, MascotPersonality::Playful, 1, 10);
        assert_eq!(played, state.allows_reaction(), "{state:?}");
        if !played {
            assert_eq!(
                animator.pose_at(500),
                MascotAnimator::new(state, 0).pose_at(500)
            );
        }
    }
}
