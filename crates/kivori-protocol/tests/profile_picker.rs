//! The profile picker and the hold ring (tags 25 to 27, capabilities `PROFILE_PICKER_V1` and
//! `HOLD_RING_V1`, protocol minor 4) and Hold + turn mode (tag 28, `HELD_TURN_MODE_V1`, minor 5):
//! the additive-change evidence of ADR-0002. A peer from before each change (frozen here as
//! `Minor3Message`, tags 0 to 24, and `Minor4Message`, tags 0 to 27) keeps decoding everything it
//! knew, and a new message reaches a peer only if it negotiated the capability.

use heapless::Vec;
use kivori_model::desk::MediaText;
use kivori_model::picker::{
    HoldActions, PickerName, ProfileChoices, ProfilePick, PICKER_PINNED_MAX,
};
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_frame, decode_message, encode_message, negotiate, BuddySettings, BuildInfo, Bye,
    ControlLabelsUpdate, Diagnostic, ErrorReport, Feedback, Health, HeldTurnModeUpdate, Hello,
    HelloAck, HoldActionsUpdate, Identity, InputEvent, InternalsReport, InternalsRequest,
    MascotActionApplied, MediaInfoUpdate, Message, MicStateUpdate, Ping, PlayMascotAction, Pong,
    Presentation, ProfileChoicesUpdate, ProfilePickReport, Ready, SetState, StateReport, Status,
    MAX_FRAME, MAX_PAYLOAD, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use serde::Deserialize;

/// The `Message` enum exactly as protocol 0.3 shipped it (tags 0 to 24). Never edit: it stands for
/// the firmware and apps already in the field.
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

/// The `Message` enum exactly as protocol 0.4 shipped it (tags 0 to 27). Never edit.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum Minor4Message {
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
    ProfileChoices(ProfileChoicesUpdate),
    ProfilePick(ProfilePickReport),
    HoldActions(HoldActionsUpdate),
}

const VERSION: ProtocolVersion = ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR);

fn payload(msg: &Message) -> std::vec::Vec<u8> {
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, VERSION, 1, &mut wire).unwrap();
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    decode_frame(&wire[..wire.len() - 1], &mut scratch)
        .unwrap()
        .1
        .to_vec()
}

fn roundtrip(msg: &Message) -> Message {
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, VERSION, 1, &mut wire).unwrap();
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    decode_message(&wire[..wire.len() - 1], &mut scratch, &[PROTOCOL_MAJOR])
        .unwrap()
        .1
}

fn full_choices() -> ProfileChoices {
    let name = PickerName::from_text(&"x".repeat(40));
    ProfileChoices {
        generation: 255,
        count: PICKER_PINNED_MAX as u8,
        selected: PICKER_PINNED_MAX as u8,
        names: [name; PICKER_PINNED_MAX],
    }
}

#[test]
fn the_picker_messages_are_tags_25_to_27_and_roundtrip() {
    let choices = Message::ProfileChoices(ProfileChoicesUpdate {
        session: u32::MAX,
        choices: full_choices(),
    });
    let pick = Message::ProfilePick(ProfilePickReport {
        session: 7,
        pick: ProfilePick {
            generation: 3,
            index: 2,
        },
    });
    let holds = Message::HoldActions(HoldActionsUpdate {
        session: 7,
        actions: HoldActions {
            buttons: [
                MediaText::from_text("Mute"),
                MediaText::from_text("Next profile"),
                MediaText::default(),
            ],
        },
    });
    for (msg, tag) in [(choices, 25), (pick, 26), (holds, 27)] {
        assert_eq!(roundtrip(&msg), msg);
        let bytes = payload(&msg);
        assert_eq!(bytes[0], tag);
        assert!(bytes.len() <= MAX_PAYLOAD, "{} bytes", bytes.len());
    }
}

