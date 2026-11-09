//! The production runtime at RP2040-Zero speed: every tick charges what the chip is estimated to
//! spend, and the protocol's deadlines are checked in virtual time. The estimates are the v2
//! virtual board's (`simulator/src/board.rs`; to be replaced by measurements, issue #179): a
//! frame composes in 79 ms x 1.7 and the panel is fed over SPI1 at 15.625 MHz plus 100 ns of
//! staging per pixel. At those rates one pass of the loop takes 150 to 250 ms against 33 ms on the
//! C3, so everything the device owes the host per tick (heartbeat reply, input events, reading the
//! inbound bytes) is checked against the host's own allowance (2 s per heartbeat, 3 missed ends it).
//! Host simulation only.
#![cfg(feature = "host-sim")]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::rc::Rc;

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::{Clock, DisplaySink, InputSource, Transport};
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::render::TILE_ROWS;
use kivori_firmware::runtime::{Runtime, RuntimeConfig, HOST_SILENCE_MS};
use kivori_firmware::sim::SimPipe;
use kivori_firmware::tx_buffer::TxBuffered;
use kivori_model::input::{Direction, InputLevels};
use kivori_model::presentation::{PrimaryState, ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Capabilities, ElapsedMs, ProtocolVersion, Rect, Rgb565};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, InputKind, Message, Ping, Presentation,
    Ready, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const CAPS: Capabilities = Capabilities::PHYSICAL_INPUT_V1
    .union(Capabilities::PRESENTATION_V1)
    .union(Capabilities::BUTTON_INPUT_V1);
const NONCE: u32 = 0xC0DE_2040;

/// The dearest scene's compose time on the C3 (the frame-cost fixture's `connected_desk`, 79 ms)
/// scaled by the v2 estimate (x1.7), spread evenly over the tile rows.
const COMPOSE_US_PER_FRAME: u64 = 79_000 * 17 / 10;
const SPI_HZ: u64 = 15_625_000;
const STAGING_NS_PER_PIXEL: u64 = 100;
/// The host gives up on a device that has not answered a heartbeat for this long.
const HOST_ALLOWANCE_MS: u32 = 2_000;

/// The chip's time: the runtime's clock, charged per composed row and per panel window.
struct Chip {
    us: Cell<u64>,
}

impl Chip {
    fn spend(&self, us: u64) {
        self.us.set(self.us.get() + us);
    }
}

impl Clock for Chip {
    fn now_ms(&self) -> ElapsedMs {
        (self.us.get() / 1_000) as ElapsedMs
    }

    fn row_composed(&self, _row: usize, _view: &kivori_model::desk::DeskView, _overlay: bool) {
        self.spend(COMPOSE_US_PER_FRAME / TILE_ROWS as u64);
    }
}

/// A panel that only costs time: the CASET/RASET/RAMWR commands, the pixels on the wire, and the
/// CPU converting each pixel into the staging buffer.
struct Panel<'a> {
    chip: &'a Chip,
    windows: u32,
}

impl Panel<'_> {
    fn window(&mut self, rect: Rect) {
        let pixels = u64::from(rect.w) * u64::from(rect.h);
        let bytes = 11 + pixels * 2;
        self.chip
            .spend(bytes * 8 * 1_000_000 / SPI_HZ + pixels * STAGING_NS_PER_PIXEL / 1_000);
        self.windows += 1;
    }
}

impl DisplaySink for Panel<'_> {
    type Error = Infallible;

    fn blit_tile(&mut self, rect: Rect, _pixels: &[Rgb565]) -> Result<(), Self::Error> {
        self.window(rect);
        Ok(())
    }

    fn blit_tiles(&mut self, rect: Rect, _tiles: &[Rgb565], _cols: u16) -> Result<(), Self::Error> {
        self.window(rect);
        Ok(())
    }
}

/// The USB link as the host sees it, stamping every byte the device writes with the chip's time at
/// that moment, so a reply is dated when it left the device, not when the pass that sent it ended.
struct Link {
    pipe: SimPipe,
    chip: Rc<Chip>,
    written_at: VecDeque<u32>,
}

