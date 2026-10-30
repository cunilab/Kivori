//! Truthful mic mute on the real runtime (ADR-0011): the indicator and the covered-mouth face
//! follow the desktop's `MicState`, only from the accepted session with the capability, and are
//! forgotten when the session ends. Host simulation only.

use std::cell::Cell;

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::InputSource;
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig, Tick};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_model::desk::MicState;
use kivori_model::input::InputLevels;
use kivori_model::{Capabilities, CompanionState, ProtocolVersion, SendableState};
use kivori_protocol::{
    decode_message, encode_message, Bye, ByeReason, FirmwareVersion, Hello, Message,
    MicStateUpdate, Nonce, Ready, SetState, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const CAPS: Capabilities = Capabilities::PHYSICAL_INPUT_V1
    .union(Capabilities::BUTTON_INPUT_V1)
    .union(Capabilities::CONTEXT_BUTTONS_V1)
    .union(Capabilities::MIC_STATE_V1);

/// Levels the test sets between steps.
#[derive(Default)]
struct Levels(Cell<(bool, bool, bool)>, Cell<[bool; 3]>);

impl InputSource for &Levels {
    fn sample(&mut self) -> InputLevels {
        let (a, b, sw) = self.0.get();
        InputLevels {
            a,
            b,
            sw,
            keys: self.1.get(),
        }
    }
}

struct Rig {
    runtime: Runtime<'static>,
    clock: VirtualClock,
    pipe: SimPipe,
    display: Box<CaptureDisplay>,
    blob: AssetBlob<'static>,
    levels: Levels,
    seq: u16,
    nonce: Nonce,
}

impl Rig {
    fn new() -> Self {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            device_id: [0x42; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            capabilities: CAPS,
        };
        let mut rig = Self {
            runtime: Runtime::new(identity, RuntimeConfig::default()),
            clock: VirtualClock::new(),
            pipe: SimPipe::new(),
            display: Box::new(CaptureDisplay::new()),
            blob: AssetBlob::parse(bytes).expect("valid blob"),
            levels: Levels::default(),
            seq: 0,
            nonce: 0x5EED_0001,
        };
        rig.step();
        rig
    }

    fn send(&mut self, message: &Message) {
        let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
        encode_message(
            message,
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            self.seq,
            &mut wire,
        )
        .expect("encode");
        self.seq += 1;
        self.pipe.host_send(&wire).expect("pipe capacity");
    }

    fn connect(&mut self, caps: Capabilities) {
        let hello = Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: caps,
            nonce: self.nonce,
        });
        self.send(&hello);
        self.step();
        self.send(&Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: caps,
        }));
        self.step();
        let _ = self.drain();
    }

    fn step(&mut self) -> Tick {
        let mut input = &self.levels;
        self.runtime.step(
            &self.clock,
            &mut self.pipe,
            &mut input,
            self.display.as_mut(),
            &self.blob,
        )
    }

    /// Advances `ms` in 10 ms ticks, returning whether any tick asked for a reboot.
    fn run(&mut self, ms: u32) -> bool {
        let mut reboot = false;
        for _ in 0..ms / 10 {
            self.clock.advance(10);
            reboot |= self.step().reboot;
        }
        reboot
    }

    fn drain(&mut self) -> Vec<Message> {
        let bytes = self.pipe.host_recv();
        let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
        bytes
            .split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .filter_map(|p| decode_message(p, &mut scratch, &[PROTOCOL_MAJOR]).ok())
            .map(|(_, m)| m)
            .collect()
    }
}

impl Rig {
    /// A fresh Idle buddy on a connected session.
    fn idle() -> Self {
        let mut rig = Self::new();
        rig.connect(CAPS);
        rig.send(&Message::SetState(SetState {
            desired: SendableState::Idle,
            at_ms: None,
        }));
        rig.run(1_000);
        assert_eq!(rig.runtime.state(), CompanionState::Idle);
        rig
    }

