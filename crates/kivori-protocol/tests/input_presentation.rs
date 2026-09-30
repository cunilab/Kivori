use heapless::Vec;
use kivori_model::input::Direction;
use kivori_model::presentation::{PrimaryState, ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_frame, decode_message, encode_message, ControlId, InputEvent, InputKind, Message,
    Presentation, MAX_FRAME, MAX_WIRE,
};

/// Mirrors the helper in `crates/kivori-protocol/tests/roundtrip.rs`.
/// `decode_message` takes the packet WITHOUT the trailing 0x00 delimiter.
fn roundtrip(msg: &Message) -> Message {
    let v = ProtocolVersion::new(1, 0);
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, v, 7, &mut wire).unwrap();
    let packet = &wire[..wire.len() - 1];
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    let (_, decoded) = decode_message(packet, &mut scratch, &[1]).unwrap();
    decoded
}

/// The first payload byte is the postcard variant index, i.e. the wire tag.
fn payload_tag(msg: &Message) -> u8 {
    let v = ProtocolVersion::new(1, 0);
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, v, 0, &mut wire).unwrap();
    let packet = &wire[..wire.len() - 1];
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    let (_, payload) = decode_frame(packet, &mut scratch).unwrap();
    payload[0]
}

#[test]
fn input_event_roundtrips() {
    let msg = Message::InputEvent(InputEvent {
        session: 0xDEAD_BEEF,
        gesture_id: 42,
        control: ControlId::Rotary,
        kind: InputKind::Detent(Direction::Ccw),
        device_ms: 123_456,
    });
    assert_eq!(roundtrip(&msg), msg);
}

#[test]
fn every_input_kind_roundtrips() {
    for kind in [
        InputKind::GestureStarted,
        InputKind::Detent(Direction::Cw),
        InputKind::Detent(Direction::Ccw),
        InputKind::GestureEnded,
    ] {
        let msg = Message::InputEvent(InputEvent {
            session: 1,
            gesture_id: 1,
            control: ControlId::Rotary,
            kind,
            device_ms: 0,
        });
        assert_eq!(roundtrip(&msg), msg);
    }
}

#[test]
fn presentation_roundtrips_with_and_without_a_value() {
    let with_value = Message::Presentation(Presentation {
        session: 9,
        revision: 12,
        primary: PrimaryState::Active,
        value: Some(ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: 64,
            confidence: ValueConfidence::Preview,
            at_boundary: false,
        }),
        transient_ms: 800,
    });
    assert_eq!(roundtrip(&with_value), with_value);

    let without_value = Message::Presentation(Presentation {
        session: 9,
        revision: 13,
        primary: PrimaryState::Idle,
        value: None,
        transient_ms: 0,
    });
    assert_eq!(roundtrip(&without_value), without_value);
}

#[test]
fn every_value_confidence_roundtrips() {
    for confidence in [
        ValueConfidence::Preview,
        ValueConfidence::Confirmed,
        ValueConfidence::Unverified,
    ] {
        let msg = Message::Presentation(Presentation {
            session: 1,
            revision: 1,
            primary: PrimaryState::Active,
            value: Some(ValueDisplay {
                kind: ValueKind::Volume,
                current_percent: 50,
                confidence,
                at_boundary: false,
            }),
            transient_ms: 800,
        });
        assert_eq!(roundtrip(&msg), msg);
    }
}

/// The postcard variant index IS the wire tag. Existing tags 0..=10 must not move.
#[test]
fn new_variants_are_appended_at_tags_13_and_14() {
    assert_eq!(
        payload_tag(&Message::InputEvent(InputEvent {
            session: 0,
            gesture_id: 0,
            control: ControlId::Rotary,
            kind: InputKind::GestureEnded,
            device_ms: 0,
        })),
        13,
        "InputEvent must be wire tag 13"
    );

    assert_eq!(
        payload_tag(&Message::Presentation(Presentation {
            session: 0,
            revision: 0,
            primary: PrimaryState::Idle,
            value: None,
            transient_ms: 0,
        })),
        14,
        "Presentation must be wire tag 14"
    );
}

#[test]
fn the_two_new_capability_bits_are_distinct_and_stable() {
    assert_eq!(Capabilities::PHYSICAL_INPUT_V1.bits(), 1 << 1);
    assert_eq!(Capabilities::PRESENTATION_V1.bits(), 1 << 2);
}

#[test]
fn a_peer_without_the_bit_leaves_the_capability_unnegotiated() {
    let desktop = Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::PRESENTATION_V1);
    let old_device = Capabilities::NONE;
    let negotiated = desktop.intersection(old_device);

    assert!(!negotiated.contains(Capabilities::PHYSICAL_INPUT_V1));
    assert!(!negotiated.contains(Capabilities::PRESENTATION_V1));
}

#[test]
fn a_peer_with_only_one_bit_negotiates_only_that_bit() {
    let desktop = Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::PRESENTATION_V1);
    let device = Capabilities::PHYSICAL_INPUT_V1;
    let negotiated = desktop.intersection(device);

    assert!(negotiated.contains(Capabilities::PHYSICAL_INPUT_V1));
    assert!(!negotiated.contains(Capabilities::PRESENTATION_V1));
}
