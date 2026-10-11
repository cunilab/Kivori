use kivori_desktop::render::animation::{
    AnimationEvent, AnimationTimeline, IdleLifeSpec, MascotActionEvent,
};
use kivori_model::{CompanionState, MascotAction, MascotAnimator, MascotPersonality};

#[test]
fn seeking_replays_the_same_interrupted_pose_as_live_device_events() {
    let timeline = AnimationTimeline {
        initial_state: "idle".into(),
        events: vec![
            AnimationEvent {
                at_ms: 100,
                state: "happy".into(),
            },
            AnimationEvent {
                at_ms: 250,
                state: "sleeping".into(),
            },
        ],
        action_events: vec![],
        idle_life: None,
    };
    let mut live = MascotAnimator::new(CompanionState::Idle, 0);
    live.set_state(CompanionState::Happy, 100);
    assert_eq!(timeline.resolve(175).unwrap().1, live.pose_at(175));
    live.set_state(CompanionState::Sleeping, 250);
    for ms in [250, 251, 400, 849, 850, 4_001] {
        assert_eq!(timeline.resolve(ms).unwrap().1, live.pose_at(ms));
    }
    assert_eq!(timeline.resolve(175).unwrap().0, CompanionState::Happy);
}

#[test]
fn rejects_invalid_event_order_and_unknown_states() {
    let mut timeline = AnimationTimeline {
        initial_state: "idle".into(),
        events: vec![
            AnimationEvent {
                at_ms: 250,
                state: "happy".into(),
            },
            AnimationEvent {
                at_ms: 100,
                state: "busy".into(),
            },
        ],
        action_events: vec![],
        idle_life: None,
    };
    assert!(timeline.validate().is_err());
    timeline.events.clear();
    timeline.initial_state = "unknown".into();
    assert!(timeline.validate().is_err());
    timeline.initial_state = "idle".into();
    timeline.events = vec![
        AnimationEvent {
            at_ms: 0,
            state: "happy".into()
        };
        257
    ];
    assert!(timeline.validate().is_err());
}

#[test]
fn seeking_replays_seeded_social_action_cues() {
    let timeline = AnimationTimeline {
        initial_state: "idle".into(),
        events: vec![],
        action_events: vec![MascotActionEvent {
            at_ms: 100,
            action: "tickle".into(),
            personality: "playful".into(),
            seed: 7,
        }],
        idle_life: None,
    };
    let mut live = MascotAnimator::new(CompanionState::Idle, 0);
    live.trigger_action(MascotAction::Tickle, MascotPersonality::Playful, 7, 100);

    assert_eq!(timeline.resolve(700).unwrap().1, live.pose_at(700));
}

/// Device Studio runs the very idle life the firmware runs: polled once per 33 ms frame with the
/// fixed seed, so a preview of any moment equals a live animator driven the same way.
#[test]
fn idle_life_preview_matches_the_live_loop_including_events() {
    use kivori_model::mascot::DEFAULT_IDLE_SEED;
    use kivori_model::IdleLife;

    let timeline = AnimationTimeline {
        initial_state: "idle".into(),
        events: vec![AnimationEvent {
            at_ms: 40_000,
            state: "busy".into(),
        }],
        action_events: vec![],
        idle_life: Some(IdleLifeSpec {
            personality: "playful".into(),
            self_play: true,
        }),
    };
    let mut live = MascotAnimator::new_seeded(CompanionState::Idle, 0, DEFAULT_IDLE_SEED);
    let mut life = IdleLife::new(MascotPersonality::Playful, true, DEFAULT_IDLE_SEED, 0);
    let mut moved = false;
    for ms in (0..=60_000).step_by(33) {
        if ms >= 40_000 && live.target() != CompanionState::Busy {
            live.set_state(CompanionState::Busy, 40_000);
        }
        life.poll(&mut live, ms);
        if ms % 990 == 0 {
            let (state, pose) = timeline.resolve(ms).unwrap();
            assert_eq!(pose, live.pose_at(ms), "at {ms}");
            assert_eq!(state, live.target());
            moved |= pose.eye_offset_q8.0.abs() >= 512 || pose.body_offset_q8 != (0, 0);
        }
    }
    assert!(
        moved,
        "playful idle life never glanced or reacted in a minute"
    );
}
