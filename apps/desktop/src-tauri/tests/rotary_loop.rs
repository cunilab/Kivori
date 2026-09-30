use kivori_desktop::input::{InputIngress, LogicalInput, RejectReason};
use kivori_model::input::Direction;
use kivori_protocol::message::{ControlId, InputEvent, InputKind};

fn ev(session: u32, gesture_id: u16, kind: InputKind) -> InputEvent {
    InputEvent {
        session,
        gesture_id,
        control: ControlId::Rotary,
        kind,
        device_ms: 0,
    }
}

#[test]
fn a_started_gesture_admits_its_detents() {
    let mut ingress = InputIngress::new();
    ingress.begin_session(0xAAAA);

    assert_eq!(
        ingress.accept(&ev(0xAAAA, 1, InputKind::GestureStarted)),
        Ok(Some(LogicalInput::GestureStarted { gesture_id: 1 }))
    );
    assert_eq!(
        ingress.accept(&ev(0xAAAA, 1, InputKind::Detent(Direction::Cw))),
        Ok(Some(LogicalInput::Detent {
            gesture_id: 1,
            direction: Direction::Cw
        }))
    );
    assert_eq!(
        ingress.accept(&ev(0xAAAA, 1, InputKind::GestureEnded)),
        Ok(Some(LogicalInput::GestureEnded { gesture_id: 1 }))
    );
}

#[test]
fn a_detent_for_a_gesture_that_never_started_is_rejected() {
    let mut ingress = InputIngress::new();
    ingress.begin_session(0xAAAA);

    assert_eq!(
        ingress.accept(&ev(0xAAAA, 7, InputKind::Detent(Direction::Cw))),
        Err(RejectReason::UnknownGesture)
    );
}

/// THE ADVERSARIAL CASE that removed the epoch-free design.
/// A COMPLETE stale pair — GestureStarted AND Detent — replayed after a reconnect.
#[test]
fn a_complete_stale_gesture_pair_after_reconnect_executes_nothing() {
    let mut ingress = InputIngress::new();

    ingress.begin_session(0x1111);
    ingress
        .accept(&ev(0x1111, 1, InputKind::GestureStarted))
        .unwrap();

    // Link drops, a new session is established with a different nonce.
    ingress.end_session();
    ingress.begin_session(0x2222);

    // Both halves of the old gesture arrive, in order, from the stale buffer.
    assert_eq!(
        ingress.accept(&ev(0x1111, 1, InputKind::GestureStarted)),
        Err(RejectReason::StaleSession)
    );
    assert_eq!(
        ingress.accept(&ev(0x1111, 1, InputKind::Detent(Direction::Cw))),
        Err(RejectReason::StaleSession)
    );
}

#[test]
fn gesture_ids_reused_by_a_new_session_do_not_inherit_old_state() {
    let mut ingress = InputIngress::new();

    ingress.begin_session(0x1111);
    ingress
        .accept(&ev(0x1111, 1, InputKind::GestureStarted))
        .unwrap();

    ingress.end_session();
    ingress.begin_session(0x2222);

    // Same gesture id, new session: the open-gesture set was cleared, so a bare
    // detent has no start to attach to.
    assert_eq!(
        ingress.accept(&ev(0x2222, 1, InputKind::Detent(Direction::Cw))),
        Err(RejectReason::UnknownGesture)
    );
}

#[test]
fn input_outside_a_session_is_rejected() {
    let mut ingress = InputIngress::new();
    assert_eq!(
        ingress.accept(&ev(0x1111, 1, InputKind::GestureStarted)),
        Err(RejectReason::NoSession)
    );
}

#[test]
fn a_gesture_cannot_continue_after_it_ended() {
    let mut ingress = InputIngress::new();
    ingress.begin_session(0xAAAA);
    ingress
        .accept(&ev(0xAAAA, 1, InputKind::GestureStarted))
        .unwrap();
    ingress
        .accept(&ev(0xAAAA, 1, InputKind::GestureEnded))
        .unwrap();

    assert_eq!(
        ingress.accept(&ev(0xAAAA, 1, InputKind::Detent(Direction::Cw))),
        Err(RejectReason::UnknownGesture)
    );
}

use kivori_desktop::action::volume::{apply_step, BASE_STEP_PERCENT};
use kivori_desktop::action::{resolve_binding, ActionId, Outcome};
use kivori_desktop::platform::{ActionAvailability, FakeVolumeBackend};

