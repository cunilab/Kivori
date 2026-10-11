//! M1 wire additions: push-switch input, desk `Status`, action `Feedback`.

use heapless::Vec;
use kivori_model::desk::{
    ActionKind, ClockTime, ControlLabels, DeskStatus, DisplayMode, FeedbackKind, MediaStatus,
    MediaText,
};
use kivori_model::internals::{ChipModel, DeviceInternals, ResetReason};
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_frame, decode_message, encode_message, ControlId, ControlLabelsUpdate, Feedback,
    InputEvent, InputKind, InternalsReport, InternalsRequest, Message, Status, MAX_FRAME,
    MAX_PAYLOAD, MAX_WIRE,
};

fn encode(msg: &Message) -> Vec<u8, MAX_WIRE> {
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, ProtocolVersion::new(1, 3), 3, &mut wire).unwrap();
    wire
}

fn roundtrip(msg: &Message) -> Message {
    let wire = encode(msg);
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    decode_message(&wire[..wire.len() - 1], &mut scratch, &[1])
        .unwrap()
        .1
}

/// Bytes of the postcard payload: the first is the `Message` tag.
fn payload(msg: &Message) -> std::vec::Vec<u8> {
    let wire = encode(msg);
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    decode_frame(&wire[..wire.len() - 1], &mut scratch)
        .unwrap()
        .1
        .to_vec()
}

fn full_status() -> DeskStatus {
    DeskStatus {
        mode: DisplayMode::System,
        clock: Some(ClockTime {
            hour: 23,
            minute: 59,
            second: 58,
        }),
        volume_percent: Some(100),
        muted: Some(true),
        media: Some(MediaStatus::Paused),
        cpu_percent: Some(97),
        ram_percent: Some(0),
        high_load: true,
    }
}

#[test]
fn button_input_events_roundtrip() {
    for kind in [InputKind::Press, InputKind::Hold] {
        let msg = Message::InputEvent(InputEvent {
            session: 0xFEED_0001,
            gesture_id: 9,
            control: ControlId::Button,
            kind,
            device_ms: 77,
        });
        assert_eq!(roundtrip(&msg), msg);
    }
}

#[test]
fn status_roundtrips_known_and_unknown() {
    for status in [DeskStatus::UNKNOWN, full_status()] {
        for mode in DisplayMode::ALL {
            let msg = Message::Status(Status {
                session: 4,
                status: DeskStatus { mode, ..status },
            });
            assert_eq!(roundtrip(&msg), msg);
        }
    }
}

#[test]
fn every_feedback_roundtrips() {
    for action in [
        ActionKind::Volume,
        ActionKind::PlayPause,
        ActionKind::Mute,
        ActionKind::Shortcut,
        ActionKind::Launch,
    ] {
        for kind in [
            FeedbackKind::Processing,
            FeedbackKind::StateConfirmed,
            FeedbackKind::ExecutionConfirmed,
            FeedbackKind::Unverified,
            FeedbackKind::Error,
            FeedbackKind::Triggered,
        ] {
            let msg = Message::Feedback(Feedback {
                session: 5,
                action,
                kind,
            });
            assert_eq!(roundtrip(&msg), msg);
        }
    }
}

/// Tags and nested variant indices are the wire contract: appended, never moved.
#[test]
fn m1_variants_are_appended_after_every_existing_tag() {
    let status = Message::Status(Status {
        session: 0,
        status: DeskStatus::UNKNOWN,
    });
    assert_eq!(payload(&status)[0], 15, "Status is tag 15");
    let feedback = Message::Feedback(Feedback {
        session: 0,
        action: ActionKind::Volume,
        kind: FeedbackKind::Processing,
    });
    assert_eq!(payload(&feedback)[0], 16, "Feedback is tag 16");

    // InputEvent: tag, session varint (0), gesture_id varint (0), control, kind.
    let press = Message::InputEvent(InputEvent {
        session: 0,
        gesture_id: 0,
        control: ControlId::Button,
        kind: InputKind::Press,
        device_ms: 0,
    });
    assert_eq!(
        &payload(&press)[..5],
        &[13, 0, 0, 1, 3],
        "Button = 1, Press = 3"
    );
    let hold = Message::InputEvent(InputEvent {
        kind: InputKind::Hold,
        ..match press {
            Message::InputEvent(event) => event,
            _ => unreachable!(),
        }
    });
    assert_eq!(payload(&hold)[4], 4, "Hold = 4");
}

#[test]
fn the_m1_capability_bits_are_stable_and_distinct() {
    assert_eq!(Capabilities::BUTTON_INPUT_V1.bits(), 1 << 3);
    assert_eq!(Capabilities::DESK_STATUS_V1.bits(), 1 << 4);
    assert_eq!(Capabilities::ACTION_FEEDBACK_V1.bits(), 1 << 5);
}

#[test]
fn the_largest_status_fits_one_frame() {
    assert!(
        payload(&Message::Status(Status {
            session: u32::MAX,
            status: full_status(),
        }))
        .len()
            < 64
    );
}

