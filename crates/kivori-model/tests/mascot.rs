use kivori_model::{
    CompanionState, IdleLife, LookTarget, MascotAction, MascotAnimator, MascotExpression,
    MascotPersonality, SelfPlay, MASCOT_TRANSITION_MS,
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

/// docs/product.md, display and buddy: a personality reaction must never wear a desktop state's face.
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

/// docs/product.md, display and buddy: reactions give way to meaningful states.
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

#[test]
fn only_happy_presses_the_cap_and_the_press_eases_in_and_out() {
    for state in CompanionState::ALL {
        let expected = if state == CompanionState::Happy {
            16 * 256
        } else {
            0
        };
        let pose = MascotAnimator::new(state, 0).pose_at(5_000);
        assert_eq!(pose.press_q8, expected, "{state:?}");
    }
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.set_state(CompanionState::Happy, 1_000);
    let entering: Vec<i32> = (1_000..=1_350)
        .step_by(50)
        .map(|ms| animator.pose_at(ms).press_q8)
        .collect();
    assert_eq!(entering.first(), Some(&0));
    assert_eq!(entering.last(), Some(&(16 * 256)));
    assert!(entering.windows(2).all(|w| w[0] <= w[1]), "{entering:?}");
    assert!(entering.iter().any(|&p| p > 0 && p < 16 * 256));
    animator.set_state(CompanionState::Busy, 2_000);
    assert_eq!(animator.pose_at(2_000).press_q8, 16 * 256);
    assert_eq!(animator.pose_at(2_350).press_q8, 0);
}

/// docs/product.md, display and buddy: idle life never wears a desktop state's face. Every pose of
/// every new idle moment, swept over time, keeps the state's own face or a reaction-style one.
#[test]
fn idle_moments_never_look_like_another_state() {
    let faces = |e: MascotExpression| (e.eye_frame(), e.mouth_frame());
    let controls = [
        LookTarget::Knob,
        LookTarget::Button(0),
        LookTarget::Button(1),
        LookTarget::Button(2),
    ];
    for state in [
        CompanionState::Idle,
        CompanionState::Happy,
        CompanionState::Busy,
        CompanionState::Offline,
    ] {
        for evening in [false, true] {
            for control in controls {
                let mut animator = MascotAnimator::new(state, 0);
                animator.set_evening(evening);
                animator.glance(768, 1_000);
                animator.look_at(control, 20_000);
                for ms in (0..40_000).step_by(37) {
                    let face = faces(animator.pose_at(ms).expression);
                    let own = faces(MascotExpression::for_state(state));
                    if face == own {
                        continue;
                    }
                    assert_eq!(state, CompanionState::Idle, "{state:?} changed its face");
                    for other in CompanionState::ALL {
                        assert_ne!(
                            face,
                            faces(MascotExpression::for_state(other)),
                            "idle life looks like {other:?}"
                        );
                    }
                }
            }
        }
    }
}

/// docs/product.md: outside Sleeping the eyes never fully close, so idle life never reads as Sleeping.
#[test]
fn eyes_never_fully_close_outside_sleeping() {
    for state in CompanionState::ALL {
        if state == CompanionState::Sleeping {
            continue;
        }
        for evening in [false, true] {
            let mut animator = MascotAnimator::new_seeded(state, 0, 0x1234_5678);
            animator.set_evening(evening);
            // Skip the Booting open-up: it starts from the same floor.
            for ms in (0..60_000).step_by(13) {
                animator.look_at(LookTarget::Knob, 30_000);
                let pose = animator.pose_at(ms);
                assert!(
                    pose.eyes_scale_y_q8 >= 48,
                    "{state:?} evening={evening} closed to {} at {ms}",
                    pose.eyes_scale_y_q8
                );
            }
        }
    }
}

#[test]
fn evening_blinks_are_slower_and_just_as_open() {
    let closed = |evening: bool| {
        let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
        animator.set_evening(evening);
        (0..6_000)
            .filter(|&ms| animator.pose_at(ms).eyes_scale_y_q8 < 256)
            .count()
    };
    assert!(closed(true) > closed(false) * 2);
}

#[test]
fn a_glance_moves_the_eyes_and_returns() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    let rest: Vec<_> = (0..3_000)
        .step_by(50)
        .map(|ms| animator.pose_at(ms + 10_000).eye_offset_q8)
        .collect();
    assert!(animator.glance(-768, 10_000));
    let mid = animator.pose_at(10_700).eye_offset_q8;
    assert!(mid.0 < rest[14].0 - 400, "{mid:?}");
    let after: Vec<_> = (0..3_000)
        .step_by(50)
        .map(|ms| animator.pose_at(ms + 10_000).eye_offset_q8)
        .collect();
    assert_eq!(after.last(), rest.last());
    // No second glance, and none over a busy, sleeping or booting buddy.
    assert!(!animator.glance(768, 10_500));
    for state in [
        CompanionState::Busy,
        CompanionState::Sleeping,
        CompanionState::Booting,
        CompanionState::Offline,
    ] {
        assert!(!MascotAnimator::new(state, 0).glance(768, 100), "{state:?}");
    }
}

