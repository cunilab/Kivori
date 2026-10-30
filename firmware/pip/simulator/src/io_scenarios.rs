//! Digital I/O scheduling checks; payload wire-time is modeled, not measured hardware latency.
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use kivori_assets::AssetBlob;
use kivori_firmware::{
    health::DeviceDiagnostic,
    ports::{Clock, DisplaySink, InputSource, Transport},
    profile::physical_st7789::{CAPABILITIES, SPI_CLOCK_HZ},
    proto::DeviceIdentity,
    render::FRAME_PIXELS,
    runtime::{Runtime, RuntimeConfig, Tick},
    sim::CaptureDisplay,
    tx_buffer::{TxBuffered, FIFO_BYTES},
};
use kivori_model::{
    input::{Direction, InputLevels},
    ProtocolVersion, Rect, Rgb565,
};
use kivori_protocol::{
    decode_message, encode_message, Hello, InputKind, Message, Ready, PROTOCOL_MAJOR,
    PROTOCOL_MINOR,
};

const SERVICE_US: u64 = 1_000;
const EDGE_CAPACITY: usize = 128;

#[derive(Default)]
struct Bus {
    us: u64,
    blocked: bool,
    fifo: VecDeque<u8>,
    rx: VecDeque<u8>,
    delivered: Vec<(u8, u64)>,
    high_water: usize,
}

impl Bus {
    fn advance(&mut self, us: u64) {
        let next = (self.us / SERVICE_US + 1) * SERVICE_US;
        self.us += us;
        if !self.blocked && next <= self.us {
            self.delivered
                .extend(self.fifo.drain(..).map(|b| (b, next)));
        }
    }
}

type Shared = Rc<RefCell<Bus>>;
struct IoClock(Shared);
impl Clock for IoClock {
    fn now_ms(&self) -> u32 {
        (self.0.borrow().us / 1_000) as u32
    }
}

struct Fifo(Shared);
impl Transport for Fifo {
    type Error = ();
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        let mut bus = self.0.borrow_mut();
        let count = buf.len().min(bus.rx.len()).min(FIFO_BYTES);
        for byte in &mut buf[..count] {
            *byte = bus.rx.pop_front().unwrap();
        }
        Ok(count)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, ()> {
        let mut bus = self.0.borrow_mut();
        let count = bytes.len().min(FIFO_BYTES - bus.fifo.len());
        bus.fifo.extend(&bytes[..count]);
        bus.high_water = bus.high_water.max(bus.fifo.len());
        Ok(count)
    }
}

struct TimedDisplay {
    capture: Box<CaptureDisplay>,
    bus: Shared,
    fail_after: Option<usize>,
}
impl TimedDisplay {
    fn transfer(&mut self, pixels: usize) -> Result<(), ()> {
        if self.fail_after == Some(0) {
            self.fail_after = None;
            return Err(());
        }
        if let Some(left) = &mut self.fail_after {
            *left -= 1;
        }
        let us = (pixels as u64 * 16 * 1_000_000).div_ceil(u64::from(SPI_CLOCK_HZ));
        self.bus.borrow_mut().advance(us);
        Ok(())
    }
}
impl DisplaySink for TimedDisplay {
    type Error = ();
    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), ()> {
        self.transfer(pixels.len())?;
        self.capture.blit_tile(rect, pixels).unwrap();
        Ok(())
    }
    fn blit_tiles(&mut self, rect: Rect, pixels: &[Rgb565], cols: u16) -> Result<(), ()> {
        self.transfer(pixels.len())?;
        self.capture.blit_tiles(rect, pixels, cols).unwrap();
        Ok(())
    }
    fn set_brightness(&mut self, step: u8) {
        self.capture.set_brightness(step);
    }
}

#[derive(Default)]
struct ScheduledInput {
    scheduled: VecDeque<(u32, InputLevels)>,
    levels: InputLevels,
    dropped: u32,
    high_water: u32,
}
impl InputSource for ScheduledInput {
    fn sample(&mut self) -> InputLevels {
        self.levels
    }
    fn drain(&mut self, now: u32, f: &mut dyn FnMut(InputLevels, u32)) {
        let mut queue = VecDeque::new();
        while self.scheduled.front().is_some_and(|(at, _)| *at <= now) {
            if queue.len() == EDGE_CAPACITY {
                queue.pop_front();
                self.dropped += 1;
            }
            queue.push_back(self.scheduled.pop_front().unwrap());
        }
        self.high_water = self.high_water.max(queue.len() as u32);
        for (at, levels) in queue {
            self.levels = levels;
            f(levels, at);
        }
        f(self.levels, now);
    }
    fn dropped_edges(&self) -> u32 {
        self.dropped
    }
    fn edge_high_water(&self) -> u32 {
        self.high_water
    }
}