#[test]
fn the_global_rotary_binding_resolves_to_master_volume() {
    assert_eq!(
        resolve_binding(ControlId::Rotary),
        Some(ActionId::MasterVolume)
    );
}

#[test]
fn one_detent_moves_exactly_the_base_step() {
    assert_eq!(
        apply_step(50, Direction::Cw),
        (50 + BASE_STEP_PERCENT, false)
    );
    assert_eq!(
        apply_step(50, Direction::Ccw),
        (50 - BASE_STEP_PERCENT, false)
    );
}

#[test]
fn values_clamp_at_both_bounds_and_flag_the_boundary() {
    assert_eq!(apply_step(100, Direction::Cw), (100, true));
    assert_eq!(apply_step(0, Direction::Ccw), (0, true));
    // Approaching the bound from inside one step lands exactly on it, not past it.
    assert_eq!(apply_step(99, Direction::Cw), (100, false));
    assert_eq!(apply_step(1, Direction::Ccw), (0, false));
}

#[test]
fn the_first_reverse_detent_leaves_the_boundary_immediately() {
    let (at_max, boundary) = apply_step(100, Direction::Cw);
    assert!(boundary);
    assert_eq!(
        apply_step(at_max, Direction::Ccw),
        (100 - BASE_STEP_PERCENT, false)
    );
}

#[test]
fn a_successful_set_with_readback_is_state_confirmed() {
    let backend = FakeVolumeBackend::new(40);
    let outcome = kivori_desktop::action::execute_volume(&backend, 60);
    assert_eq!(outcome, Outcome::StateConfirmed { volume_percent: 60 });
}

#[test]
fn state_confirmed_reports_the_observed_value_not_the_requested_one() {
    let backend = FakeVolumeBackend::quantised(40, 5);
    let outcome = kivori_desktop::action::execute_volume(&backend, 62);
    assert_eq!(
        outcome,
        Outcome::StateConfirmed { volume_percent: 60 },
        "confirmation must carry OS truth, never the request"
    );
}

#[test]
fn a_write_without_readback_is_unverified_never_confirmed() {
    let backend = FakeVolumeBackend::unreadable_after_write(30);
    assert_eq!(
        kivori_desktop::action::execute_volume(&backend, 40),
        Outcome::TriggeredUnverified
    );
}

#[test]
fn an_unavailable_backend_fails_rather_than_silently_succeeding() {
    let backend = FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
        reason: "no default render endpoint".to_string(),
    });
    assert!(matches!(
        kivori_desktop::action::execute_volume(&backend, 40),
        Outcome::Failed { .. }
    ));
}

#[test]
fn an_unimplemented_backend_does_not_attempt_execution() {
    let backend = kivori_desktop::platform::unimplemented::UnimplementedVolumeBackend::new("macos");
    assert!(matches!(
        kivori_desktop::action::execute_volume(&backend, 40),
        Outcome::Failed { .. }
    ));
}

use kivori_desktop::action::gesture_value::{GestureValue, ValueUpdate};
use kivori_model::presentation::ValueConfidence;

fn preview(percent: u8) -> Option<ValueUpdate> {
    Some(ValueUpdate {
        percent,
        confidence: ValueConfidence::Preview,
        at_boundary: false,
        failed: false,
    })
}

fn confirmed(percent: u8) -> Option<ValueUpdate> {
    Some(ValueUpdate {
        percent,
        confidence: ValueConfidence::Confirmed,
        at_boundary: false,
        failed: false,
    })
}

#[test]
fn detents_during_a_gesture_are_preview_never_confirmed() {
    let backend = FakeVolumeBackend::new(50);
    let mut gv = GestureValue::new();

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    assert_eq!(
        gv.on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw
            },
            &backend
        ),
        preview(52)
    );
    assert_eq!(
        gv.on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw
            },
            &backend
        ),
        preview(54)
    );
}

#[test]
fn gesture_end_reconciles_to_the_value_the_backend_reports() {
    // The backend quantises, so the confirmed value differs from the preview.
    let backend = FakeVolumeBackend::quantised(50, 5);
    let mut gv = GestureValue::new();

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    assert_eq!(
        gv.on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw
            },
            &backend
        ),
        preview(52)
    );
    assert_eq!(
        gv.on_input(LogicalInput::GestureEnded { gesture_id: 1 }, &backend),
        confirmed(50),
        "confirmed state wins over the local preview"
    );
}

