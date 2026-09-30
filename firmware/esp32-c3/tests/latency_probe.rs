//! `latency-probe` (development-only): the on-panel detent -> flushed-frame latency readout.
#![cfg(feature = "latency-probe")]

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::latency_probe::{LatencyProbe, Readout, BOX_X, BOX_Y};
use kivori_firmware::ports::{Clock, DisplaySink};
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig};
use kivori_firmware::sim::{CaptureDisplay, ScriptedInput, SimPipe, VirtualClock, SCRIPT_CAPACITY};
use kivori_model::input::InputLevels;
use kivori_model::presentation::{PrimaryState, ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Capabilities, ProtocolVersion, Rect, Rgb565};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, InputKind, Message, Presentation,
    Ready, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const NONCE: u32 = 0x1A7E_0001;
/// Each tile write costs this many ms on the virtual clock, so "flush done" is distinguishable
/// from "presentation received".
const MS_PER_BLIT: u32 = 3;

/// A capture display whose every tile write advances the shared virtual clock.
struct SlowDisplay<'c> {
    inner: Box<CaptureDisplay>,
    clock: &'c VirtualClock,
}

impl DisplaySink for SlowDisplay<'_> {
    type Error = core::convert::Infallible;
    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        self.clock.advance(MS_PER_BLIT);
        self.inner.blit_tile(rect, pixels)
    }
}

struct Rig<'c> {
    runtime: Runtime<'static>,
    clock: &'c VirtualClock,
    pipe: SimPipe,
    display: SlowDisplay<'c>,
    blob: AssetBlob<'static>,
    seq: u16,
}

fn lv(a: bool, b: bool) -> InputLevels {
    InputLevels { a, b, sw: false }
}

impl<'c> Rig<'c> {
    fn new(clock: &'c VirtualClock) -> Self {
        let caps = Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::PRESENTATION_V1);
        let identity = DeviceIdentity {
            device_id: [0x1A; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            capabilities: caps,
        };
        let blob_bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let mut rig = Self {
            runtime: Runtime::new(identity, RuntimeConfig::default()),
            clock,
            pipe: SimPipe::new(),
            display: SlowDisplay {
                inner: Box::new(CaptureDisplay::new()),
                clock,
            },
            blob: AssetBlob::parse(blob_bytes).expect("valid blob"),
            seq: 0,
        };
        rig.send(&Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: caps,
            nonce: NONCE,
        }));
        rig.idle();
        rig.send(&Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: caps,
        }));
        rig.idle();
        let _ = rig.pipe.host_recv();
        rig
    }

    fn send(&mut self, msg: &Message) {
        let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
        let version = ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR);
        encode_message(msg, version, self.seq, &mut wire).expect("encode");
        self.seq += 1;
        self.pipe.host_send(&wire).expect("pipe capacity");
    }

    fn step_with(&mut self, input: &mut ScriptedInput) -> kivori_firmware::runtime::Tick {
        self.runtime.step(
            self.clock,
            &mut self.pipe,
            input,
            &mut self.display,
            &self.blob,
        )
    }

    fn idle(&mut self) -> kivori_firmware::runtime::Tick {
        let mut idle = ScriptedInput::new(HVec::from_slice(&[lv(false, false)]).unwrap());
        self.step_with(&mut idle)
    }

    /// Turns one CW detent and returns the device ms the `Detent` `InputEvent` carried on the wire.
    fn detent(&mut self) -> u32 {
        let levels: HVec<InputLevels, SCRIPT_CAPACITY> = HVec::from_slice(&[
            lv(false, false),
            lv(false, true),
            lv(true, true),
            lv(true, false),
            lv(false, false),
        ])
        .unwrap();
        let mut cw = ScriptedInput::new(levels);
        for _ in 0..5 {
            self.clock.advance(1);
            self.step_with(&mut cw);
        }
        let bytes = self.pipe.host_recv();
        let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
        bytes
            .split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .filter_map(|p| decode_message(p, &mut scratch, &[PROTOCOL_MAJOR]).ok())
            .find_map(|(_, m)| match m {
                Message::InputEvent(e) if matches!(e.kind, InputKind::Detent(_)) => {
                    Some(e.device_ms)
                }
                _ => None,
            })
            .expect("a Detent InputEvent on the wire")
    }

    fn present(&mut self, revision: u32, percent: u8) {
        self.send(&Message::Presentation(Presentation {
            session: NONCE,
            revision,
            primary: PrimaryState::Idle,
            value: Some(ValueDisplay {
                kind: ValueKind::Volume,
                current_percent: percent,
                confidence: ValueConfidence::Confirmed,
                at_boundary: false,
            }),
            transient_ms: 0,
        }));
    }

    /// Advances past the frame cadence and runs one rendered tick. Returns (tick start, tiles).
    fn frame(&mut self) -> (u32, u32) {
        self.clock.advance(40);
        let start = self.clock.now_ms();
        let tick = self.idle();
        assert!(tick.frame_rendered);
        (start, tick.tiles_flushed)
    }
}