struct Rig {
    runtime: Runtime<'static>,
    seq: u16,
    bus: Shared,
    clock: IoClock,
    transport: TxBuffered<Fifo>,
    display: TimedDisplay,
    input: ScheduledInput,
}
impl Rig {
    fn new() -> Self {
        let bus = Shared::default();
        let frame = Box::leak(
            vec![Rgb565::from_raw(0); FRAME_PIXELS]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
        );
        Self {
            runtime: Runtime::with_frame_buffer(
                DeviceIdentity {
                    device_id: *b"IO-SMOKE-PIP-001",
                    firmware_version: super::VERSION,
                    capabilities: CAPABILITIES,
                },
                RuntimeConfig::default(),
                frame,
            ),
            seq: 0,
            bus: bus.clone(),
            clock: IoClock(bus.clone()),
            transport: TxBuffered::new(Fifo(bus.clone())),
            display: TimedDisplay {
                capture: Box::new(CaptureDisplay::new()),
                bus,
                fail_after: None,
            },
            input: ScheduledInput::default(),
        }
    }
    fn send(&mut self, message: Message) {
        let mut wire = Default::default();
        encode_message(
            &message,
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            self.seq,
            &mut wire,
        )
        .unwrap();
        self.seq = self.seq.wrapping_add(1);
        self.bus.borrow_mut().rx.extend(wire);
    }
    fn handshake(&mut self, nonce: u32) {
        self.send(Message::Hello(Hello {
            desktop_version: super::VERSION,
            desktop_caps: CAPABILITIES,
            nonce,
        }));
        self.send(Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: CAPABILITIES,
        }));
    }
    fn step(&mut self) -> Tick {
        let blob =
            AssetBlob::parse(include_bytes!(concat!(env!("OUT_DIR"), "/pip.assets"))).unwrap();
        self.runtime.step(
            &self.clock,
            &mut self.transport,
            &mut self.input,
            &mut self.display,
            &blob,
        )
    }
    fn drain(&mut self) {
        for _ in 0..100 {
            self.bus.borrow_mut().advance(SERVICE_US);
            self.transport.pump().unwrap();
            if self.transport.pending() == 0 && self.bus.borrow().fifo.is_empty() {
                return;
            }
        }
        panic!("finite output did not drain");
    }
    fn detent(&mut self, start: u32) {
        for (i, (a, b)) in [(false, true), (true, true), (true, false), (false, false)]
            .into_iter()
            .enumerate()
        {
            self.input.scheduled.push_back((
                start + i as u32,
                InputLevels {
                    a,
                    b,
                    ..InputLevels::default()
                },
            ));
        }
    }
    fn packets(&self) -> Vec<(Result<Message, String>, u64)> {
        let mut packet = Vec::new();
        let mut result = Vec::new();
        for &(byte, at) in &self.bus.borrow().delivered {
            if byte == 0 {
                if !packet.is_empty() {
                    result.push((
                        decode_message(&packet, &mut Default::default(), &[PROTOCOL_MAJOR])
                            .map(|(_, m)| m)
                            .map_err(|e| format!("{e:?}")),
                        at,
                    ));
                    packet.clear();
                }
            } else {
                packet.push(byte);
            }
        }
        if !packet.is_empty() {
            result.push((Err("incomplete final packet".into()), self.bus.borrow().us));
        }
        result
    }
}

fn check(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.into())
}

fn timed_input_and_retry() -> Result<(), String> {
    let mut rig = Rig::new();
    rig.handshake(42);
    let tick = rig.step();
    check(
        tick.tiles_flushed == 36 && tick.windows_flushed == 3,
        "initial full frame must use three buffered windows",
    )?;
    check(
        rig.bus.borrow().us == 46_080,
        "full-frame payload must take exactly 46.08ms at 20MHz",
    )?;
    rig.drain();
    check(
        rig.packets().iter().all(|(m, _)| m.is_ok()),
        "healthy startup output must decode",
    )?;
    check(
        rig.packets()
            .iter()
            .any(|(m, _)| matches!(m, Ok(Message::HelloAck(_)))),
        "startup must handshake",
    )?;
    println!("PASS IO full-frame: 36 tiles, three windows, 46080us payload wire-time at 20MHz");

    // A transfer fault invalidates renderer signatures; the retry must repaint the whole panel.
    rig.bus.borrow_mut().advance(100_000);
    rig.display.fail_after = Some(1);
    let failed = rig.step();
    check(
        failed.windows_flushed == 1 && failed.diagnostic == Some(DeviceDiagnostic::DisplayFault),
        "second-window failure must report DisplayFault after one successful window",
    )?;
    rig.drain();
    rig.bus.borrow_mut().advance(100_000);
    let start_us = rig.bus.borrow().us;
    let start_ms = rig.clock.now_ms();
    rig.bus.borrow_mut().delivered.clear();
    rig.detent(start_ms + 1);
    let retry = rig.step();
    let frame_end = rig.bus.borrow().us;
    check(
        retry.tiles_flushed == 36 && retry.windows_flushed == 3 && retry.diagnostic.is_none(),
        "failed transfer must retry a complete buffered frame",
    )?;
    rig.drain();
    let detents: Vec<_> = rig
        .packets()
        .into_iter()
        .filter_map(|(m, at)| match m {
            Ok(Message::InputEvent(e)) if matches!(e.kind, InputKind::Detent(_)) => Some((e, at)),
            _ => None,
        })
        .collect();
    check(
        detents.len() == 1,
        "timed raw cycle must emit exactly one detent",
    )?;
    let (event, delivered) = &detents[0];
    check(
        event.session == 42 && event.kind == InputKind::Detent(Direction::Cw),
        "timed input must preserve direction and session",
    )?;
    check(
        rig.packets().iter().all(|(m, _)| m.is_ok()),
        "healthy timed stream must decode completely",
    )?;
    check(
        event.device_ms == start_ms + 4,
        "event must retain edge capture time",
    )?;
    check(
        *delivered < frame_end,
        "detent must reach host before frame completion",
    )?;
    // The two input packets fit in one FIFO; the host services it every modeled millisecond.
    let max_latency_us = 15_360 + SERVICE_US;
    check(
        delivered - u64::from(event.device_ms) * 1_000 <= max_latency_us,
        "detent exceeded one window plus 1ms host-service bound",
    )?;
    check(
        frame_end - start_us == 46_080,
        "retry full-frame payload wire-time drifted",
    )?;
    println!("PASS IO timed-input: raw detent reaches host before frame completion; bounded by one window + 1ms");
    println!("PASS IO display-retry: second-window failure reports diagnostic and next frame repaints all 36 tiles");
    Ok(())
}

