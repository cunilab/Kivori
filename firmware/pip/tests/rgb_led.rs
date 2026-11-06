//! The RGB LED on the real runtime (`RGB_LED_V1`): dark until the app sends settings, accepted only
//! from the accepted session with the capability, and then driven by the device's own state (the
//! face, the mic, the session, a press). Host simulation only: it reads the engine the runtime
//! publishes, never a pin.

use std::cell::Cell;

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::led::FLASH_MS;
use kivori_firmware::ports::{Clock, InputSource};
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig};
use kivori_firmware::sim::{CaptureDisplay, CaptureLed, SimPipe, VirtualClock};
use kivori_model::desk::MicState;
use kivori_model::input::InputLevels;
use kivori_model::led::{LedMode, LedSpeed};
use kivori_model::{Capabilities, CompanionState, ProtocolVersion, SendableState};
use kivori_protocol::{
    encode_message, Bye, ByeReason, FirmwareVersion, Hello, LedSettings, Message, MicStateUpdate,
    Nonce, Ping, Ready, SetState, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const CAPS: Capabilities = Capabilities::PHYSICAL_INPUT_V1
    .union(Capabilities::BUTTON_INPUT_V1)
    .union(Capabilities::CONTEXT_BUTTONS_V1)
    .union(Capabilities::MIC_STATE_V1)
    .union(Capabilities::RGB_LED_V1);

/// The duty `100 %` of this colour gives (gamma included): what the engine drives for it.
fn full(color: [u8; 3]) -> [u8; 3] {
    color.map(|c| (u32::from(c) * u32::from(c)).div_ceil(255) as u8)
}

#[derive(Default)]
struct Levels(Cell<bool>, Cell<[bool; 3]>);

impl InputSource for &Levels {
    fn sample(&mut self) -> InputLevels {
        InputLevels {
            a: false,
            b: false,
            sw: self.0.get(),
            keys: self.1.get(),
        }
    }
}

struct Rig {
    runtime: Runtime<'static>,
    clock: VirtualClock,
    pipe: SimPipe,
    display: Box<CaptureDisplay>,
    led: CaptureLed,
    blob: AssetBlob<'static>,
    levels: Levels,
    seq: u16,
    nonce: Nonce,
}

impl Rig {
    fn new(caps: Capabilities) -> Self {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            device_id: [0x42; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            capabilities: caps,
        };
        let mut rig = Self {
            runtime: Runtime::new(identity, RuntimeConfig::default()),
            clock: VirtualClock::new(),
            pipe: SimPipe::new(),
            display: Box::new(CaptureDisplay::new()),
            led: CaptureLed::new(),
            blob: AssetBlob::parse(bytes).expect("valid blob"),
            levels: Levels::default(),
            seq: 0,
            nonce: 0x5EED_0001,
        };
        rig.step();
        rig.connect(caps);
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
        self.send(&Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: caps,
            nonce: self.nonce,
        }));
        self.step();
        self.send(&Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: caps,
        }));
        self.step();
        let _ = self.pipe.host_recv();
    }

    fn step(&mut self) {
        let mut input = &self.levels;
        self.runtime.step(
            &self.clock,
            &mut self.pipe,
            &mut input,
            self.display.as_mut(),
            &self.blob,
        );
        self.led.tick(self.runtime.led(), self.clock.now_ms());
    }

    /// Advances `ms` in 10 ms ticks, pinging once a second so the host never reads as gone.
    fn run(&mut self, ms: u32) {
        for _ in 0..ms / 10 {
            self.clock.advance(10);
            if self.clock.now_ms().is_multiple_of(1_000) {
                self.send(&Message::Ping(Ping {
                    t_ms: self.clock.now_ms(),
                }));
            }
            self.step();
            let _ = self.pipe.host_recv();
        }
    }

    fn state(&mut self, state: SendableState) {
        self.send(&Message::SetState(SetState {
            desired: state,
            at_ms: None,
        }));
        // Long enough for the face to finish its transition.
        self.run(1_500);
    }

    fn led(&mut self, mode: LedMode, color: [u8; 3], brightness: u8) {
        let session = self.nonce;
        self.led_for(session, mode, color, brightness);
    }

    fn led_for(&mut self, session: Nonce, mode: LedMode, color: [u8; 3], brightness: u8) {
        self.send(&Message::LedSettings(LedSettings {
            session,
            mode,
            color,
            brightness,
            speed: LedSpeed::Medium,
        }));
        self.run(30);
    }

    fn mic(&mut self, muted: bool) {
        self.send(&Message::MicState(MicStateUpdate {
            session: self.nonce,
            mic: MicState {
                muted: Some(muted),
                in_use: None,
            },
        }));
        self.run(60);
    }

    /// What the LED shows now.
    fn rgb(&self) -> [u8; 3] {
        self.runtime.led().frame(self.clock.now_ms())
    }

    /// The brightest the LED got over `ms`, per channel.
    fn peak(&mut self, ms: u32) -> [u8; 3] {
        let mut peak = [0u8; 3];
        for _ in 0..ms / 10 {
            self.run(10);
            let now = self.rgb();
            peak = [0, 1, 2].map(|i| peak[i].max(now[i]));
        }
        peak
    }
}

