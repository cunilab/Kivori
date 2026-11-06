//! `LedSettings` (tag 25, capability `RGB_LED_V1`, protocol minor 4): a peer from before the change
//! (frozen here as `Minor3Message`, tags 0 to 24) keeps decoding everything it knew and rejects the
//! new tag, so the capability gate is what keeps it from ever seeing one.

use heapless::Vec;
use kivori_model::led::{LedMode, LedSpeed};
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_frame, decode_message, encode_message, negotiate, BuddySettings, BuildInfo, Bye,
    ControlLabelsUpdate, Diagnostic, ErrorReport, Feedback, Health, Hello, HelloAck, Identity,
    InputEvent, InternalsReport, InternalsRequest, LedSettings, MascotActionApplied,
    MediaInfoUpdate, Message, MicStateUpdate, Ping, PlayMascotAction, Pong, Presentation, Ready,
    SetState, StateReport, Status, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use serde::Deserialize;

/// The `Message` enum exactly as protocol 0.3 shipped it (tags 0 to 24). Never edit.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum Minor3Message {
    Hello(Hello),
    HelloAck(HelloAck),
    Ready(Ready),
    Bye(Bye),
    SetState(SetState),
    StateReport(StateReport),
    Ping(Ping),
    Pong(Pong),
    Health(Health),
    Diagnostic(Diagnostic),
    Error(ErrorReport),
    PlayMascotAction(PlayMascotAction),
    MascotActionApplied(MascotActionApplied),
    InputEvent(InputEvent),
    Presentation(Presentation),
    Status(Status),
    Feedback(Feedback),
    MediaInfo(MediaInfoUpdate),
    ControlLabels(ControlLabelsUpdate),
    InternalsRequest(InternalsRequest),
    Internals(InternalsReport),
    BuddySettings(BuddySettings),
    MicState(MicStateUpdate),
    Identity(Identity),
    BuildInfo(BuildInfo),
}

const VERSION: ProtocolVersion = ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR);

fn wire(msg: &Message) -> Vec<u8, MAX_WIRE> {
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, VERSION, 1, &mut wire).unwrap();
    wire
}

fn payload(msg: &Message) -> std::vec::Vec<u8> {
    let wire = wire(msg);
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    decode_frame(&wire[..wire.len() - 1], &mut scratch)
        .unwrap()
        .1
        .to_vec()
}

fn sample(mode: LedMode, speed: LedSpeed) -> Message {
    Message::LedSettings(LedSettings {
        session: 7,
        mode,
        color: [0x2E, 0xC4, 0xB6],
        brightness: 60,
        speed,
    })
}

#[test]
fn led_settings_are_appended_as_tag_25_and_roundtrip() {
    for mode in [
        LedMode::Off,
        LedMode::Solid,
        LedMode::Breathe,
        LedMode::Cycle,
        LedMode::Status,
        LedMode::Mood,
    ] {
        for speed in [LedSpeed::Slow, LedSpeed::Medium, LedSpeed::Fast] {
            let msg = sample(mode, speed);
            let bytes = payload(&msg);
            assert_eq!(bytes[0], 25);
            assert!(bytes.len() <= 1 + 5 + 1 + 3 + 1 + 1);
            let wire = wire(&msg);
            let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
            let (_, back) =
                decode_message(&wire[..wire.len() - 1], &mut scratch, &[PROTOCOL_MAJOR]).unwrap();
            assert_eq!(back, msg);
        }
    }
}

#[test]
fn led_settings_wire_bytes_are_stable() {
    // tag 25, session 7, mode Status (4), colour as three raw bytes, brightness 60, speed Fast (2).
    assert_eq!(
        payload(&Message::LedSettings(LedSettings {
            session: 7,
            mode: LedMode::Status,
            color: [0x2E, 0xC4, 0xB6],
            brightness: 60,
            speed: LedSpeed::Fast,
        })),
        [25, 7, 4, 0x2E, 0xC4, 0xB6, 60, 2]
    );
}

#[test]
fn the_variant_order_of_the_led_enums_is_the_wire_encoding() {
    // Append-only: these indices are what a device already in the field decodes.
    for (index, mode) in [
        LedMode::Off,
        LedMode::Solid,
        LedMode::Breathe,
        LedMode::Cycle,
        LedMode::Status,
        LedMode::Mood,
    ]
    .into_iter()
    .enumerate()
    {
        let mut buf = [0u8; 4];
        assert_eq!(postcard::to_slice(&mode, &mut buf).unwrap(), [index as u8]);
    }
    for (index, speed) in [LedSpeed::Slow, LedSpeed::Medium, LedSpeed::Fast]
        .into_iter()
        .enumerate()
    {
        let mut buf = [0u8; 4];
        assert_eq!(postcard::to_slice(&speed, &mut buf).unwrap(), [index as u8]);
    }
}

#[test]
fn a_minor_3_peer_still_decodes_what_it_knew_and_rejects_tag_25() {
    let known = Message::BuildInfo(BuildInfo {
        session: 7,
        source_id: None,
        dirty: false,
    });
    postcard::from_bytes::<Minor3Message>(&payload(&known)).expect("old peer decodes tag 24");
    let new = payload(&sample(LedMode::Solid, LedSpeed::Medium));
    assert!(postcard::from_bytes::<Minor3Message>(&new).is_err());
    // And the other way: bytes an old peer wrote decode unchanged.
    assert_eq!(
        postcard::from_bytes::<Message>(&payload(&known)).unwrap(),
        known
    );
}

#[test]
fn the_led_is_only_negotiated_when_both_peers_have_it() {
    let both = Capabilities::RGB_LED_V1;
    let (_, caps) = negotiate(4, 4, both, both);
    assert!(caps.contains(Capabilities::RGB_LED_V1));
    // A board without an LED never advertises the bit, so the desktop must not send the message.
    let (_, caps) = negotiate(4, 4, both, Capabilities::NONE);
    assert!(!caps.contains(Capabilities::RGB_LED_V1));
    // Older desktop, newer device.
    let (minor, caps) = negotiate(3, 4, Capabilities::NONE, both);
    assert_eq!(minor, 3);
    assert!(!caps.contains(Capabilities::RGB_LED_V1));
}