#[test]
fn a_pick_has_stable_wire_bytes() {
    // tag 26, session 7, generation 3, index 2
    assert_eq!(
        payload(&Message::ProfilePick(ProfilePickReport {
            session: 7,
            pick: ProfilePick {
                generation: 3,
                index: 2
            },
        })),
        [26, 7, 3, 2]
    );
}

#[test]
fn the_capability_bits_are_stable_and_each_needs_both_peers() {
    assert_eq!(Capabilities::PROFILE_PICKER_V1.bits(), 1 << 18);
    assert_eq!(Capabilities::HOLD_RING_V1.bits(), 1 << 19);
    assert_eq!(Capabilities::KNOB_HOLD_V1.bits(), 1 << 20);
    assert_eq!(Capabilities::HELD_TURN_MODE_V1.bits(), 1 << 21);
    let both = Capabilities::PROFILE_PICKER_V1.union(Capabilities::HOLD_RING_V1);
    let (_, negotiated) = negotiate(PROTOCOL_MINOR, PROTOCOL_MINOR, both, both);
    assert!(negotiated.contains(both));
    // An older desktop (neither bit) never gets them.
    let (_, negotiated) = negotiate(PROTOCOL_MINOR, 3, both, Capabilities::PHYSICAL_INPUT_V1);
    assert!(!negotiated.contains(Capabilities::PROFILE_PICKER_V1));
    assert!(!negotiated.contains(Capabilities::HOLD_RING_V1));
}

#[test]
fn a_minor_3_peer_still_decodes_what_it_knew_and_rejects_the_new_tags() {
    let known = Message::Ping(Ping { t_ms: 9 });
    postcard::from_bytes::<Minor3Message>(&payload(&known)).expect("old peer decodes a known tag");
    // This is why the capability gate matters: an old decoder would count tags 25 to 27 as malformed.
    for msg in [
        Message::ProfilePick(ProfilePickReport {
            session: 1,
            pick: ProfilePick {
                generation: 0,
                index: 0,
            },
        }),
        Message::ProfileChoices(ProfileChoicesUpdate {
            session: 1,
            choices: ProfileChoices::default(),
        }),
        Message::HoldActions(HoldActionsUpdate {
            session: 1,
            actions: HoldActions::default(),
        }),
    ] {
        assert!(postcard::from_bytes::<Minor3Message>(&payload(&msg)).is_err());
    }
}

#[test]
fn held_turn_mode_is_tag_28_and_roundtrips() {
    let mode = Message::HeldTurnMode(HeldTurnModeUpdate {
        session: 7,
        picker: false,
    });
    assert_eq!(roundtrip(&mode), mode);
    // tag 28, session 7, picker false
    assert_eq!(payload(&mode), [28, 7, 0]);
    let on = Message::HeldTurnMode(HeldTurnModeUpdate {
        session: u32::MAX,
        picker: true,
    });
    assert_eq!(roundtrip(&on), on);
    assert!(payload(&on).len() <= MAX_PAYLOAD);
}

#[test]
fn a_minor_4_peer_still_decodes_the_picker_and_rejects_held_turn_mode() {
    let known = Message::HoldActions(HoldActionsUpdate {
        session: 1,
        actions: HoldActions::default(),
    });
    postcard::from_bytes::<Minor4Message>(&payload(&known)).expect("0.4 decodes tag 27");
    let mode = Message::HeldTurnMode(HeldTurnModeUpdate {
        session: 1,
        picker: true,
    });
    assert!(postcard::from_bytes::<Minor4Message>(&payload(&mode)).is_err());
    let (_, negotiated) = negotiate(
        PROTOCOL_MINOR,
        4,
        Capabilities::HELD_TURN_MODE_V1.union(Capabilities::KNOB_HOLD_V1),
        Capabilities::PROFILE_PICKER_V1,
    );
    assert!(!negotiated.contains(Capabilities::HELD_TURN_MODE_V1));
    assert!(!negotiated.contains(Capabilities::KNOB_HOLD_V1));
}
