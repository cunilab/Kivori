//! M1 wire additions: push-switch input, desk `Status`, action `Feedback`.

use heapless::Vec;
use kivori_model::desk::{
    ActionKind, ClockTime, ControlLabels, DeskStatus, DisplayMode, FeedbackKind, MediaStatus,
    MediaText,
};
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_frame, decode_message, encode_message, ControlId, ControlLabelsUpdate, Feedback,
    InputEvent, InputKind, Message, Status, MAX_FRAME, MAX_WIRE,
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
        },
    });
    assert_eq!(roundtrip(&msg), msg);
    let bytes = payload(&msg);
    assert_eq!(bytes[0], 18, "ControlLabels is tag 18");
    assert!(bytes.len() <= kivori_protocol::MAX_PAYLOAD);
    assert_eq!(Capabilities::CONTROL_LABELS_V1.bits(), 1 << 8);
}