#[test]
fn looking_at_a_control_starts_at_once_and_eases_out() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    let rest = animator.pose_at(10_000).eye_offset_q8;
    animator.look_at(LookTarget::Button(2), 10_000);
    assert_eq!(animator.pose_at(10_000).eye_offset_q8, rest);
    assert!(animator.pose_at(10_060).eye_offset_q8.0 > rest.0);
    assert!(animator.pose_at(10_400).eye_offset_q8.0 >= rest.0 + 600);
    // Held while touched, gone a second or so after the last touch.
    animator.look_at(LookTarget::Button(2), 10_600);
    assert!(animator.pose_at(11_200).eye_offset_q8.0 >= rest.0 + 600);
    assert_eq!(
        animator.pose_at(12_000).eye_offset_q8,
        MascotAnimator::new(CompanionState::Idle, 0)
            .pose_at(12_000)
            .eye_offset_q8
    );
    // A state change keeps the pose continuous and drops the look.
    animator.look_at(LookTarget::Knob, 20_000);
    let before = animator.pose_at(20_300);
    animator.set_state(CompanionState::Busy, 20_300);
    assert_eq!(animator.pose_at(20_300), before);
}

#[test]
fn looking_goes_both_ways_for_left_and_right_buttons() {
    let look = |control| {
        let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
        animator.look_at(control, 10_000);
        animator.pose_at(10_300).eye_offset_q8.0
            - MascotAnimator::new(CompanionState::Idle, 0)
                .pose_at(10_300)
                .eye_offset_q8
                .0
    };
    assert!(look(LookTarget::Button(0)) < 0);
    assert!(look(LookTarget::Button(2)) > 0);
    assert_eq!(look(LookTarget::Button(1)), 0);
}

#[test]
fn self_play_is_deterministic_and_follows_the_personality_cadence() {
    for (personality, range) in [
        (MascotPersonality::Calm, 18_000..=30_000),
        (MascotPersonality::Cozy, 8_000..=16_000),
        (MascotPersonality::Playful, 4_000..=8_000),
    ] {
        let mut first = SelfPlay::new(personality, true, 99, 5_000);
        let mut second = SelfPlay::new(personality, true, 99, 5_000);
        let mut now = 5_000;
        for _ in 0..50 {
            let due = first.next_due_ms();
            assert!(
                range.contains(&(due - now)),
                "{personality:?} {}",
                due - now
            );
            assert_eq!(
                first.poll(due, CompanionState::Idle),
                second.poll(due, CompanionState::Idle)
            );
            now = due;
        }
    }
    // A different seed, a different schedule: units differ.
    let a = SelfPlay::new(MascotPersonality::Cozy, true, 1, 0);
    let b = SelfPlay::new(MascotPersonality::Cozy, true, 2, 0);
    assert_ne!(a.next_due_ms(), b.next_due_ms());
}

/// Runs idle life over `seconds` of a state and returns (reactions started, glances started).
fn run_idle_life(life: &mut IdleLife, animator: &mut MascotAnimator, seconds: u32) -> (u32, u32) {
    let (mut reactions, mut glances) = (0, 0);
    let mut was_glancing = false;
    for ms in (0..seconds * 1_000).step_by(33) {
        life.poll(animator, ms);
        let pose = animator.pose_at(ms);
        let glancing = pose.eye_offset_q8.0.abs() >= 512;
        glances += u32::from(glancing && !was_glancing);
        was_glancing = glancing;
        reactions += u32::from(pose.body_offset_q8 != (0, 0));
    }
    (reactions, glances)
}