fn stalled_host() -> Result<(), String> {
    let mut rig = Rig::new();
    rig.handshake(42);
    rig.step();
    rig.drain();
    rig.bus.borrow_mut().delivered.clear();
    rig.bus.borrow_mut().blocked = true;
    let mut late_windows = 0;
    for second in 0..121 {
        rig.bus.borrow_mut().advance(1_000_000);
        let now = rig.clock.now_ms();
        rig.detent(now);
        let tick = rig.step();
        if second >= 111 {
            check(
                tick.frame_rendered,
                "render scheduling stopped in the final ten stalled ticks",
            )?;
            late_windows += tick.windows_flushed;
        }
    }
    check(
        rig.bus.borrow().fifo.len() == FIFO_BYTES && rig.transport.pending() > 0,
        "blocked host must fill FIFO and production TX queue",
    )?;
    check(
        rig.bus.borrow().delivered.is_empty(),
        "blocked host must receive no bytes",
    )?;
    check(
        late_windows > 0,
        "display must transfer windows in the final ten stalled ticks",
    )?;
    rig.bus.borrow_mut().blocked = false;
    rig.handshake(77);
    rig.step();
    rig.drain();
    check(
        rig.packets()
            .iter()
            .any(|(m, _)| matches!(m, Ok(Message::HelloAck(ack)) if ack.nonce_echo == 77)),
        "fresh Hello must recover after stale queued output",
    )?;
    // Old FIFO bytes may precede the delimiter that repairs a cut-off stale frame.
    rig.bus.borrow_mut().delivered.clear();
    let start = rig.clock.now_ms() + 1;
    rig.detent(start);
    rig.bus.borrow_mut().advance(10_000);
    rig.step();
    rig.drain();
    let packets = rig.packets();
    check(
        packets.iter().all(|(m, _)| m.is_ok()),
        "post-recovery stream must contain only complete packets",
    )?;
    check(packets.iter().filter(|(m, _)| matches!(m, Ok(Message::InputEvent(e)) if e.session == 77 && matches!(e.kind, InputKind::Detent(_)))).count() == 1, "fresh session must accept exactly one new detent")?;
    check(
        rig.bus.borrow().high_water == FIFO_BYTES,
        "FIFO must fill to its modeled 64-byte capacity",
    )?;
    println!("PASS IO host-stall: 121 virtual seconds, bounded 64-byte FIFO, rendering continues, fresh handshake and raw detent recover");
    Ok(())
}

pub fn run() -> Result<(), String> {
    timed_input_and_retry()?;
    stalled_host()?;
    println!("IO model excludes analog contacts, CPU/DMA overhead, USB enumeration, physical timing and real OS outputs.");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn production_stack_survives_timed_io_and_backpressure() {
        super::run().unwrap();
    }

    #[test]
    fn scheduled_input_drops_oldest_and_retains_release() {
        use super::*;
        let mut input = ScheduledInput::default();
        for at in 0..129 {
            input.scheduled.push_back((
                at,
                InputLevels {
                    sw: at < 128,
                    ..InputLevels::default()
                },
            ));
        }
        let mut samples = Vec::new();
        input.drain(129, &mut |levels, at| samples.push((levels.sw, at)));
        assert_eq!(input.dropped_edges(), 1);
        assert_eq!(input.edge_high_water(), 128);
        assert_eq!(samples.len(), 129);
        assert_eq!(samples.first(), Some(&(true, 1)));
        assert_eq!(samples.last(), Some(&(false, 129)));
        assert!(input.scheduled.is_empty());
        assert!(!input.sample().sw);
    }
}