#[test]
fn the_led_is_dark_until_the_app_sends_settings() {
    let mut rig = Rig::new(CAPS);
    rig.state(SendableState::Idle);
    rig.mic(true);
    assert_eq!(
        rig.peak(2_000),
        [0; 3],
        "a connected device with no app settings stays dark"
    );
    assert_eq!(rig.led.rgb, [0; 3]);
}

#[test]
fn solid_follows_the_settings_and_off_turns_it_dark() {
    let mut rig = Rig::new(CAPS);
    rig.led(LedMode::Solid, [255, 128, 0], 100);
    assert_eq!(rig.rgb(), full([255, 128, 0]));
    assert_eq!(rig.led.rgb, full([255, 128, 0]));
    rig.led(LedMode::Solid, [0, 0, 255], 100);
    assert_eq!(rig.led.rgb, [0, 0, 255]);
    rig.led(LedMode::Solid, [0, 0, 255], 0);
    assert_eq!(rig.led.rgb, [0; 3], "brightness 0 is dark");
    rig.led(LedMode::Off, [0, 0, 255], 100);
    assert_eq!(rig.led.rgb, [0; 3]);
}

#[test]
fn only_the_accepted_session_with_the_capability_counts() {
    let mut rig = Rig::new(CAPS);
    let wrong = rig.nonce ^ 1;
    rig.led_for(wrong, LedMode::Solid, [255, 0, 0], 100);
    assert_eq!(rig.led.rgb, [0; 3], "another session is ignored");

    // A device without the capability negotiated treats the message as unknown and inert.
    let mut rig = Rig::new(CAPS.intersection(Capabilities::PHYSICAL_INPUT_V1));
    rig.led(LedMode::Solid, [255, 0, 0], 100);
    assert_eq!(rig.led.rgb, [0; 3]);
}

#[test]
fn mood_follows_the_face_and_goes_dim_when_the_host_is_gone() {
    let mut rig = Rig::new(CAPS);
    rig.led(LedMode::Mood, [0; 3], 100);
    rig.state(SendableState::Idle);
    assert_eq!(rig.runtime.state(), CompanionState::Idle);
    // The welcome face after a connect has passed by now.
    rig.run(4_000);
    let idle = rig.peak(5_000);
    assert_eq!(
        idle,
        full([46, 196, 182]),
        "Idle breathes up to the soft teal"
    );
    rig.state(SendableState::Busy);
    let busy = rig.peak(5_000);
    assert_eq!(busy, full([255, 80, 10]), "Busy is orange");
    assert_ne!(idle, busy);
    rig.state(SendableState::Idle);
    rig.mic(true);
    // The covered-mouth face is the mic's own colour, not a status override.
    assert_eq!(rig.peak(5_000), full([230, 30, 70]));
    rig.mic(false);

    rig.send(&Message::Bye(Bye {
        reason: ByeReason::Shutdown,
    }));
    rig.run(4_000);
    let offline = rig.peak(6_000);
    assert!(
        offline.into_iter().max().unwrap() < 40,
        "Offline is dim: {offline:?}"
    );
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
}

#[test]
fn status_shows_the_base_colour_red_for_a_muted_mic_and_amber_without_a_host() {
    let mut rig = Rig::new(CAPS);
    rig.state(SendableState::Idle);
    rig.led(LedMode::Status, [10, 200, 30], 100);
    assert_eq!(rig.led.rgb, full([10, 200, 30]));
    rig.mic(true);
    assert_eq!(rig.led.rgb, [255, 0, 0], "muted mic: solid red");
    rig.mic(false);
    assert_eq!(rig.led.rgb, full([10, 200, 30]));

    // The host goes: dim amber pulse, and the settings persist across the session end.
    rig.send(&Message::Bye(Bye {
        reason: ByeReason::Shutdown,
    }));
    rig.run(500);
    let peak = rig.peak(3_000);
    assert!(
        peak[0] > peak[1] && peak[1] > 0 && peak[2] == 0,
        "amber: {peak:?}"
    );
    assert!(peak[0] < 128, "dim: {peak:?}");
}

#[test]
fn a_knob_press_and_a_key_press_flash_white_in_status() {
    let mut rig = Rig::new(CAPS);
    rig.state(SendableState::Idle);
    rig.led(LedMode::Status, [10, 200, 30], 100);
    let base = rig.rgb();
    assert_eq!(rig.peak(300), base);

    for press in [
        |l: &Levels| l.0.set(true),
        |l: &Levels| l.1.set([false, true, false]),
    ] {
        press(&rig.levels);
        // The switch debounces for 20 ms, then the flash is the next thing the LED does.
        let started = rig.clock.now_ms();
        let mut flashed = None;
        while rig.clock.now_ms() - started < 200 {
            rig.run(10);
            if rig.rgb() == [255; 3] {
                flashed.get_or_insert(rig.clock.now_ms());
            }
        }
        let flashed = flashed.expect("the press flashed the LED");
        assert!(
            flashed - started <= 60,
            "flash begins within a debounce: {}",
            flashed - started
        );
        // It is over FLASH_MS after the press, with the key still held down.
        rig.run(200);
        assert_eq!(rig.rgb(), base, "flash is {FLASH_MS} ms, not while held");
        rig.levels.0.set(false);
        rig.levels.1.set([false; 3]);
        rig.run(600);
    }
}