impl Transport for Link {
    type Error = Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.pipe.read(buf)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let taken = self.pipe.write(buf)?;
        self.written_at
            .extend(std::iter::repeat_n(self.chip.now_ms(), taken));
        Ok(taken)
    }
}

/// Encoder snapshots the test schedules, handed over once the runtime's clock reaches them.
#[derive(Default)]
struct Knob {
    edges: RefCell<VecDeque<(InputLevels, u32)>>,
    last: RefCell<InputLevels>,
}

impl InputSource for &Knob {
    fn sample(&mut self) -> InputLevels {
        *self.last.borrow()
    }

    fn drain(&mut self, now_ms: u32, f: &mut dyn FnMut(InputLevels, u32)) {
        let mut edges = self.edges.borrow_mut();
        while edges.front().is_some_and(|(_, at)| *at <= now_ms) {
            let (levels, at) = edges.pop_front().expect("front checked");
            *self.last.borrow_mut() = levels;
            f(levels, at);
        }
        f(*self.last.borrow(), now_ms);
    }
}

fn levels(phase: u8) -> InputLevels {
    InputLevels {
        a: phase & 1 != 0,
        b: phase & 2 != 0,
        ..InputLevels::default()
    }
}

struct Rig {
    runtime: Runtime<'static>,
    chip: Rc<Chip>,
    tx: TxBuffered<Link>,
    blob: AssetBlob<'static>,
    knob: Knob,
    seq: u16,
    /// (echo, host-observed ms) for every `Pong`.
    pongs: Vec<(u32, u32)>,
    /// Host-observed ms of every detent event and every `Health`.
    detents: Vec<u32>,
    health: Vec<u32>,
    link_dropped_at: Option<u32>,
    /// The longest a single `step` took.
    worst_tick_ms: u32,
    /// Host writes that found the device's receive pipe full and had to wait.
    host_blocked: u32,
}

