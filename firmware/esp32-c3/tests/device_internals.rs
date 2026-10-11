//! Device internals over the wire on the real runtime (ADR-0013): a negotiated `InternalsRequest`
//! from the accepted session is answered with one snapshot; anything else is ignored. Host
//! simulation only.

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::internals::ASSET_BLOB_CAP_BYTES;
use kivori_firmware::ports::InputSource;
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_model::input::InputLevels;
use kivori_model::internals::{ChipModel, DeviceInternals};
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, InternalsReport, InternalsRequest,
    Message, Nonce, Ready, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

struct NoInput;

impl InputSource for NoInput {
    fn sample(&mut self) -> InputLevels {
        InputLevels::default()
    }

    fn dropped_edges(&self) -> u32 {
        3
    }

    fn edge_high_water(&self) -> u32 {
        9
    }
}

/// A stand-in for the board: it measures a few things and cannot read the temperature.
fn probe(out: &mut DeviceInternals) {
    out.app_image_bytes = Some(700_000);
    out.asset_blob_bytes = Some(60_000);
    out.chip_model = Some(ChipModel::Esp32C3);
}

struct Rig {
    runtime: Runtime<'static>,
    clock: VirtualClock,
    pipe: SimPipe,
    display: Box<CaptureDisplay>,
    blob: AssetBlob<'static>,
    seq: u16,
    received: Vec<Message>,
}

impl Rig {
    fn new(with_probe: bool) -> Self {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            device_id: [0x42; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 3,
                patch: 0,
            },
            capabilities: Capabilities::DEVICE_INTERNALS_V1,
        };
        let mut runtime = Runtime::new(identity, RuntimeConfig::default());
        if with_probe {
            runtime.set_chip_probe(probe);
        }
        let mut rig = Self {
            runtime,
            clock: VirtualClock::new(),
            pipe: SimPipe::new(),
            display: Box::new(CaptureDisplay::new()),
            blob: AssetBlob::parse(bytes).expect("valid blob"),
            seq: 0,
            received: Vec::new(),
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

    fn connect(&mut self, nonce: Nonce, caps: Capabilities) {
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
    }

    fn step(&mut self) {
        let mut input = NoInput;
        self.runtime.step(
            &self.clock,
            &mut self.pipe,
            &mut input,
            self.display.as_mut(),
            &self.blob,
        );
        let bytes = self.pipe.host_recv();
        let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
        for packet in bytes.split(|&b| b == 0).filter(|p| !p.is_empty()) {
            if let Ok((_, message)) = decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
                self.received.push(message);
            }
        }
    }

    fn run(&mut self, ms: u32) {
        for _ in 0..ms / 10 {
            self.clock.advance(10);
            self.step();
        }
    }

    fn reports(&self) -> Vec<InternalsReport> {
        self.received
            .iter()
            .filter_map(|m| match m {
                Message::Internals(report) => Some(*report),
                _ => None,
            })
            .collect()
    }

    fn ask(&mut self, session: Nonce) {
        self.send(&Message::InternalsRequest(InternalsRequest { session }));
        self.step();
    }
}

#[test]
fn a_negotiated_request_gets_one_snapshot_with_what_the_device_measured() {
    let mut rig = Rig::new(true);
    rig.connect(7, Capabilities::DEVICE_INTERNALS_V1);
    rig.run(2000);
    rig.ask(7);

    let reports = rig.reports();
    assert_eq!(reports.len(), 1, "one request, one snapshot");
    assert_eq!(reports[0].session, 7);
    let internals = reports[0].internals;

    // From the board probe, and the cap that goes with a known blob size.
    assert_eq!(internals.app_image_bytes, Some(700_000));
    assert_eq!(internals.asset_blob_bytes, Some(60_000));
    assert_eq!(internals.asset_blob_cap_bytes, Some(ASSET_BLOB_CAP_BYTES));
    assert_eq!(internals.chip_model, Some(ChipModel::Esp32C3));
    // The probe could not read these, so they are unavailable, never zero.
    assert_eq!(internals.temperature_decicelsius, None);
    assert_eq!(internals.flash_total_bytes, None);
    assert_eq!(internals.reset_reason, None);

    // From the run loop and its counters.
    assert!(internals.uptime_ms.is_some_and(|ms| ms >= 2000));
    assert!(internals.busy_percent.is_some_and(|p| p <= 100));
    assert!(internals.fps.is_some_and(|fps| fps > 0));
    assert!(internals.compose_ms.is_some() && internals.flush_ms.is_some());
    assert_eq!(internals.edge_queue_high_water, Some(9));
    assert_eq!(internals.dropped_edges, Some(3));
    assert_eq!(internals.invalid_transitions, Some(0));
    assert!(internals.frames_in.is_some_and(|n| n >= 3));
    assert!(internals.frames_out.is_some_and(|n| n >= 1));
    assert_eq!(internals.frames_rejected, Some(0));
}

#[test]
fn without_a_board_probe_the_chip_figures_are_unavailable() {
    let mut rig = Rig::new(false);
    rig.connect(7, Capabilities::DEVICE_INTERNALS_V1);
    rig.ask(7);
    let internals = rig.reports()[0].internals;
    assert_eq!(internals.app_image_bytes, None);
    assert_eq!(internals.asset_blob_bytes, None);
    assert_eq!(internals.asset_blob_cap_bytes, None);
    assert_eq!(internals.chip_model, None);
    assert!(internals.uptime_ms.is_some());
}

#[test]
fn a_request_without_the_negotiated_capability_is_ignored() {
    let mut rig = Rig::new(true);
    rig.connect(7, Capabilities::NONE);
    rig.ask(7);
    rig.run(100);
    assert!(rig.reports().is_empty());
}

#[test]
fn a_request_for_another_session_is_ignored() {
    let mut rig = Rig::new(true);
    rig.connect(7, Capabilities::DEVICE_INTERNALS_V1);
    rig.ask(8);
    rig.run(100);
    assert!(rig.reports().is_empty());
}

#[test]
fn every_request_is_answered_once() {
    let mut rig = Rig::new(true);
    rig.connect(7, Capabilities::DEVICE_INTERNALS_V1);
    for _ in 0..3 {
        rig.ask(7);
    }
    let reports = rig.reports();
    assert_eq!(reports.len(), 3);
    let frames_out: Vec<u32> = reports
        .iter()
        .filter_map(|r| r.internals.frames_out)
        .collect();
    assert!(frames_out.windows(2).all(|w| w[0] < w[1]), "{frames_out:?}");
}
