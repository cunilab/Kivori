//! `BuildInfo` (tag 24, capability `BUILD_INFO_V1`, protocol minor 2): the additive-change evidence
//! of ADR-0002. A peer from before the change (frozen here as `Minor1Message`, tags 0 to 23) must keep
//! decoding everything it knew, and the new message must reach it only if it negotiated the
//! capability, so it never sees an unknown tag.

use heapless::Vec;
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_frame, decode_message, encode_message, negotiate, BuddySettings, BuildInfo, Bye,
    ControlLabelsUpdate, Diagnostic, ErrorReport, Feedback, Health, Hello, HelloAck, Identity,
    IdentitySource, InputEvent, InternalsReport, InternalsRequest, MascotActionApplied,
    MediaInfoUpdate, Message, MicStateUpdate, Ping, PlayMascotAction, Pong, Presentation, Ready,
    SetState, StateReport, Status, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use serde::Deserialize;

/// The `Message` enum exactly as protocol 0.1 shipped it (tags 0 to 23). Never edit: it stands for
/// the firmware and apps already in the field.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum Minor1Message {
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
}

fn payload(msg: &Message, version: ProtocolVersion) -> std::vec::Vec<u8> {
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, version, 1, &mut wire).unwrap();
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    decode_frame(&wire[..wire.len() - 1], &mut scratch)
        .unwrap()
        .1
        .to_vec()
}

const COMMIT: [u8; 6] = [0xc2, 0xff, 0x8c, 0x4f, 0x68, 0xde];

fn sample(commit: Option<[u8; 6]>, dirty: bool) -> Message {
    Message::BuildInfo(BuildInfo {
        session: 7,
        commit,
        dirty,
    })
}

#[test]
fn build_info_is_appended_as_tag_24_and_roundtrips() {
    for (commit, dirty) in [(Some(COMMIT), false), (Some(COMMIT), true), (None, false)] {
        let msg = sample(commit, dirty);
        let version = ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR);
        let bytes = payload(&msg, version);
        assert_eq!(bytes[0], 24);
        assert!(bytes.len() <= 1 + 5 + 1 + 6 + 1);
        let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
        encode_message(&msg, version, 1, &mut wire).unwrap();
        let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
        let (_, back) =
            decode_message(&wire[..wire.len() - 1], &mut scratch, &[PROTOCOL_MAJOR]).unwrap();
        assert_eq!(back, msg);
    }
}

#[test]
fn build_info_wire_bytes_are_stable() {
    // tag 24, session 7, Some(1) + six commit bytes, dirty true
    assert_eq!(
        payload(
            &sample(Some(COMMIT), true),
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
        ),
        [24, 7, 1, 0xc2, 0xff, 0x8c, 0x4f, 0x68, 0xde, 1]
    );
    assert_eq!(
        payload(
            &sample(None, false),
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
        ),
        [24, 7, 0, 0]
    );
}

#[test]
fn a_minor_1_peer_still_decodes_every_message_it_knew() {
    // New encoder, old decoder: the tags the old peer knows are byte for byte what it expects,
    // including the neighbour `Identity` (tag 23).
    let known = [
        Message::Identity(Identity {
            session: 7,
            product: 1,
            board: 1,
            hw_rev: Some(2),
            source: IdentitySource::Chip,
        }),
        Message::Ping(Ping { t_ms: 9 }),
        Message::Pong(Pong {
            t_ms_echo: 9,
            uptime_ms: 100,
        }),
    ];
    for msg in &known {
        let bytes = payload(msg, ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR));
        postcard::from_bytes::<Minor1Message>(&bytes).expect("old peer decodes a known message");
    }
}

#[test]
fn a_minor_1_peer_rejects_build_info_as_an_unknown_tag() {
    // This is why the capability gate matters: the device never sends tag 24 to a peer that did
    // not negotiate it, because an old decoder would count it as a malformed message.
    let bytes = payload(
        &sample(Some(COMMIT), false),
        ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
    );
    assert!(postcard::from_bytes::<Minor1Message>(&bytes).is_err());
}

#[test]
fn an_old_encoder_is_decoded_unchanged_by_the_new_decoder() {
    // Old encoder, new decoder: bytes an old peer produces decode to the same message.
    let old = Message::Identity(Identity {
        session: 5,
        product: 1,
        board: 1,
        hw_rev: None,
        source: IdentitySource::Build,
    });
    let bytes = payload(&old, ProtocolVersion::new(PROTOCOL_MAJOR, 1));
    assert_eq!(postcard::from_bytes::<Message>(&bytes).unwrap(), old);
}

#[test]
fn build_info_is_only_negotiated_when_both_peers_have_it() {
    let both = Capabilities::BUILD_INFO_V1.union(Capabilities::PRODUCT_IDENTITY_V1);
    let (minor, caps) = negotiate(2, 2, both, both);
    assert_eq!(minor, 2);
    assert!(caps.contains(Capabilities::BUILD_INFO_V1));
    // Older desktop (1.x, no bit 16) with a newer device: the device must stay silent.
    let (minor, caps) = negotiate(1, 2, Capabilities::PRODUCT_IDENTITY_V1, both);
    assert_eq!(minor, 1);
    assert!(!caps.contains(Capabilities::BUILD_INFO_V1));
    // Newer desktop with an older device: nothing to receive, nothing to wait for.
    let (minor, caps) = negotiate(2, 1, both, Capabilities::PRODUCT_IDENTITY_V1);
    assert_eq!(minor, 1);
    assert!(!caps.contains(Capabilities::BUILD_INFO_V1));
}
