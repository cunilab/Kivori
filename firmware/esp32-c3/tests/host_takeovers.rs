//! Host takeovers on the real runtime over the wire: `Bye(HostSleeping)` keeps the buddy asleep
//! across the link loss, `Bye(FirmwareUpdate)` shows Updating for at most 120 s, and a host that
//! goes silent for 4 s is dropped. Host simulation only.

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::InputSource;
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig, Tick, HOST_SILENCE_MS, UPDATING_MAX_MS};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_framebuffer::hash_rgb565;
use kivori_model::input::InputLevels;
use kivori_model::{Capabilities, CompanionState, ProtocolVersion, SendableState};
use kivori_protocol::{
    encode_message, Bye, ByeReason, FirmwareVersion, Hello, Message, Nonce, Ping, Ready, SetState,
    MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

struct NoInput;

impl InputSource for NoInput {
    fn sample(&mut self) -> InputLevels {
        InputLevels {
            a: false,
            b: false,
            sw: false,
            keys: [false; 3],
        }
    }
}

struct Rig {
    runtime: Runtime<'static>,
    clock: VirtualClock,
    pipe: SimPipe,
    display: Box<CaptureDisplay>,
    blob: AssetBlob<'static>,
    seq: u16,
}

impl Rig {
    fn new() -> Self {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            device_id: [0x42; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 3,
                patch: 0,
            },
            capabilities: Capabilities::HOST_TAKEOVERS_V1,
        };
        let mut rig = Self {
            runtime: Runtime::new(identity, RuntimeConfig::default()),
            clock: VirtualClock::new(),
            pipe: SimPipe::new(),
            display: Box::new(CaptureDisplay::new()),
            blob: AssetBlob::parse(bytes).expect("valid blob"),
            seq: 0,
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

    /// A full handshake (new nonce, sequence restarts like a restarted desktop).
    fn connect(&mut self, nonce: Nonce, caps: Capabilities) {
        self.seq = 0;
        self.send(&Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: caps,
            nonce,
        }));
        self.step();
        self.send(&Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: caps,
        }));
        self.step();
        let _ = self.pipe.host_recv();
    }

    fn step(&mut self) -> Tick {
        let mut input = NoInput;
        self.runtime.step(
            &self.clock,
            &mut self.pipe,
            &mut input,
            self.display.as_mut(),
            &self.blob,
        )
    }

    /// Advances `ms` in 10 ms ticks.
    fn run(&mut self, ms: u32) {
        for _ in 0..ms / 10 {
            self.clock.advance(10);
            self.step();
            let _ = self.pipe.host_recv();
        }
    }

    /// Like [`Self::run`], with a `Ping` from the host every second.
    fn run_pinging(&mut self, ms: u32) {
        for i in 0..ms / 10 {
            if i % 100 == 99 {
                self.send(&Message::Ping(Ping { t_ms: i }));
            }
            self.clock.advance(10);
            self.step();
            let _ = self.pipe.host_recv();
        }
    }

    fn set_state(&mut self, desired: SendableState) {
        self.send(&Message::SetState(SetState {
            desired,
            at_ms: None,
        }));
        self.run(50);
    }

    fn bye(&mut self, reason: ByeReason) {
        self.send(&Message::Bye(Bye { reason }));
        self.run(50);
    }
}

const NEGOTIATED: Capabilities = Capabilities::HOST_TAKEOVERS_V1;

#[test]
fn bye_host_sleeping_then_link_loss_still_shows_sleeping() {
    let mut rig = Rig::new();
    rig.connect(1, NEGOTIATED);
    rig.set_state(SendableState::Busy);
    assert_eq!(rig.runtime.state(), CompanionState::Busy);

    rig.bye(ByeReason::HostSleeping);
    assert_eq!(rig.runtime.state(), CompanionState::Sleeping);
    // Well past the silence timeout and any later link loss: the buddy keeps sleeping.
    rig.run(HOST_SILENCE_MS * 2);
    assert_eq!(rig.runtime.state(), CompanionState::Sleeping);
}

#[test]
fn a_plain_shutdown_bye_still_goes_offline() {
    let mut rig = Rig::new();
    rig.connect(1, NEGOTIATED);
    rig.set_state(SendableState::Idle);
    rig.bye(ByeReason::Shutdown);
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
}

#[test]
fn an_unnegotiated_host_sleeping_bye_is_inert() {
    let mut rig = Rig::new();
    rig.connect(1, Capabilities::NONE);
    rig.set_state(SendableState::Idle);
    rig.bye(ByeReason::HostSleeping);
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
}

#[test]
fn a_new_hello_clears_the_sleep_latch() {
    let mut rig = Rig::new();
    rig.connect(1, NEGOTIATED);
    rig.bye(ByeReason::HostSleeping);
    assert_eq!(rig.runtime.state(), CompanionState::Sleeping);

    rig.connect(2, NEGOTIATED);
    rig.set_state(SendableState::Idle);
    assert_eq!(rig.runtime.state(), CompanionState::Idle);
    // The next link loss is an ordinary one again.
    rig.run(HOST_SILENCE_MS + 100);
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
}

#[test]
fn four_seconds_of_silence_goes_offline() {
    let mut rig = Rig::new();
    rig.connect(1, NEGOTIATED);
    rig.set_state(SendableState::Happy);
    rig.run(HOST_SILENCE_MS - 500);
    assert_eq!(rig.runtime.state(), CompanionState::Happy);
    rig.run(1_000);
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
}

#[test]
fn a_ping_inside_four_seconds_keeps_the_session() {
    let mut rig = Rig::new();
    rig.connect(1, NEGOTIATED);
    rig.set_state(SendableState::Happy);
    rig.run_pinging(HOST_SILENCE_MS * 3);
    assert_eq!(rig.runtime.state(), CompanionState::Happy);
}

#[test]
fn silence_without_a_session_changes_nothing() {
    let mut rig = Rig::new();
    rig.run(HOST_SILENCE_MS * 2);
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
}

#[test]
fn bye_firmware_update_shows_updating_and_expires_after_120_seconds() {
    let mut rig = Rig::new();
    rig.connect(1, NEGOTIATED);
    rig.set_state(SendableState::Idle);
    rig.run(100);
    let before = hash_rgb565(rig.display.frame());

    rig.bye(ByeReason::FirmwareUpdate);
    rig.run(100);
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
    let updating = hash_rgb565(rig.display.frame());
    assert_ne!(updating, before, "the Updating screen replaces the buddy");

    rig.run(UPDATING_MAX_MS - 5_000);
    assert_eq!(hash_rgb565(rig.display.frame()), updating, "still updating");
    rig.run(6_000);
    let after = hash_rgb565(rig.display.frame());
    assert_ne!(after, updating, "the takeover lapsed");
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
}

#[test]
fn a_new_hello_ends_the_updating_takeover() {
    let mut rig = Rig::new();
    rig.connect(1, NEGOTIATED);
    rig.bye(ByeReason::FirmwareUpdate);
    rig.run(100);
    let updating = hash_rgb565(rig.display.frame());

    rig.connect(2, NEGOTIATED);
    rig.set_state(SendableState::Idle);
    rig.run(100);
    assert_ne!(hash_rgb565(rig.display.frame()), updating);
}