impl Rig {
    /// A connected session at virtual time ~0, with the handshake done.
    fn new() -> Self {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            device_id: [0xAB; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            capabilities: CAPS,
        };
        let chip = Rc::new(Chip { us: Cell::new(0) });
        let mut rig = Self {
            runtime: Runtime::new(identity, RuntimeConfig::default()),
            chip: chip.clone(),
            tx: TxBuffered::new(Link {
                pipe: SimPipe::new(),
                chip,
                written_at: VecDeque::new(),
            }),
            blob: AssetBlob::parse(bytes).expect("valid blob"),
            knob: Knob::default(),
            seq: 0,
            pongs: Vec::new(),
            detents: Vec::new(),
            health: Vec::new(),
            link_dropped_at: None,
            worst_tick_ms: 0,
            host_blocked: 0,
        };
        rig.step();
        rig.send(&Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: CAPS,
            nonce: NONCE,
        }));
        rig.step();
        rig.send(&Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: CAPS,
        }));
        rig.step();
        rig.pongs.clear();
        rig.detents.clear();
        rig.health.clear();
        rig.worst_tick_ms = 0;
        rig
    }

    fn now(&self) -> u32 {
        self.chip.now_ms()
    }

    /// Host -> device. `false` when the device's receive pipe is full (the host would wait).
    fn send(&mut self, message: &Message) -> bool {
        let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
        encode_message(
            message,
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            self.seq,
            &mut wire,
        )
        .expect("encode");
        if self.tx.inner_mut().pipe.host_send(&wire).is_err() {
            self.host_blocked += 1;
            return false;
        }
        self.seq += 1;
        true
    }

    /// One pass of the run loop. A pass that composed no frame is the loop spinning: one
    /// millisecond, the resolution of every timestamp. The host reads what the pass produced.
    fn step(&mut self) {
        let before = self.now();
        let mut input = &self.knob;
        let mut panel = Panel {
            chip: &self.chip,
            windows: 0,
        };
        let tick = self.runtime.step(
            &*self.chip,
            &mut self.tx,
            &mut input,
            &mut panel,
            &self.blob,
        );
        if !tick.frame_rendered {
            self.chip.spend(1_000);
        }
        let now = self.now();
        self.worst_tick_ms = self.worst_tick_ms.max(now - before);
        if tick.link_dropped && self.link_dropped_at.is_none() {
            self.link_dropped_at = Some(now);
        }
        let link = self.tx.inner_mut();
        let bytes = link.pipe.host_recv();
        let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
        let mut packet: Vec<u8> = Vec::new();
        for byte in bytes {
            let at = link.written_at.pop_front().expect("every byte is stamped");
            if byte != 0 {
                packet.push(byte);
                continue;
            }
            let decoded = decode_message(&packet, &mut scratch, &[PROTOCOL_MAJOR]);
            packet.clear();
            let Ok((_, message)) = decoded else { continue };
            match message {
                Message::Pong(pong) => self.pongs.push((pong.t_ms_echo, at)),
                Message::Health(_) => self.health.push(at),
                Message::InputEvent(e) if matches!(e.kind, InputKind::Detent(_)) => {
                    self.detents.push(at);
                }
                _ => {}
            }
        }
    }

    /// Schedules `count` clockwise detents, one every `period_ms`, the first starting at `start_ms`.
    /// A detent is four snapshots 1 ms apart; the device stamps it at the last.
    fn spin(&mut self, count: u32, start_ms: u32, period_ms: u32) {
        let phases = match Direction::Cw {
            Direction::Cw => [2, 3, 1, 0],
            Direction::Ccw => [1, 3, 2, 0],
        };
        let mut edges = self.knob.edges.borrow_mut();
        for detent in 0..count {
            for (i, phase) in phases.into_iter().enumerate() {
                edges.push_back((levels(phase), start_ms + detent * period_ms + i as u32));
            }
        }
    }

    /// Runs until `until_ms`, the host pinging every `ping_ms` (`0` = never) and sending
    /// `flood_per_s` presentations a second (what a spin of that many detents a second makes the
    /// desktop send back). Returns the (ping send time) of every ping sent.
    fn run(&mut self, until_ms: u32, ping_ms: u32, flood_per_s: u32) -> Vec<u32> {
        let (mut pings, mut revision) = (Vec::new(), 0u32);
        let start = self.now();
        let mut next_ping = start + ping_ms;
        let mut next_flood = start;
        while self.now() < until_ms {
            let now = self.now();
            if ping_ms > 0
                && now >= next_ping
                && self.send(&Message::Ping(Ping { t_ms: next_ping }))
            {
                pings.push(next_ping);
                next_ping += ping_ms;
            }
            // The host sends each presentation when it is due, however slow the device is; a full
            // pipe makes it wait (USB flow control), and it then sends the backlog as space frees.
            while flood_per_s > 0 && now >= next_flood {
                revision += 1;
                let sent = self.send(&Message::Presentation(Presentation {
                    session: NONCE,
                    revision,
                    primary: PrimaryState::Idle,
                    value: Some(ValueDisplay {
                        kind: ValueKind::Volume,
                        current_percent: 50 + (revision % 2) as u8,
                        confidence: ValueConfidence::Confirmed,
                        at_boundary: false,
                    }),
                    transient_ms: 1_500,
                }));
                if !sent {
                    break;
                }
                next_flood += 1_000 / flood_per_s;
            }
            self.step();
        }
        pings
    }

    /// The longest any ping went unanswered, or `None` if one was never answered (a ping sent in
    /// the last second is still in flight and is left out).
    fn worst_round_trip(&self, pings: &[u32]) -> Option<u32> {
        let settled = pings.len().saturating_sub(1);
        pings[..settled]
            .iter()
            .map(|sent| {
                self.pongs
                    .iter()
                    .find(|(echo, _)| echo == sent)
                    .map(|(_, seen)| seen - sent)
            })
            .try_fold(0, |worst, rtt| rtt.map(|rtt| worst.max(rtt)))
    }
}

#[test]
fn a_whole_pass_of_the_loop_is_a_few_hundred_milliseconds_at_rp2040_speed() {
    // Not a requirement but the premise of the other tests: if this changes, so do their margins.
    let mut rig = Rig::new();
    let _ = rig.run(rig.now() + 3_000, 0, 0);
    assert!(
        (130..=330).contains(&rig.worst_tick_ms),
        "worst pass {} ms",
        rig.worst_tick_ms
    );
}

