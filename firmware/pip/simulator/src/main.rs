mod input_scenarios;
mod io_scenarios;
mod modelled;
mod probe;
mod timing;

use std::{
    cell::RefCell,
    convert::Infallible,
    io::{self, BufRead, Write},
    rc::Rc,
};

use kivori_assets::AssetBlob;
use kivori_firmware::{
    identity::{BootCheck, ProductIdentity},
    ports::{InputSource, Transport},
    profile::physical_st7789::CAPABILITIES as CAPS,
    proto::DeviceIdentity,
    runtime::{Runtime, RuntimeConfig, Tick},
    sim::{CaptureDisplay, SimPipe, PIPE_CAPACITY},
    tx_buffer::TxBuffered,
};
use kivori_model::input::InputLevels;
use kivori_protocol::FirmwareVersion;
use modelled::{Gate, ModelledSimulator};
use serde::{Deserialize, Serialize};
use timing::{SharedDisplay, SimClock};

include!(concat!(env!("OUT_DIR"), "/identity.rs"));
const MAX_LINE: usize = 400_000;
const MAX_EDGES: usize = 128;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edge {
    at_ms: u32,
    levels: u8,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Fault {
    #[default]
    Normal,
    Silent,
    WrongMajor,
    UnknownProduct,
    UnknownBoard,
    Exit,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    now_ms: u32,
    wire: Vec<u8>,
    edges: Vec<Edge>,
    capture: bool,
    #[serde(default)]
    fault: Fault,
}

#[derive(Serialize)]
struct Response {
    wire: Vec<u8>,
    frame: Option<Vec<u16>>,
    brightness: Option<u8>,
    reboot: bool,
    /// The firmware companion state after this tick, so harnesses can tell Sleeping from Offline
    /// without inferring it from pixels. Development-only; not a wire-schema field.
    state: &'static str,
}

#[derive(Default)]
struct BufferedInput {
    levels: InputLevels,
    edges: Vec<Edge>,
    high_water: u32,
    /// Modelled timing only: the host-time gate that supplies edges as device time reaches them.
    gate: Option<Rc<RefCell<Gate>>>,
}

fn levels(mask: u8) -> InputLevels {
    InputLevels {
        a: mask & 1 != 0,
        b: mask & 2 != 0,
        sw: mask & 4 != 0,
        keys: [mask & 8 != 0, mask & 16 != 0, mask & 32 != 0],
    }
}

impl InputSource for BufferedInput {
    fn sample(&mut self) -> InputLevels {
        self.levels
    }
    fn drain(&mut self, now_ms: u32, f: &mut dyn FnMut(InputLevels, u32)) {
        // Every service point of the runtime (the start of a tick, between tile rows and between
        // display windows) comes through here, so this is where modelled time lets the host catch up
        // and where input stamped later than `now_ms` stays queued, as on the device.
        if let Some(gate) = &self.gate {
            self.edges.extend(gate.borrow_mut().service());
            self.high_water = self.high_water.max(self.edges.len() as u32);
        }
        let due = self.edges.iter().take_while(|e| e.at_ms <= now_ms).count();
        for edge in self.edges.drain(..due) {
            self.levels = levels(edge.levels);
            f(self.levels, edge.at_ms);
        }
        f(self.levels, now_ms);
    }
    fn edge_high_water(&self) -> u32 {
        self.high_water
    }
}

/// The host side of the in-memory pipe, shared so the modelled device can deliver bytes and collect
/// replies while the runtime owns the transport.
struct SharedPipe(Rc<RefCell<SimPipe>>);

impl Transport for SharedPipe {
    type Error = Infallible;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.borrow_mut().read(buf)
    }
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.0.borrow_mut().write(buf)
    }
}

/// Handles the device and the process loop share.
#[derive(Clone)]
struct Shared {
    clock: Rc<SimClock>,
    link: Rc<RefCell<SimPipe>>,
    capture: Rc<RefCell<CaptureDisplay>>,
}

/// What one request returns before faults are applied.
struct Report {
    wire: Vec<u8>,
    frame: Option<Vec<u16>>,
    brightness: Option<u8>,
    reboot: bool,
    state: &'static str,
}

