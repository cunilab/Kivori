//! T028 — round-trip encode/decode of every `Message` variant.

use heapless::Vec;
use kivori_model::{
    Capabilities, CompanionState, MascotAction, MascotPersonality, ProtocolVersion, SendableState,
};
use kivori_protocol::{
    decode_message, encode_message, Bye, ByeReason, Diagnostic, ErrorCategory, ErrorReport,
    FirmwareVersion, Health, Hello, HelloAck, MascotActionApplied, Message, Ping, PlayMascotAction,
    Pong, Ready, SetState, StateReport, MAX_FRAME, MAX_WIRE,
};

fn roundtrip(msg: &Message) -> Message {
    let v = ProtocolVersion::new(1, 0);
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, v, 7, &mut wire).unwrap();
    assert_eq!(
        *wire.last().unwrap(),
        0,
        "wire packet ends with the 0x00 delimiter"
    );
    let packet = &wire[..wire.len() - 1];
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    let (header, decoded) = decode_message(packet, &mut scratch, &[1]).unwrap();
    assert_eq!(header.seq, 7);
    assert_eq!(header.version, v);
    decoded
}

fn all_messages() -> [Message; 13] {
    let fw = FirmwareVersion {
        major: 1,
        minor: 2,
        patch: 3,
    };
    [
        Message::Hello(Hello {
            desktop_version: fw,
            desktop_caps: Capabilities::from_bits(0b101),
            nonce: 0xDEAD_BEEF,
        }),
        Message::HelloAck(HelloAck {
            device_caps: Capabilities::from_bits(0b011),
            device_id: [7u8; 16],
            firmware_version: fw,
            nonce_echo: 0xDEAD_BEEF,
        }),
        Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::from_bits(0b001),
        }),
        Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        Message::SetState(SetState {
            desired: SendableState::Busy,
            at_ms: Some(1234),
        }),
        Message::StateReport(StateReport {
            reported: CompanionState::Offline,
            elapsed_ms: 999,
        }),
        Message::Ping(Ping { t_ms: 42 }),
        Message::Pong(Pong {
            t_ms_echo: 42,
            uptime_ms: 100_000,
        }),
        Message::Health(Health {
            free_bytes: 200_000,
        }),
        Message::Diagnostic(Diagnostic {
            category: ErrorCategory::Framing,
            code: 5,
        }),
        Message::Error(ErrorReport {
            category: ErrorCategory::BadPayload,
            code: 9,
        }),
        Message::PlayMascotAction(PlayMascotAction {
            action: MascotAction::Tickle,
            personality: MascotPersonality::Playful,
            seed: 42,
        }),
        Message::MascotActionApplied(MascotActionApplied {
            action: MascotAction::Tickle,
            personality: MascotPersonality::Playful,
            seed: 42,
            applied_at_ms: 12_345,
        }),
    ]
}

#[test]
fn mascot_interaction_capability_is_independently_negotiable() {
    let advertised = Capabilities::MASCOT_INTERACTION.union(Capabilities::from_bits(0b1000));
    assert!(advertised.contains(Capabilities::MASCOT_INTERACTION));
    assert!(!Capabilities::NONE.contains(Capabilities::MASCOT_INTERACTION));
}

#[test]
fn every_message_round_trips() {
    for msg in all_messages() {
        assert_eq!(roundtrip(&msg), msg);
    }
}

#[test]
fn set_state_with_none_round_trips() {
    let msg = Message::SetState(SetState {
        desired: SendableState::Idle,
        at_ms: None,
    });
    assert_eq!(roundtrip(&msg), msg);
}

#[test]
fn takeover_bye_reasons_roundtrip_and_keep_their_wire_values() {
    // Append-only: the three original reasons keep 0..=2, the takeovers follow.
    let cases = [
        (ByeReason::IncompatibleVersion, 0u8),
        (ByeReason::Shutdown, 1),
        (ByeReason::ProtocolError, 2),
        (ByeReason::HostSleeping, 3),
        (ByeReason::FirmwareUpdate, 4),
    ];
    for (reason, wire_value) in cases {
        let msg = Message::Bye(Bye { reason });
        assert_eq!(roundtrip(&msg), msg);
        let v = ProtocolVersion::new(1, 5);
        let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
        encode_message(&msg, v, 1, &mut wire).unwrap();
        let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
        let (_, payload) =
            kivori_protocol::decode_frame(&wire[..wire.len() - 1], &mut scratch).unwrap();
        assert_eq!(
            payload.last().copied(),
            Some(wire_value),
            "{reason:?} wire value"
        );
    }
}
