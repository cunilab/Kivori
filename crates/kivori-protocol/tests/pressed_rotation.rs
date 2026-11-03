use heapless::Vec;
use kivori_model::{input::Direction, Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, negotiate, ControlId, InputEvent, InputKind, Message,
    MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

#[test]
fn control_ids_keep_their_wire_ordinals_and_pressed_rotation_roundtrips() {
    let mut buffer = [0u8; 16];
    assert_eq!(
        postcard::to_slice(&ControlId::Rotary, &mut buffer).unwrap(),
        &[0]
    );
    assert_eq!(
        postcard::to_slice(&ControlId::Button, &mut buffer).unwrap(),
        &[1]
    );
    assert_eq!(
        postcard::to_slice(&ControlId::ContextButton(1), &mut buffer).unwrap(),
        &[2, 1]
    );
    assert_eq!(
        postcard::to_slice(&ControlId::PressedRotary, &mut buffer).unwrap(),
        &[3]
    );

    let message = Message::InputEvent(InputEvent {
        session: 42,
        gesture_id: 7,
        control: ControlId::PressedRotary,
        kind: InputKind::Detent(Direction::Ccw),
        device_ms: 100,
    });
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(
        &message,
        ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
        5,
        &mut wire,
    )
    .unwrap();
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    let (_, decoded) =
        decode_message(&wire[..wire.len() - 1], &mut scratch, &[PROTOCOL_MAJOR]).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn held_rotation_requires_both_peers() {
    let caps = Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::HELD_ROTATION_V1);
    let (_, negotiated) = negotiate(PROTOCOL_MINOR, PROTOCOL_MINOR, caps, caps);
    assert!(negotiated.contains(Capabilities::HELD_ROTATION_V1));
    let (_, negotiated) = negotiate(PROTOCOL_MINOR, 2, caps, Capabilities::PHYSICAL_INPUT_V1);
    assert!(!negotiated.contains(Capabilities::HELD_ROTATION_V1));
}