impl Shared {
    fn report(&self, reboot: bool, state: &'static str, capture: bool) -> Report {
        let display = self.capture.borrow();
        Report {
            wire: self.link.borrow_mut().host_recv().to_vec(),
            frame: capture.then(|| display.frame().iter().map(|p| p.raw()).collect()),
            brightness: display.brightness,
            reboot,
            state,
        }
    }
}

/// Rejects a request the runner will not run, before any state changes.
fn validate(now: u32, rebooted: bool, request: &Request) -> Result<(), String> {
    if request.fault == Fault::Exit {
        return Err("exit fault is handled by the process loop".into());
    }
    if rebooted {
        return Err("reboot requested; restart the simulator process".into());
    }
    if request.now_ms < now || request.wire.len() > PIPE_CAPACITY || request.edges.len() > MAX_EDGES
    {
        return Err("request exceeds time, wire or edge limits".into());
    }
    let mut previous = now;
    for edge in &request.edges {
        if edge.levels & !63 != 0 || edge.at_ms < previous || edge.at_ms > request.now_ms {
            return Err("invalid or unordered input edge".into());
        }
        previous = edge.at_ms;
    }
    Ok(())
}

/// Applies the request's reply fault to what the firmware produced.
fn finish(fault: Fault, report: Report) -> Result<Response, String> {
    let mut wire = report.wire;
    match fault {
        Fault::Silent => wire.clear(),
        Fault::WrongMajor => {
            wire = wrong_major_ack(&wire)?;
        }
        _ => {}
    }
    Ok(Response {
        wire,
        frame: report.frame,
        brightness: report.brightness,
        reboot: report.reboot,
        state: report.state,
    })
}

/// How the runner models time: `Ideal` freezes the clock at the request's time for the whole tick
/// (every frame costs 0 ms, one request is one tick, as the deterministic scenarios and goldens
/// need); `Modelled` runs the device on its own clock and charges the ESP32-C3's compose and flush
/// time (docs/virtual-pip.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Timing {
    Ideal,
    Modelled,
}

struct Simulator {
    runtime: Runtime<'static>,
    shared: Shared,
    pipe: TxBuffered<SharedPipe>,
    display: SharedDisplay,
    input: BufferedInput,
    blob: AssetBlob<'static>,
    now: u32,
    rebooted: bool,
}

impl Simulator {
    /// The zero-time simulator: one request is one runtime tick at the request's time.
    fn new() -> Self {
        Self::assemble(Timing::Ideal)
    }

    fn assemble(timing: Timing) -> Self {
        let shared = Shared {
            clock: Rc::new(match timing {
                Timing::Ideal => SimClock::ideal(),
                Timing::Modelled => SimClock::modelled(),
            }),
            link: Rc::new(RefCell::new(SimPipe::new())),
            capture: Rc::new(RefCell::new(CaptureDisplay::new())),
        };
        let mut runtime = Runtime::with_frame_buffer(
            DeviceIdentity {
                // Deliberately recognizable, deterministic virtual identity; never a chip identity.
                device_id: *b"VIRTUAL-PIP-0001",
                firmware_version: VERSION,
                capabilities: CAPS,
            },
            RuntimeConfig::default(),
            Box::leak(Box::new(
                [kivori_model::Rgb565::from_raw(0); kivori_firmware::render::FRAME_PIXELS],
            )),
        );
        runtime.set_chip_probe(probe::probe);
        Self {
            runtime,
            pipe: TxBuffered::new(SharedPipe(shared.link.clone())),
            display: SharedDisplay::new(shared.capture.clone(), shared.clock.clone()),
            shared,
            input: BufferedInput::default(),
            blob: AssetBlob::parse(include_bytes!(concat!(env!("OUT_DIR"), "/pip.assets")))
                .expect("build-time asset compiler produces a valid blob"),
            now: 0,
            rebooted: false,
        }
    }