#[test]
fn an_external_change_during_a_gesture_does_not_overwrite_the_preview() {
    let backend = FakeVolumeBackend::new(50);
    let mut gv = GestureValue::new();

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    gv.on_input(
        LogicalInput::Detent {
            gesture_id: 1,
            direction: Direction::Cw,
        },
        &backend,
    );

    // Somebody moves the Windows flyout mid-gesture.
    assert_eq!(
        gv.on_external_change(10),
        None,
        "external updates must not visually fight an active gesture"
    );

    // It is applied once the gesture ends.
    backend.external_change(10);
    assert_eq!(
        gv.on_input(LogicalInput::GestureEnded { gesture_id: 1 }, &backend),
        confirmed(10)
    );
}

#[test]
fn a_gesture_dropped_by_a_session_end_releases_the_preview() {
    let backend = FakeVolumeBackend::new(50);
    let mut gv = GestureValue::new();
    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);

    // The link drops mid-turn: the device never sends `GestureEnded` for gesture 1.
    gv.end_session();
    assert_eq!(gv.on_external_change(20), confirmed(20));
}

#[test]
fn an_external_change_outside_a_gesture_is_confirmed_immediately() {
    let mut gv = GestureValue::new();
    assert_eq!(gv.on_external_change(77), confirmed(77));
}

#[test]
fn a_write_without_readback_reconciles_as_unverified() {
    let backend = FakeVolumeBackend::unreadable_after_write(30);
    let mut gv = GestureValue::new();

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    let update = gv
        .on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw,
            },
            &backend,
        )
        .expect("update");
    assert_eq!(update.confidence, ValueConfidence::Unverified);
    assert_ne!(
        update.confidence,
        ValueConfidence::Confirmed,
        "an unobservable write must never read as confirmed"
    );
}

#[test]
fn confidence_never_reaches_confirmed_without_a_backend_read() {
    let backend = FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
        reason: "no default render endpoint".to_string(),
    });
    let mut gv = GestureValue::new();
    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    let update = gv.on_input(
        LogicalInput::Detent {
            gesture_id: 1,
            direction: Direction::Cw,
        },
        &backend,
    );
    if let Some(u) = update {
        assert_ne!(u.confidence, ValueConfidence::Confirmed);
    }
}

#[test]
fn boundary_is_flagged_only_while_pressure_continues_into_the_bound() {
    let backend = FakeVolumeBackend::new(100);
    let mut gv = GestureValue::new();
    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);

    let update = gv
        .on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw,
            },
            &backend,
        )
        .expect("update");
    assert!(update.at_boundary);
    assert_eq!(update.percent, 100);

    let reversed = gv
        .on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Ccw,
            },
            &backend,
        )
        .expect("update");
    assert!(!reversed.at_boundary);
    assert_eq!(reversed.percent, 98);
}

#[test]
fn an_endpoint_rebind_mid_gesture_discards_the_gesture_rather_than_retargeting() {
    let backend = FakeVolumeBackend::new(50);
    let mut gv = GestureValue::new();
    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    gv.on_input(
        LogicalInput::Detent {
            gesture_id: 1,
            direction: Direction::Cw,
        },
        &backend,
    );

    // The user switched output device. The new endpoint is at 20.
    assert_eq!(gv.on_endpoint_rebind(20), confirmed(20));

    // Remaining detents of the abandoned gesture must not drive the NEW endpoint.
    assert_eq!(
        gv.on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw
            },
            &backend
        ),
        None
    );
}

use kivori_desktop::presentation::{PresentationResolver, ProductSnapshot};
use kivori_model::presentation::{PrimaryState, ValueKind};

#[test]
fn revision_increases_strictly_within_a_session() {
    let mut r = PresentationResolver::new(0xAAAA);
    let a = r.resolve(&ProductSnapshot::idle());
    let b = r.resolve(&ProductSnapshot::idle());
    assert!(b.revision > a.revision);
    assert_eq!(a.session, 0xAAAA);
}