    fn mic(&mut self, session: Nonce, muted: Option<bool>, in_use: Option<bool>) {
        self.send(&Message::MicState(MicStateUpdate {
            session,
            mic: MicState { muted, in_use },
        }));
        self.run(60);
    }

    /// The top-right indicator slot of the last frame.
    fn slot(&self) -> Vec<kivori_model::Rgb565> {
        let frame = self.display.frame();
        (8..32)
            .flat_map(|y| (208..232).map(move |x| frame[y * 240 + x]))
            .collect()
    }

    /// Whether the mouth wears the covered-mouth patch: its fill (`#35a87c`) appears in the mouth
    /// crop, where only the plain cap colour is otherwise. Breathing moves the buddy a little, so a
    /// whole-region comparison between two moments would be too strict.
    fn covered(&self) -> bool {
        let frame = self.display.frame();
        let fill = kivori_model::Rgb565::from_rgb888(0x35, 0xa8, 0x7c);
        (139..152)
            .flat_map(|y| (108..132).map(move |x| frame[y * 240 + x]))
            .filter(|p| *p == fill)
            .count()
            > 40
    }
}

#[test]
fn muted_shows_the_indicator_and_the_covered_mouth() {
    let mut rig = Rig::idle();
    rig.run(300);
    let slot = rig.slot();
    assert!(!rig.covered());
    let session = rig.nonce;
    rig.mic(session, Some(true), None);
    rig.run(500);
    assert_ne!(rig.slot(), slot, "no mic indicator");
    assert!(rig.covered(), "no covered mouth");
    // Unknown again: both go, nothing claims live.
    rig.mic(session, None, None);
    rig.run(500);
    assert_eq!(rig.slot(), slot);
    assert!(!rig.covered());
}

#[test]
fn live_shows_a_cue_only_while_in_use_and_never_the_muted_face() {
    let mut rig = Rig::idle();
    rig.run(300);
    let slot = rig.slot();
    let session = rig.nonce;
    rig.mic(session, Some(false), Some(false));
    assert_eq!(rig.slot(), slot, "not in use: no cue");
    rig.mic(session, Some(false), None);
    assert_eq!(rig.slot(), slot, "unknown use: no cue, never 'not live'");
    rig.mic(session, Some(false), Some(true));
    assert_ne!(rig.slot(), slot, "in use and unmuted: Live");
    assert!(!rig.covered(), "live is not the covered mouth");
}

#[test]
fn only_the_accepted_session_with_the_capability_counts() {
    let mut rig = Rig::idle();
    rig.run(300);
    let slot = rig.slot();
    let wrong = rig.nonce ^ 1;
    rig.mic(wrong, Some(true), None);
    assert_eq!(rig.slot(), slot, "another session is ignored");

    // Without the capability the message is inert.
    let mut rig = Rig::new();
    rig.connect(Capabilities::PHYSICAL_INPUT_V1);
    rig.send(&Message::SetState(SetState {
        desired: SendableState::Idle,
        at_ms: None,
    }));
    rig.run(1_300);
    let slot = rig.slot();
    let session = rig.nonce;
    rig.mic(session, Some(true), None);
    assert_eq!(rig.slot(), slot);
}

#[test]
fn a_session_boundary_forgets_the_mic() {
    let mut rig = Rig::idle();
    rig.run(300);
    let session = rig.nonce;
    rig.mic(session, Some(true), None);
    rig.send(&Message::Bye(Bye {
        reason: ByeReason::Shutdown,
    }));
    rig.run(1_500);
    // Back on a new session nothing is known until the desktop says so.
    rig.nonce += 1;
    rig.connect(CAPS);
    rig.send(&Message::SetState(SetState {
        desired: SendableState::Idle,
        at_ms: None,
    }));
    rig.run(1_500);
    let slot = rig.slot();
    rig.mic(rig.nonce, Some(true), None);
    assert_ne!(rig.slot(), slot);
}