#[test]
fn every_heartbeat_is_answered_and_health_keeps_its_cadence_at_rp2040_speed() {
    let mut rig = Rig::new();
    let pings = rig.run(60_000, 1_000, 0);
    assert!(rig.link_dropped_at.is_none(), "the session was dropped");
    let worst = rig.worst_round_trip(&pings).expect("every ping answered");
    // A ping due during a pass waits for it to end, and the next pass answers it as it begins:
    // one pass at the most.
    assert!(worst <= rig.worst_tick_ms + 30, "round trip {worst} ms");
    assert!(worst < HOST_ALLOWANCE_MS / 4);
    // Health is on a 1 s cadence, late by at most one pass.
    let worst_gap = rig
        .health
        .windows(2)
        .map(|w| w[1] - w[0])
        .max()
        .expect("health reports");
    assert!(
        worst_gap <= 1_000 + rig.worst_tick_ms,
        "gap between Health reports {worst_gap} ms"
    );
    assert!(
        rig.health.len() >= 55,
        "{} Health reports in 60 s",
        rig.health.len()
    );
}

#[test]
fn a_detent_reaches_the_host_within_one_service_interval_of_composing() {
    let mut rig = Rig::new();
    let start = rig.now() + 300;
    // Sparse detents, each landing at a different point of a pass.
    let period = 497;
    rig.spin(24, start, period);
    let _ = rig.run(start + 24 * period + 1_000, 1_000, 0);
    assert_eq!(rig.detents.len(), 24, "every detent reported");
    // The detent completes 3 ms after it starts. Input is serviced between tile rows and windows,
    // so the longest wait is the longest stretch without a service point: one composed row or one
    // panel window, not a whole frame.
    let worst = (0..24)
        .map(|n| rig.detents[n] - (start + n as u32 * period + 3))
        .max()
        .expect("detents");
    let row_ms = (COMPOSE_US_PER_FRAME / TILE_ROWS as u64 / 1_000) as u32;
    assert!(
        worst <= 3 * row_ms,
        "worst detent-to-host latency {worst} ms (one compose row is {row_ms} ms)"
    );
}

#[test]
fn a_silent_host_ends_the_session_four_seconds_after_its_last_frame_to_within_one_pass() {
    let mut rig = Rig::new();
    let silent_from = rig.now();
    let _ = rig.run(silent_from + 10_000, 0, 0);
    let dropped = rig.link_dropped_at.expect("a silent host ends the session");
    let after = dropped - silent_from;
    assert!(
        (HOST_SILENCE_MS - rig.worst_tick_ms..=HOST_SILENCE_MS + rig.worst_tick_ms)
            .contains(&after),
        "dropped {after} ms after the last frame"
    );
}

#[test]
fn a_host_that_pings_inside_the_silence_limit_keeps_the_session_for_minutes() {
    let mut rig = Rig::new();
    // A ping every 3.5 s leaves 500 ms of margin against the 4 s limit; one slow pass is 250 ms.
    let pings = rig.run(120_000, 3_500, 0);
    assert!(
        rig.link_dropped_at.is_none(),
        "dropped at {:?}",
        rig.link_dropped_at
    );
    assert!(rig.worst_round_trip(&pings).is_some());
}

#[test]
fn a_fast_spin_with_a_presentation_per_detent_keeps_the_heartbeat_inside_the_hosts_allowance() {
    // 25, 50 and 100 detents a second (a slow, a fast and a violent human spin), each answered by
    // the desktop with one presentation.
    for rate in [25, 50, 100] {
        let mut rig = Rig::new();
        let start = rig.now();
        rig.spin(rate * 5, start + 1, 1_000 / rate);
        let pings = rig.run(start + 6_000, 1_000, rate);
        let worst = rig.worst_round_trip(&pings);
        eprintln!(
            "{rate}/s: worst round trip {worst:?} ms, host waited on a full pipe {} times, {} of {} detents",
            rig.host_blocked,
            rig.detents.len(),
            rate * 5
        );
        assert!(
            rig.link_dropped_at.is_none(),
            "{rate}/s dropped the session"
        );
        let worst = worst.unwrap_or(u32::MAX);
        assert!(
            worst < HOST_ALLOWANCE_MS,
            "{rate} detents a second: heartbeat round trip {worst} ms"
        );
    }
}