#[test]
fn control_labels_are_tag_18_roundtrip_and_fit_one_frame() {
    let full = MediaText::from_text(&"x".repeat(40));
    let msg = Message::ControlLabels(ControlLabelsUpdate {
        session: u32::MAX,
        labels: ControlLabels {
            rotate: full,
            press: MediaText::from_text("Play/Pause"),
            hold: MediaText::default(),
            buttons: [full, full, full],
            profile: full,
            pinned: true,
            mood: kivori_model::desk::ContextMood::Meeting,
        },
    });
    assert_eq!(roundtrip(&msg), msg);
    let bytes = payload(&msg);
    assert_eq!(bytes[0], 18, "ControlLabels is tag 18");
    assert!(bytes.len() <= kivori_protocol::MAX_PAYLOAD);
    assert_eq!(Capabilities::CONTROL_LABELS_V1.bits(), 1 << 8);
}

#[test]
fn context_buttons_are_control_variant_2_with_their_index() {
    let press = Message::InputEvent(InputEvent {
        session: 0,
        gesture_id: 0,
        control: ControlId::ContextButton(2),
        kind: InputKind::Hold,
        device_ms: 0,
    });
    assert_eq!(roundtrip(&press), press);
    assert_eq!(
        &payload(&press)[..6],
        &[13, 0, 0, 2, 2, 4],
        "ContextButton = 2, index, Hold"
    );
    assert_eq!(Capabilities::CONTEXT_BUTTONS_V1.bits(), 1 << 9);
    assert_eq!(Capabilities::HOST_TAKEOVERS_V1.bits(), 1 << 10);
    assert_eq!(Capabilities::QUIET_ACK_V1.bits(), 1 << 11);
    assert_eq!(Capabilities::DEVICE_INTERNALS_V1.bits(), 1 << 12);
}

/// Append-only: `Triggered` follows the original five kinds.
#[test]
fn triggered_feedback_is_appended_as_kind_5() {
    for (kind, wire_value) in [(FeedbackKind::Error, 4u8), (FeedbackKind::Triggered, 5)] {
        let msg = Message::Feedback(Feedback {
            session: 0,
            action: ActionKind::Volume,
            kind,
        });
        assert_eq!(payload(&msg).last().copied(), Some(wire_value), "{kind:?}");
    }
}

fn full_internals() -> DeviceInternals {
    DeviceInternals {
        app_image_bytes: Some(u32::MAX),
        app_partition_bytes: Some(u32::MAX),
        asset_blob_bytes: Some(u32::MAX),
        asset_blob_cap_bytes: Some(u32::MAX),
        flash_total_bytes: Some(u32::MAX),
        ram_total_bytes: Some(u32::MAX),
        ram_static_bytes: Some(u32::MAX),
        ram_free_bytes: Some(u32::MAX),
        stack_free_min_bytes: Some(u32::MAX),
        busy_percent: Some(100),
        compose_ms: Some(u16::MAX),
        flush_ms: Some(u16::MAX),
        fps: Some(u8::MAX),
        chip_model: Some(ChipModel::Esp32C3),
        chip_revision: Some((u8::MAX, u8::MAX)),
        temperature_decicelsius: Some(i16::MIN),
        uptime_ms: Some(u32::MAX),
        reset_reason: Some(ResetReason::Other),
        edge_queue_high_water: Some(u16::MAX),
        dropped_edges: Some(u32::MAX),
        invalid_transitions: Some(u32::MAX),
        frames_in: Some(u32::MAX),
        frames_out: Some(u32::MAX),
        frames_rejected: Some(u32::MAX),
    }
}

/// Append-only: the request and the snapshot follow `ControlLabels` (tag 18) as tags 19 and 20.
#[test]
fn internals_messages_are_appended_as_tags_19_and_20() {
    let request = Message::InternalsRequest(InternalsRequest { session: 7 });
    assert_eq!(payload(&request)[0], 19);
    assert_eq!(roundtrip(&request), request);

    let report = Message::Internals(InternalsReport {
        session: 7,
        internals: full_internals(),
    });
    assert_eq!(payload(&report)[0], 20);
    assert_eq!(roundtrip(&report), report);
}

/// Every field optional: a device that measures nothing still sends a valid, small snapshot, and
/// `None` stays `None` (never 0) across the wire.
#[test]
fn an_empty_internals_snapshot_roundtrips_as_all_unavailable() {
    let report = Message::Internals(InternalsReport {
        session: 1,
        internals: DeviceInternals::default(),
    });
    assert_eq!(roundtrip(&report), report);
    // Tag, session varint, then one `None` byte per field.
    assert!(payload(&report).len() <= 2 + 5 + 24);
}

/// The fullest possible snapshot must stay far inside one frame.
#[test]
fn the_fullest_internals_snapshot_fits_one_frame() {
    let report = Message::Internals(InternalsReport {
        session: u32::MAX,
        internals: full_internals(),
    });
    assert!(payload(&report).len() < MAX_PAYLOAD / 2);
}