    /// One production runtime tick at the clock's current time, with `fault`'s boot identity.
    fn run_tick(&mut self, fault: Fault) -> Tick {
        let mut identity = ProductIdentity::BUILD;
        match fault {
            Fault::UnknownProduct => identity.product = u16::MAX,
            Fault::UnknownBoard => identity.board = u16::MAX,
            _ => {}
        }
        self.runtime.set_boot_check(BootCheck::Run(identity));
        self.runtime.step(
            &*self.shared.clock,
            &mut self.pipe,
            &mut self.input,
            &mut self.display,
            &self.blob,
        )
    }

    fn step(&mut self, request: Request) -> Result<Response, String> {
        validate(self.now, self.rebooted, &request)?;
        self.shared
            .link
            .borrow_mut()
            .host_send(&request.wire)
            .map_err(|()| "pending firmware input exceeds pipe capacity")?;
        self.input.high_water = self.input.high_water.max(request.edges.len() as u32);
        self.input.edges = request.edges;
        self.now = request.now_ms;
        self.shared.clock.set_ms(self.now);
        let tick = self.run_tick(request.fault);
        self.rebooted = tick.reboot;
        finish(
            request.fault,
            self.shared.report(
                tick.reboot,
                state_token(self.runtime.state()),
                request.capture,
            ),
        )
    }
}

const fn state_token(state: kivori_model::CompanionState) -> &'static str {
    use kivori_model::CompanionState as S;
    match state {
        S::Booting => "booting",
        S::Idle => "idle",
        S::Happy => "happy",
        S::Busy => "busy",
        S::Sleeping => "sleeping",
        S::Offline => "offline",
    }
}

// Change only the actual HelloAck header; preserve its firmware-produced payload and sequence.
fn wrong_major_ack(wire: &[u8]) -> Result<Vec<u8>, String> {
    use kivori_protocol::{
        decode_message,
        frame::{decode_frame, encode_frame},
        Message, PROTOCOL_MAJOR,
    };
    let mut result = Vec::with_capacity(wire.len());
    let mut scratch = Default::default();
    for packet in wire
        .split(|&byte| byte == 0)
        .filter(|packet| !packet.is_empty())
    {
        let (_, message) = decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR])
            .map_err(|e| format!("firmware response decode: {e:?}"))?;
        if matches!(message, Message::HelloAck(_)) {
            let (mut header, payload) = decode_frame(packet, &mut scratch)
                .map_err(|e| format!("firmware frame decode: {e:?}"))?;
            header.version.major = PROTOCOL_MAJOR.wrapping_add(1);
            let mut encoded = Default::default();
            encode_frame(&header, payload, &mut encoded)
                .map_err(|e| format!("fault frame encode: {e:?}"))?;
            result.extend_from_slice(&encoded);
        } else {
            result.extend_from_slice(packet);
            result.push(0);
        }
    }
    Ok(result)
}

// Read a bounded record without allocating an unbounded line before checking its size.
fn read_record(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut record = Vec::new();
    loop {
        let chunk = reader.fill_buf().map_err(|e| e.to_string())?;
        if chunk.is_empty() {
            return if record.is_empty() {
                Ok(None)
            } else {
                Err("truncated JSON record".into())
            };
        }
        let newline = chunk.iter().position(|b| *b == b'\n');
        let n = newline.unwrap_or(chunk.len());
        if record.len() + n > MAX_LINE {
            return Err("JSON record exceeds line limit".into());
        }
        record.extend_from_slice(&chunk[..n]);
        reader.consume(n + usize::from(newline.is_some()));
        if newline.is_some() {
            return Ok(Some(record));
        }
    }
}

/// The device behind the process loop, in either timing.
enum Device {
    Ideal(Box<Simulator>),
    Modelled(ModelledSimulator),
}

impl Device {
    fn new(timing: Timing) -> Self {
        match timing {
            Timing::Ideal => Self::Ideal(Box::new(Simulator::new())),
            Timing::Modelled => Self::Modelled(ModelledSimulator::spawn()),
        }
    }

    fn step(&mut self, request: Request) -> Result<Response, String> {
        match self {
            Self::Ideal(sim) => sim.step(request),
            Self::Modelled(sim) => sim.step(request),
        }
    }
}