#[test]
fn revision_resets_when_a_new_session_begins() {
    let mut r = PresentationResolver::new(0xAAAA);
    for _ in 0..743 {
        r.resolve(&ProductSnapshot::idle());
    }
    let high = r.resolve(&ProductSnapshot::idle()).revision;
    assert!(high >= 743);

    r.begin_session(0xBBBB);
    let fresh = r.resolve(&ProductSnapshot::idle());
    assert_eq!(fresh.session, 0xBBBB);
    assert_eq!(
        fresh.revision, 1,
        "a restarted desktop starts at revision 1"
    );
}

#[test]
fn a_value_update_becomes_a_transient_overlay_over_the_underlying_state() {
    let mut r = PresentationResolver::new(1);
    let p = r.resolve(&ProductSnapshot::with_value(ValueUpdate {
        percent: 60,
        confidence: ValueConfidence::Preview,
        at_boundary: false,
        failed: false,
    }));

    let value = p.value.expect("overlay present");
    assert_eq!(value.kind, ValueKind::Volume);
    assert_eq!(value.current_percent, 60);
    assert_eq!(value.confidence, ValueConfidence::Preview);
    assert!(p.transient_ms > 0, "the overlay must expire");
    assert_eq!(
        p.primary,
        PrimaryState::Idle,
        "the underlying truth the device restores to"
    );
}

#[test]
fn a_failed_outcome_resolves_to_error_without_a_value_overlay() {
    let mut r = PresentationResolver::new(1);
    let p = r.resolve(&ProductSnapshot::failed());
    assert_eq!(p.primary, PrimaryState::Error);
    assert_eq!(p.value, None);
}

#[test]
fn a_detent_against_an_unimplemented_backend_emits_no_value_overlay() {
    // macOS/Linux today: the backend is honest that it is not implemented, so `execute_volume`
    // never even attempts a write. Turning the knob must therefore paint nothing at all — a
    // dashed bar marching upward while nothing happens is state the desktop cannot observe
    // (invariant 2) presented instead of unavailability (invariant 19).
    let backend = kivori_desktop::platform::unimplemented::UnimplementedVolumeBackend::new("macos");
    let mut gv = GestureValue::new();
    let mut r = PresentationResolver::new(1);

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);

    let mut percents = Vec::new();
    for _ in 0..3 {
        let update = gv
            .on_input(
                LogicalInput::Detent {
                    gesture_id: 1,
                    direction: Direction::Cw,
                },
                &backend,
            )
            .expect("a failure must be reported, not swallowed");
        percents.push(update.percent);

        let p = r.resolve(&ProductSnapshot::with_value(update));
        assert_eq!(
            p.value, None,
            "no write was attempted, so no volume bar may be painted"
        );
        assert_eq!(p.transient_ms, 0, "there is no overlay to expire");
    }
    assert_eq!(
        percents,
        vec![0, 0, 0],
        "the local target must not advance while nothing whatsoever has happened"
    );
}

#[test]
fn a_known_failure_reaches_error_through_the_production_path() {
    // Invariant 4 is MUST-level: known success, known failure and unknown outcome stay distinct.
    // This walks the same chain `runtime::device_task` walks — GestureValue -> ProductSnapshot ->
    // PresentationResolver — so `PrimaryState::Error` has a real production producer, not only a
    // resolver unit test that production can never reach.
    let failing = FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
        reason: "no default render endpoint".to_string(),
    });
    let mut gv = GestureValue::new();
    let mut r = PresentationResolver::new(1);

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &failing);
    let update = gv
        .on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw,
            },
            &failing,
        )
        .expect("a known failure must be reported");
    let p = r.resolve(&ProductSnapshot::with_value(update));
    assert_eq!(
        p.primary,
        PrimaryState::Error,
        "known failure must not collapse into unknown outcome"
    );
    assert_eq!(p.value, None);

    // ...and an unobservable write stays distinct from that known failure.
    let unverified = FakeVolumeBackend::unreadable_after_write(30);
    let mut gv2 = GestureValue::new();
    gv2.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &unverified);
    let update = gv2
        .on_input(
            LogicalInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw,
            },
            &unverified,
        )
        .expect("update");
    let p = r.resolve(&ProductSnapshot::with_value(update));
    assert_eq!(
        p.primary,
        PrimaryState::Idle,
        "an unobservable write is not a known failure"
    );
    assert_eq!(
        p.value.expect("overlay present").confidence,
        ValueConfidence::Unverified
    );
}
