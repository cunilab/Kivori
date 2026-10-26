use std::io::{self, BufRead, Write};

use kivori_assets::AssetBlob;
use kivori_firmware::{
    ports::InputSource,
    profile::physical_st7789::CAPABILITIES as CAPS,
    proto::DeviceIdentity,
    runtime::{Runtime, RuntimeConfig},
    sim::{CaptureDisplay, SimPipe, VirtualClock, PIPE_CAPACITY},
};
use kivori_model::input::InputLevels;
use kivori_protocol::FirmwareVersion;
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/identity.rs"));
const MAX_LINE: usize = 400_000;
const MAX_EDGES: usize = 128;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edge {
    at_ms: u32,
    levels: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    now_ms: u32,
    wire: Vec<u8>,
    edges: Vec<Edge>,
    capture: bool,
}

#[derive(Serialize)]
struct Response {
    wire: Vec<u8>,
    frame: Option<Vec<u16>>,
    brightness: Option<u8>,
    reboot: bool,
}

#[derive(Default)]
struct BufferedInput {
    levels: InputLevels,
    edges: Vec<Edge>,
    high_water: u32,
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
        for edge in self.edges.drain(..) {
            self.levels = levels(edge.levels);
            f(self.levels, edge.at_ms);
        }
        f(self.levels, now_ms);
    }
    fn edge_high_water(&self) -> u32 {
        self.high_water
    }
}

struct Simulator {
    runtime: Runtime<'static>,
    clock: VirtualClock,
    pipe: SimPipe,
    display: Box<CaptureDisplay>,
    input: BufferedInput,
    now: u32,
    rebooted: bool,
}

impl Simulator {
    fn new() -> Self {
        Self {
            runtime: Runtime::new(
                DeviceIdentity {
                    // Deliberately recognizable, deterministic virtual identity; never a chip identity.
                    device_id: *b"VIRTUAL-PIP-0001",
                    firmware_version: VERSION,
                    capabilities: CAPS,
                },
                RuntimeConfig::default(),
            ),
            clock: VirtualClock::new(),
            pipe: SimPipe::new(),
            display: Box::new(CaptureDisplay::new()),
            input: BufferedInput::default(),
            now: 0,
            rebooted: false,
        }
    }

    fn step(&mut self, request: Request) -> Result<Response, String> {
        if self.rebooted {
            return Err("reboot requested; restart the simulator process".into());
        }
        if request.now_ms < self.now
            || request.wire.len() > PIPE_CAPACITY
            || request.edges.len() > MAX_EDGES
        {
            return Err("request exceeds time, wire or edge limits".into());
        }
        let mut previous = self.now;
        for edge in &request.edges {
            if edge.levels & !63 != 0 || edge.at_ms < previous || edge.at_ms > request.now_ms {
                return Err("invalid or unordered input edge".into());
            }
            previous = edge.at_ms;
        }
        self.pipe
            .host_send(&request.wire)
            .map_err(|()| "pending firmware input exceeds pipe capacity")?;
        self.input.high_water = self.input.high_water.max(request.edges.len() as u32);
        self.input.edges = request.edges;
        self.now = request.now_ms;
        self.clock.set(self.now);
        let blob = AssetBlob::parse(include_bytes!(concat!(env!("OUT_DIR"), "/pip.assets")))
            .expect("build-time asset compiler produces a valid blob");
        let tick = self.runtime.step(
            &self.clock,
            &mut self.pipe,
            &mut self.input,
            &mut *self.display,
            &blob,
        );
        self.rebooted = tick.reboot;
        Ok(Response {
            wire: self.pipe.host_recv().to_vec(),
            frame: request
                .capture
                .then(|| self.display.frame().iter().map(|p| p.raw()).collect()),
            brightness: self.display.brightness,
            reboot: tick.reboot,
        })
    }
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

fn run(reader: &mut impl BufRead, writer: &mut impl Write) -> Result<(), String> {
    let mut sim = Simulator::new();
    while let Some(record) = read_record(reader)? {
        let request = serde_json::from_slice(&record).map_err(|e| e.to_string())?;
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
    if let Err(error) = run(&mut io::stdin().lock(), &mut io::stdout().lock()) {
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
        }
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
        assert!(run(&mut Cursor::new(b"{}\n"), &mut output).is_err());
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
                reboot: false
            })
            .unwrap()
            .len()
                <= MAX_LINE
        );
    }
}