fn run(reader: &mut impl BufRead, writer: &mut impl Write, timing: Timing) -> Result<(), String> {
    let mut sim = Device::new(timing);
    while let Some(record) = read_record(reader)? {
        let request: Request = serde_json::from_slice(&record).map_err(|e| e.to_string())?;
        if request.fault == Fault::Exit {
            return Ok(());
        }
        let response = sim.step(request)?;
        let line = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
        if line.len() > MAX_LINE {
            return Err("response exceeds line limit".into());
        }
        writer
            .write_all(&line)
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    let result = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => run(&mut io::stdin().lock(), &mut io::stdout().lock(), Timing::Modelled),
        [flag] if flag == "--ideal-time" => {
            run(&mut io::stdin().lock(), &mut io::stdout().lock(), Timing::Ideal)
        }
        [flag] if flag == "--input-smoke" => input_scenarios::run(),
        [flag] if flag == "--io-smoke" => io_scenarios::run(),
        [flag, seed] if flag == "--replay" => input_scenarios::replay(seed),
        _ => Err(
            "usage: kivori-pip-simulator [--ideal-time | --input-smoke | --io-smoke | --replay <17|29>]"
                .into(),
        ),
    };
    if let Err(error) = result {
        eprintln!("Pip simulator: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn request(now_ms: u32) -> Request {
        Request {
            now_ms,
            wire: vec![],
            edges: vec![],
            capture: false,
            fault: Fault::Normal,
        }
    }

    fn message_request(message: kivori_protocol::Message, now: u32) -> Request {
        let mut wire = Default::default();
        kivori_protocol::encode_message(
            &message,
            kivori_model::ProtocolVersion::new(
                kivori_protocol::PROTOCOL_MAJOR,
                kivori_protocol::PROTOCOL_MINOR,
            ),
            now as u16,
            &mut wire,
        )
        .unwrap();
        let mut req = request(now);
        req.wire = wire.to_vec();
        req
    }

    fn hello() -> Request {
        message_request(
            kivori_protocol::Message::Hello(kivori_protocol::Hello {
                desktop_version: VERSION,
                desktop_caps: CAPS,
                nonce: 42,
            }),
            0,
        )
    }

    fn ready(now: u32) -> Request {
        message_request(
            kivori_protocol::Message::Ready(kivori_protocol::Ready {
                negotiated_minor: kivori_protocol::PROTOCOL_MINOR,
                negotiated_caps: CAPS,
            }),
            now,
        )
    }

    #[test]
    fn fault_contract_is_optional_and_rejects_unknown_values() {
        let req: Request =
            serde_json::from_str(r#"{"now_ms":0,"wire":[],"edges":[],"capture":false}"#).unwrap();
        assert_eq!(req.fault, Fault::Normal);
        for (value, expected) in [
            ("normal", Fault::Normal),
            ("silent", Fault::Silent),
            ("wrong-major", Fault::WrongMajor),
            ("unknown-product", Fault::UnknownProduct),
            ("unknown-board", Fault::UnknownBoard),
            ("exit", Fault::Exit),
        ] {
            assert_eq!(
                serde_json::from_value::<Fault>(serde_json::json!(value)).unwrap(),
                expected
            );
        }
        assert!(serde_json::from_value::<Fault>(serde_json::json!("anything")).is_err());
    }

    #[test]
    fn identity_faults_use_runtime_identity_and_normal_restores_build() {
        use kivori_protocol::{decode_message, Message, PROTOCOL_MAJOR};
        for fault in [Fault::UnknownProduct, Fault::UnknownBoard, Fault::Normal] {
            let expected = match fault {
                Fault::UnknownProduct => (u16::MAX, ProductIdentity::BUILD.board),
                Fault::UnknownBoard => (ProductIdentity::BUILD.product, u16::MAX),
                _ => (ProductIdentity::BUILD.product, ProductIdentity::BUILD.board),
            };
            let mut sim = Simulator::new();
            // Exercise restoration too: a normal tick must replace a previous injected identity.
            let mut initial = request(0);
            initial.fault = Fault::UnknownProduct;
            sim.step(initial).unwrap();
            let mut h = hello();
            h.fault = fault;
            let response = sim.step(h).unwrap();
            assert!(!response.wire.is_empty());
            let mut r = ready(1);
            r.fault = match expected {
                (u16::MAX, _) => Fault::UnknownProduct,
                (_, u16::MAX) => Fault::UnknownBoard,
                _ => Fault::Normal,
            };
            let response = sim.step(r).unwrap();
            let mut scratch = Default::default();
            let identity = response
                .wire
                .split(|&b| b == 0)
                .filter(|p| !p.is_empty())
                .find_map(|packet| {
                    match decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR])
                        .unwrap()
                        .1
                    {
                        Message::Identity(identity) => Some(identity),
                        _ => None,
                    }
                })
                .expect("Ready emits actual Runtime identity");
            assert_eq!((identity.product, identity.board), expected);
            assert_eq!(identity.source, kivori_protocol::IdentitySource::Build);
        }
    }

    #[test]
    fn wrong_major_changes_only_real_ack_header_and_is_rejected() {
        use kivori_protocol::{
            decode_message, frame::decode_frame, Message, ProtoError, PROTOCOL_MAJOR,
        };
        let normal = Simulator::new().step(hello()).unwrap();
        let mut bad = hello();
        bad.fault = Fault::WrongMajor;
        let altered = Simulator::new().step(bad).unwrap();
        let mut old_scratch = Default::default();
        let mut new_scratch = Default::default();
        assert_eq!(
            normal.wire.iter().filter(|&&b| b == 0).count(),
            altered.wire.iter().filter(|&&b| b == 0).count()
        );
        let mut ack_count = 0;
        for (old, new) in normal
            .wire
            .split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .zip(altered.wire.split(|&b| b == 0).filter(|p| !p.is_empty()))
        {
            let (_, message) = decode_message(old, &mut old_scratch, &[PROTOCOL_MAJOR]).unwrap();
            if matches!(message, Message::HelloAck(_)) {
                ack_count += 1;
                assert_eq!(
                    decode_message(new, &mut new_scratch, &[PROTOCOL_MAJOR]),
                    Err(ProtoError::UnsupportedVersion)
                );
                let (a, payload_a) = decode_frame(old, &mut old_scratch).unwrap();
                let (b, payload_b) = decode_frame(new, &mut new_scratch).unwrap();
                assert_eq!(b.version.major, PROTOCOL_MAJOR.wrapping_add(1));
                assert_eq!(a.seq, b.seq);
                assert_eq!(a.version.minor, b.version.minor);
                assert_eq!(payload_a, payload_b);
            } else {
                assert_eq!(old, new);
            }
        }
        assert_eq!(ack_count, 1);
    }

    #[test]
    fn silent_drops_real_responses_but_continues_rendering_and_recovers() {
        let mut sim = Simulator::new();
        let mut h = hello();
        h.fault = Fault::Silent;
        h.capture = true;
        let response = sim.step(h).unwrap();
        assert!(response.wire.is_empty());
        assert!(response.frame.unwrap().iter().any(|&pixel| pixel != 0));
        let response = sim.step(ready(1)).unwrap();
        assert!(
            !response.wire.is_empty(),
            "normal restores outbound traffic"
        );
    }

    #[test]
    fn exit_ends_process_loop_without_response_or_consuming_next_record() {
        let mut input = Cursor::new(b"{\"now_ms\":0,\"wire\":[],\"edges\":[],\"capture\":false,\"fault\":\"exit\"}\ninvalid\n");
        let mut output = Vec::new();
        run(&mut input, &mut output, Timing::Ideal).unwrap();
        assert!(output.is_empty());
        assert_eq!(read_record(&mut input).unwrap().unwrap(), b"invalid");
        let mut req = request(0);
        req.fault = Fault::Exit;
        assert!(Simulator::new().step(req).is_err());
    }

    #[test]
    fn buffered_edges_preserve_timestamps_and_final_levels() {
        let mut source = BufferedInput {
            edges: vec![
                Edge {
                    at_ms: 2,
                    levels: 5,
                },
                Edge {
                    at_ms: 7,
                    levels: 34,
                },
            ],
            ..Default::default()
        };
        let mut seen = vec![];
        source.drain(10, &mut |level, at| seen.push((level, at)));
        assert_eq!(
            seen,
            vec![(levels(5), 2), (levels(34), 7), (levels(34), 10)]
        );
        assert!(source.edges.is_empty());
    }

    #[test]
    fn records_are_bounded_complete_and_eof_is_clean() {
        assert_eq!(
            read_record(&mut Cursor::new(b"ok\n")).unwrap(),
            Some(b"ok".to_vec())
        );
        assert!(read_record(&mut Cursor::new(b"partial")).is_err());
        assert!(read_record(&mut Cursor::new(vec![b'x'; MAX_LINE + 1])).is_err());
        assert!(read_record(&mut Cursor::new(vec![b'x'; MAX_LINE])).is_err()); // no newline
        let mut exact = vec![b'x'; MAX_LINE];
        exact.push(b'\n');
        assert_eq!(
            read_record(&mut Cursor::new(exact)).unwrap().unwrap().len(),
            MAX_LINE
        );
        assert!(read_record(&mut Cursor::new(b"")).unwrap().is_none());
    }

    #[test]
    fn invalid_requests_do_not_advance_state() {
        let mut sim = Simulator::new();
        sim.step(request(10)).unwrap();
        assert!(sim.step(request(9)).is_err());
        for edges in [
            vec![Edge {
                at_ms: 9,
                levels: 0,
            }],
            vec![Edge {
                at_ms: 11,
                levels: 64,
            }],
            vec![
                Edge {
                    at_ms: 12,
                    levels: 0,
                },
                Edge {
                    at_ms: 11,
                    levels: 0,
                },
            ],
            vec![Edge {
                at_ms: 21,
                levels: 0,
            }],
        ] {
            let mut bad = request(20);
            bad.edges = edges;
            assert!(sim.step(bad).is_err());
            assert_eq!(sim.now, 10);
        }
        let mut bad = request(20);
        bad.wire = vec![0; PIPE_CAPACITY + 1];
        assert!(sim.step(bad).is_err());
        let mut bad = request(20);
        bad.edges = (0..129)
            .map(|_| Edge {
                at_ms: 10,
                levels: 0,
            })
            .collect();
        assert!(sim.step(bad).is_err());
        assert!(sim.step(request(10)).is_ok());
    }

    #[test]
    fn malformed_json_fails_without_stdout() {
        let mut output = vec![];
        assert!(run(&mut Cursor::new(b"{}\n"), &mut output, Timing::Ideal).is_err());
        assert!(output.is_empty());
    }

    #[test]
    fn three_normalized_raw_cycles_emit_three_clockwise_detents() {
        use kivori_model::{input::Direction, ProtocolVersion};
        use kivori_protocol::{
            decode_message, encode_message, Hello, InputKind, Message, Ready, PROTOCOL_MAJOR,
            PROTOCOL_MINOR,
        };
        fn send(sim: &mut Simulator, message: Message, seq: u16, now: u32) {
            let mut wire = Default::default();
            encode_message(
                &message,
                ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
                seq,
                &mut wire,
            )
            .unwrap();
            let mut req = request(now);
            req.wire = wire.to_vec();
            sim.step(req).unwrap();
        }
        let mut sim = Simulator::new();
        send(
            &mut sim,
            Message::Hello(Hello {
                desktop_version: VERSION,
                desktop_caps: CAPS,
                nonce: 42,
            }),
            0,
            0,
        );
        send(
            &mut sim,
            Message::Ready(Ready {
                negotiated_minor: PROTOCOL_MINOR,
                negotiated_caps: CAPS,
            }),
            1,
            1,
        );
        sim.step(request(33)).unwrap();
        sim.step(request(500)).unwrap();
        let mut detents = vec![];
        let mut scratch = Default::default();
        for start in [1000, 1100, 1200] {
            let mut req = request(start + 4);
            // Physical adapter normalizes low-active pins before InputSource. Rest is 00.
            req.edges = [2, 3, 1, 0]
                .into_iter()
                .enumerate()
                .map(|(i, levels)| Edge {
                    at_ms: start + i as u32,
                    levels,
                })
                .collect();
            let response = sim.step(req).unwrap();
            for packet in response
                .wire
                .split(|&byte| byte == 0)
                .filter(|packet| !packet.is_empty())
            {
                if let Ok((_, Message::InputEvent(event))) =
                    decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR])
                {
                    if let InputKind::Detent(direction) = event.kind {
                        detents.push(direction);
                    }
                }
            }
        }
        assert_eq!(detents, vec![Direction::Cw; 3]);
    }

    #[test]
    fn real_runtime_handshake_and_frame_are_returned() {
        use kivori_model::Capabilities;
        assert!(CAPS.contains(
            Capabilities::MIC_STATE_V1
                .union(Capabilities::PRODUCT_IDENTITY_V1)
                .union(Capabilities::BUILD_INFO_V1)
                .union(Capabilities::DEVICE_INTERNALS_V1)
                .union(Capabilities::QUIET_ACK_V1)
                .union(Capabilities::IDLE_LIFE_V1)
        ));
        use kivori_model::ProtocolVersion;
        use kivori_protocol::{
            decode_message, encode_message, Hello, Message, PROTOCOL_MAJOR, PROTOCOL_MINOR,
        };
        // A wire frame buffer is inferred through this helper's generic concrete type.
        let mut wire = Default::default();
        encode_message(
            &Message::Hello(Hello {
                desktop_version: VERSION,
                desktop_caps: CAPS,
                nonce: 42,
            }),
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            0,
            &mut wire,
        )
        .unwrap();
        let mut sim = Simulator::new();
        let mut req = request(33);
        req.wire = wire.to_vec();
        req.capture = true;
        let response = sim.step(req).unwrap();
        let mut scratch = Default::default();
        let messages: Vec<_> = response
            .wire
            .split(|&byte| byte == 0)
            .filter(|packet| !packet.is_empty())
            .filter_map(|packet| {
                decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR])
                    .ok()
                    .map(|(_, message)| message)
            })
            .collect();
        assert!(messages.iter().any(|message| matches!(message, Message::HelloAck(ack) if ack.nonce_echo == 42 && ack.device_caps == CAPS)));
        wire.clear();
        encode_message(
            &Message::Ready(kivori_protocol::Ready {
                negotiated_minor: PROTOCOL_MINOR,
                negotiated_caps: CAPS,
            }),
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            1,
            &mut wire,
        )
        .unwrap();
        let mut ready = request(34);
        ready.wire = wire.to_vec();
        let identity_response = sim.step(ready).unwrap();
        // The virtual Pip builds from the same source, so it reports the same build id.
        let build = identity_response
            .wire
            .split(|&byte| byte == 0)
            .filter(|packet| !packet.is_empty())
            .find_map(
                |packet| match decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
                    Ok((_, Message::BuildInfo(build))) => Some(build),
                    _ => None,
                },
            )
            .expect("Ready returns the build id");
        assert_eq!(build.session, 42);
        assert_eq!(
            build.commit,
            kivori_firmware::identity::BuildId::BUILD.commit
        );
        assert_eq!(build.dirty, kivori_firmware::identity::BuildId::BUILD.dirty);
        let identity = identity_response
            .wire
            .split(|&byte| byte == 0)
            .filter(|packet| !packet.is_empty())
            .find_map(
                |packet| match decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
                    Ok((_, Message::Identity(identity))) => Some(identity),
                    _ => None,
                },
            )
            .expect("Ready returns the actual product identity");
        assert_eq!(
            (identity.product, identity.board),
            (
                kivori_firmware::identity::BUILD_PRODUCT,
                kivori_firmware::identity::BUILD_BOARD
            )
        );
        assert_eq!(identity.source, kivori_protocol::IdentitySource::Build);
        assert_eq!(identity.hw_rev, None);
        let frame = response.frame.unwrap();
        assert_eq!(frame.len(), 240 * 240);
        assert!(frame.iter().any(|&pixel| pixel != 0));
        assert!(
            serde_json::to_vec(&Response {
                wire: vec![255; PIPE_CAPACITY],
                frame: Some(vec![65535; 240 * 240]),
                brightness: Some(5),
                reboot: false,
                state: "offline",
            })
            .unwrap()
            .len()
                <= MAX_LINE
        );
    }
}