#[test]
fn idle_life_glances_and_reacts_on_its_own_with_the_personality_cadence() {
    let count = |personality| {
        let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
        let mut life = IdleLife::new(personality, true, 7, 0);
        run_idle_life(&mut life, &mut animator, 120)
    };
    let (calm, cozy, playful) = (
        count(MascotPersonality::Calm),
        count(MascotPersonality::Cozy),
        count(MascotPersonality::Playful),
    );
    assert!(calm.1 > 0 && calm.0 > 0, "{calm:?}");
    assert!(
        playful.1 > cozy.1 && cozy.1 > calm.1,
        "{calm:?} {cozy:?} {playful:?}"
    );
    assert!(
        playful.0 > cozy.0 && cozy.0 > calm.0,
        "{calm:?} {cozy:?} {playful:?}"
    );
}

#[test]
fn idle_life_is_identical_for_identical_seeds_and_differs_between_units() {
    let run = |seed| {
        let mut animator = MascotAnimator::new_seeded(CompanionState::Idle, 0, seed);
        let mut life = IdleLife::new(MascotPersonality::Cozy, true, seed, 0);
        (0..90_000)
            .step_by(33)
            .map(|ms| {
                life.poll(&mut animator, ms);
                animator.pose_at(ms)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(5), run(5));
    assert_ne!(run(5), run(6));
}

#[test]
fn reactions_off_stops_self_play_but_not_glances() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    let mut life = IdleLife::new(MascotPersonality::Playful, false, 3, 0);
    let (reactions, glances) = run_idle_life(&mut life, &mut animator, 120);
    assert_eq!(reactions, 0);
    assert!(glances > 0);
}

#[test]
fn idle_life_gives_way_to_busy_booting_offline_and_sleeping() {
    for state in [
        CompanionState::Busy,
        CompanionState::Booting,
        CompanionState::Offline,
        CompanionState::Sleeping,
    ] {
        let mut animator = MascotAnimator::new(state, 0);
        let mut life = IdleLife::new(MascotPersonality::Playful, true, 3, 0);
        let reference = MascotAnimator::new(state, 0);
        for ms in (0..60_000).step_by(33) {
            life.poll(&mut animator, ms);
            assert_eq!(
                animator.pose_at(ms),
                reference.pose_at(ms),
                "{state:?} at {ms}"
            );
        }
    }
}

#[test]
fn a_state_change_interrupts_self_play_and_a_stalled_loop_replays_nothing() {
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    let mut life = IdleLife::new(MascotPersonality::Playful, true, 3, 0);
    // The loop stalls for ten minutes, then resumes: one moment at most, not a backlog.
    life.poll(&mut animator, 600_000);
    animator.set_state(CompanionState::Busy, 600_100);
    assert_eq!(animator.pose_at(601_000).expression, MascotExpression::Busy);
    assert!(animator.pose_at(601_000).body_offset_q8.0.abs() <= 768);
}

#[test]
fn configuring_idle_life_only_restarts_the_cadence_on_a_change() {
    let mut life = IdleLife::new(MascotPersonality::Cozy, true, 3, 0);
    let before = life;
    life.configure(MascotPersonality::Cozy, true, 5_000);
    assert_eq!(life, before);
    life.configure(MascotPersonality::Calm, false, 5_000);
    assert_eq!(life.personality(), MascotPersonality::Calm);
    assert!(!life.self_play());
}

#[test]
fn late_evening_covers_21_to_5() {
    let at = |hour| kivori_model::desk::ClockTime {
        hour,
        minute: 0,
        second: 0,
    };
    assert!(!at(20).is_late_evening() && !at(12).is_late_evening() && !at(5).is_late_evening());
    assert!(
        at(21).is_late_evening()
            && at(23).is_late_evening()
            && at(0).is_late_evening()
            && at(4).is_late_evening()
    );
}
