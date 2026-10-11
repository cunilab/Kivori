//! Idle life on the real runtime (ADR-0012): the buddy looks towards a control as it is touched
//! (before the action's own feedback), reactions wait for the desktop's settings, and brightness
//! reaches the display sink. Host simulation only.

use std::cell::Cell;

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::InputSource;
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig, Tick};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_model::input::InputLevels;
use kivori_model::{
    Capabilities, CompanionState, MascotPersonality, ProtocolVersion, SendableState,
};
use kivori_protocol::{
    decode_message, encode_message, BuddySettings, FirmwareVersion, Hello, InputEvent, InputKind,
    Message, Nonce, Ping, Ready, SetState, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const CAPS: Capabilities = Capabilities::PHYSICAL_INPUT_V1
    .union(Capabilities::BUTTON_INPUT_V1)
    .union(Capabilities::CONTEXT_BUTTONS_V1)
    .union(Capabilities::IDLE_LIFE_V1);

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

    fn key(&mut self, index: usize, down: bool) {
        let mut keys = self.levels.1.get();
        keys[index] = down;
        self.levels.1.set(keys);
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

    fn settings(&mut self, session: Nonce, brightness: u8) {
        self.send(&Message::BuddySettings(BuddySettings {
            session,
            personality: MascotPersonality::Playful,
            self_play: true,
            brightness,
        }));
        self.run(50);
    }

    /// The eye region (crop x88..152, y96..136) of the last frame.
    fn eyes(&self) -> Vec<kivori_model::Rgb565> {
        let frame = self.display.frame();
        (96..136)
            .flat_map(|y| (88..152).map(move |x| frame[y * 240 + x]))
            .collect()
    }
}

/// docs/validation.md 13.10: touch a control and the buddy looks towards it before the action's own
/// feedback. The look starts on the touch itself (no round trip, no event sent yet) and is gone a
/// moment after the last touch.
#[test]
fn the_buddy_looks_towards_a_button_as_it_goes_down_before_anything_is_sent() {
    for index in 0..3 {
        let mut rig = Rig::idle();
        // Let any blink or glance of the seeded schedule pass, then settle on a calm frame.
        rig.run(7_300);
        let before = rig.eyes();
        let _ = rig.drain();
        rig.key(index, true);
        rig.run(250);
        let looking = rig.eyes();
        assert_ne!(looking, before, "button {index}: the eyes did not move");
        assert!(
            rig.drain().iter().all(|m| !matches!(
                m,
                Message::InputEvent(InputEvent {
                    kind: InputKind::Press,
                    ..
                })
            )),
            "the look must not wait for the press event"
        );
        rig.key(index, false);
        rig.run(2_000);
    }
}

/// Reactions wait for the desktop: a host without idle life schedules them itself.
#[test]
fn settings_are_applied_only_from_the_accepted_session_with_the_capability() {
    let mut rig = Rig::idle();
    assert_eq!(rig.display.brightness, None);
    let wrong = rig.nonce ^ 1;
    rig.settings(wrong, 2);
    assert_eq!(rig.display.brightness, None, "another session is ignored");
    let session = rig.nonce;
    rig.settings(session, 2);
    assert_eq!(rig.display.brightness, Some(2));
    // Out-of-range steps clamp into the supported range, never below the lowest lit step.
    rig.settings(session, 0);
    assert_eq!(rig.display.brightness, Some(1));
    rig.settings(session, 200);
    assert_eq!(
        rig.display.brightness,
        Some(kivori_model::desk::BRIGHTNESS_STEPS)
    );
}

#[test]
fn settings_without_the_negotiated_capability_are_ignored() {
    let mut rig = Rig::new();
    rig.connect(CAPS.intersection(Capabilities::PHYSICAL_INPUT_V1));
    let session = rig.nonce;
    rig.settings(session, 2);
    assert_eq!(rig.display.brightness, None);
}

/// docs/validation.md 13.9: with the host silent (but not yet timed out) the buddy keeps animating.
#[test]
fn the_buddy_keeps_moving_while_the_host_sends_nothing() {
    let mut rig = Rig::idle();
    let session = rig.nonce;
    rig.settings(session, 5);
    let mut seen = std::collections::HashSet::new();
    for i in 0..60 {
        // Only the one-second heartbeat arrives; nothing else, for one minute.
        rig.send(&Message::Ping(Ping { t_ms: i }));
        rig.run(1_000);
        let _ = rig.drain();
        seen.insert(rig.eyes());
    }
    assert_eq!(rig.runtime.state(), CompanionState::Idle);
    assert!(
        seen.len() > 3,
        "only {} distinct eye frames in 1 minute",
        seen.len()
    );
}

#[test]
fn units_with_different_ids_keep_different_idle_schedules() {
    let frames = |device_id: [u8; 16]| {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            device_id,
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            capabilities: CAPS,
        };
        let mut runtime = Runtime::new(identity, RuntimeConfig::default());
        let clock = VirtualClock::new();
        let mut pipe = SimPipe::new();
        let mut display = Box::new(CaptureDisplay::new());
        let blob = AssetBlob::parse(bytes).unwrap();
        let levels = Levels::default();
        let mut out = Vec::new();
        for _ in 0..600 {
            clock.advance(10);
            let mut input = &levels;
            runtime.step(&clock, &mut pipe, &mut input, display.as_mut(), &blob);
            out.push(display.frame().to_vec());
        }
        out
    };
    let id = |first: u8| {
        let mut id = [0; 16];
        id[0] = first;
        id
    };
    assert_eq!(frames(id(1)), frames(id(1)));
    assert_ne!(frames(id(1)), frames(id(9)));
}