#[test]
fn latency_is_flush_completion_minus_detent_time() {
    let clock = VirtualClock::new();
    let mut rig = Rig::new(&clock);
    let t_detent = rig.detent();
    rig.clock.advance(20); // desktop round trip
    rig.present(1, 40);
    let (received_at, tiles) = rig.frame();
    assert!(tiles > 0, "the presentation must flush tiles");
    let flush_done = received_at + tiles * MS_PER_BLIT;
    assert_eq!(
        rig.runtime.latency_readout().last,
        Some((flush_done - t_detent) as u16),
        "measured to flush completion, not to receipt ({} ms)",
        received_at - t_detent
    );

    // A slower second round trip updates `last`; `max` keeps the larger.
    let first = rig.runtime.latency_readout().last.unwrap();
    let t_detent = rig.detent();
    rig.clock.advance(200);
    rig.present(2, 45);
    let (received_at, tiles) = rig.frame();
    let second = (received_at + tiles * MS_PER_BLIT - t_detent) as u16;
    assert!(second > first);
    assert_eq!(
        rig.runtime.latency_readout(),
        Readout {
            last: Some(second),
            max: Some(second)
        }
    );

    // The next frame draws the readout: the "L" label's left stroke is white.
    rig.frame();
    assert_eq!(
        rig.display
            .inner
            .pixel(usize::from(BOX_X) + 2, usize::from(BOX_Y) + 4),
        Rgb565::WHITE
    );
}

#[test]
fn an_unanswered_detent_expires_after_one_second() {
    let clock = VirtualClock::new();
    let mut rig = Rig::new(&clock);
    rig.detent();
    rig.clock.advance(1000);
    rig.present(1, 40);
    rig.frame();
    assert_eq!(rig.runtime.latency_readout(), Readout::default());
}

#[test]
fn a_presentation_without_a_new_revision_does_not_complete_a_measurement() {
    let clock = VirtualClock::new();
    let mut rig = Rig::new(&clock);
    rig.detent();
    rig.present(1, 40);
    rig.frame();
    let after_first = rig.runtime.latency_readout();
    assert!(after_first.last.is_some());

    let t_detent = rig.detent();
    rig.present(1, 50); // same revision: rejected as stale
    rig.frame();
    assert_eq!(rig.runtime.latency_readout(), after_first);

    rig.present(2, 50);
    let (received_at, tiles) = rig.frame();
    assert_eq!(
        rig.runtime.latency_readout().last,
        Some((received_at + tiles * MS_PER_BLIT - t_detent) as u16)
    );
}

#[test]
fn a_presentation_applied_before_the_detent_does_not_count() {
    let mut probe = LatencyProbe::new();
    probe.on_presentation(10);
    probe.on_detent(20);
    assert_eq!(probe.on_frame_flushed(30), None);
    probe.on_presentation(40);
    assert_eq!(probe.on_frame_flushed(55), Some(35));
    probe.on_detent(100);
    probe.on_presentation(200);
    assert_eq!(
        probe.on_frame_flushed(5_000),
        Some(999),
        "clamped to 3 digits"
    );
}
