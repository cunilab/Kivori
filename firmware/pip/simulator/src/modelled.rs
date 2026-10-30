//! The modelled-time device: the production runtime on its own clock, advanced by the host.
//!
//! In ideal timing one request is one tick at the request's time. A modelled device instead runs the
//! way the firmware loop does, back to back and never sleeping, on a virtual clock that the runtime
//! itself advances (compose cost per tile row, SPI time per window; see `timing`). A request says "let
//! the device run until `now_ms`" and brings the input and wire bytes the host stamped in the interval
//! since the last one.
//!
//! The runtime can be in the middle of a 79 ms frame when the host's time runs out, and a knob edge
//! stamped inside that frame may not be known yet. So the device runs on its own thread and parks at
//! every point where the runtime would look at input (the start of a tick, between tile rows, between
//! display windows) until the host has granted time up to that point. Edges and wire bytes are handed
//! over only when device time reaches their stamp. The host therefore sees the device as it would on
//! the bench: a detent stamped mid-frame is serviced at the next row boundary, not after the frame,
//! and the result depends only on the sequence of requests, never on thread scheduling (exactly one
//! side runs at a time).
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::mpsc::{channel, Receiver, Sender},
    thread::JoinHandle,
};

use super::*;
use crate::timing::{IDLE_PASS_US, SERVICE_POINT_US};

/// One request, as the device thread sees it.
struct Advance {
    to_ms: u32,
    wire: Vec<u8>,
    edges: Vec<Edge>,
    capture: bool,
    fault: Fault,
}

/// Host-time gate. All of it lives on the device thread; the host talks to it through channels.
pub struct Gate {
    advances: Receiver<Advance>,
    reports: Sender<Report>,
    shared: Shared,
    /// Device time the host has covered, in microseconds; `None` until the first request.
    granted_us: Option<u64>,
    /// A request has been taken and not answered yet.
    awaiting_report: bool,
    wire: VecDeque<(u64, Vec<u8>)>,
    edges: VecDeque<Edge>,
    capture: bool,
    fault: Fault,
    /// Device time at which the current tick began; a service point after it is inside a frame.
    tick_started_us: u64,
    /// The host hung up: stop waiting and let the device loop end.
    closed: bool,
    reboot: bool,
    state: &'static str,
}

impl Gate {
    fn new(advances: Receiver<Advance>, reports: Sender<Report>, shared: Shared) -> Self {
        Self {
            advances,
            reports,
            shared,
            granted_us: None,
            awaiting_report: false,
            wire: VecDeque::new(),
            edges: VecDeque::new(),
            capture: false,
            fault: Fault::Normal,
            tick_started_us: 0,
            closed: false,
            reboot: false,
            state: "booting",
        }
    }

    fn take(&mut self, advance: Advance) {
        let at_us = u64::from(advance.to_ms) * 1000;
        self.granted_us = Some(at_us);
        self.awaiting_report = true;
        if !advance.wire.is_empty() {
            self.wire.push_back((at_us, advance.wire));
        }
        self.edges.extend(advance.edges);
        self.capture = advance.capture;
        self.fault = advance.fault;
    }

    /// A service point at the clock's current time: parks until the host has covered it, then
    /// returns the edges stamped up to it and puts the wire bytes that arrived by then in the pipe.
    pub fn service(&mut self) -> Vec<Edge> {
        let us = self.shared.clock.us();
        while !self.closed && self.granted_us.is_none_or(|granted| us > granted) {
            if self.awaiting_report {
                self.awaiting_report = false;
                let report = self.shared.report(self.reboot, self.state, self.capture);
                if self.reports.send(report).is_err() {
                    self.closed = true;
                    break;
                }
            }
            match self.advances.recv() {
                Ok(advance) => self.take(advance),
                Err(_) => self.closed = true,
            }
        }
        while self.wire.front().is_some_and(|(at, _)| *at <= us) {
            let (_, bytes) = self.wire.pop_front().expect("front checked");
            // A full pipe keeps the rest for the next service point; the runtime drains it fast.
            if self.shared.link.borrow_mut().host_send(&bytes).is_err() {
                self.wire.push_front((us, bytes));
                break;
            }
        }
        let due = self
            .edges
            .iter()
            .take_while(|edge| u64::from(edge.at_ms) * 1000 <= us)
            .count();
        // Between rows and windows the loop does real work; a tick's first look at input is free.
        if self.shared.clock.us() != self.tick_started_us {
            self.shared.clock.advance_us(SERVICE_POINT_US);
        }
        self.edges.drain(..due).collect()
    }
}

/// The host's end of a modelled device.
pub struct ModelledSimulator {
    advances: Option<Sender<Advance>>,
    reports: Receiver<Report>,
    now: u32,
    rebooted: bool,
    thread: Option<JoinHandle<()>>,
}

impl ModelledSimulator {
    pub fn spawn() -> Self {
        let (advances, advance_rx) = channel();
        let (report_tx, reports) = channel();
        // The runtime keeps big frames on the stack; give the device the main thread's headroom.
        let thread = std::thread::Builder::new()
            .name("virtual-pip".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || device(advance_rx, report_tx))
            .expect("spawn the virtual Pip thread");
        Self {
            advances: Some(advances),
            reports,
            now: 0,
            rebooted: false,
            thread: Some(thread),
        }
    }

    /// Runs the device until `request.now_ms` (plus the rest of the row or window it is in).
    pub fn step(&mut self, request: Request) -> Result<Response, String> {
        validate(self.now, self.rebooted, &request)?;
        let fault = request.fault;
        let advance = Advance {
            to_ms: request.now_ms,
            wire: request.wire,
            edges: request.edges,
            capture: request.capture,
            fault,
        };
        self.advances
            .as_ref()
            .expect("sender lives until drop")
            .send(advance)
            .map_err(|_| "virtual Pip thread stopped")?;
        self.now = request.now_ms;
        let report = self
            .reports
            .recv()
            .map_err(|_| "virtual Pip thread stopped")?;
        self.rebooted = report.reboot;
        finish(fault, report)
    }
}

impl Drop for ModelledSimulator {
    fn drop(&mut self) {
        self.advances.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The device loop: what `run_buffered` does on the board, with time supplied by the model.
fn device(advances: Receiver<Advance>, reports: Sender<Report>) {
    let mut sim = Simulator::assemble(Timing::Modelled);
    let gate = Rc::new(RefCell::new(Gate::new(
        advances,
        reports,
        sim.shared.clone(),
    )));
    sim.input.gate = Some(gate.clone());
    let clock = sim.shared.clock.clone();
    loop {
        // The start of a tick is a service point too: it reads the pipe.
        let (fault, rebooted) = {
            let mut gate = gate.borrow_mut();
            let edges = gate.service();
            sim.input.edges.extend(edges);
            if gate.closed {
                return;
            }
            (gate.fault, gate.reboot)
        };
        let started = clock.us();
        gate.borrow_mut().tick_started_us = started;
        if !rebooted {
            let tick = sim.run_tick(fault);
            let mut gate = gate.borrow_mut();
            gate.reboot |= tick.reboot;
            gate.state = state_token(sim.runtime.state());
        }
        // A pass that took no modelled time still took some: the loop spins, the host cannot. A
        // rebooted device does nothing until the host restarts the process.
        if clock.us() == started {
            clock.advance_us(IDLE_PASS_US);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kivori_model::{
        desk::{ClockTime, ControlLabels, DeskStatus, DisplayMode, MediaStatus, MediaText},
        input::Direction,
        internals::DeviceInternals,
        presentation::{PrimaryState, ValueConfidence, ValueDisplay, ValueKind},
        ProtocolVersion,
    };
    use kivori_protocol::{
        decode_message, encode_message, Hello, InputKind, InternalsRequest, Message, Ping,
        Presentation, Ready, Status, PROTOCOL_MAJOR, PROTOCOL_MINOR,
    };

    const SESSION: u32 = 7;
    /// A ping timestamp the periodic heartbeats never use.
    const MARKER: u32 = 99_999;

    fn request(now_ms: u32) -> Request {
        Request {
            now_ms,
            wire: vec![],
            edges: vec![],
            capture: false,
            fault: Fault::Normal,
        }
    }

    fn messages(response: &Response) -> Vec<Message> {
        let mut scratch = Default::default();
        response
            .wire
            .split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .filter_map(|p| {
                decode_message(p, &mut scratch, &[PROTOCOL_MAJOR])
                    .ok()
                    .map(|(_, m)| m)
            })
            .collect()
    }

    /// A modelled device and the host's side of a negotiated session with it.
    struct Rig {
        sim: ModelledSimulator,
        seq: u16,
        now: u32,
        next_ping: u32,
        /// Input events the device sent: kind and the device time it stamped.
        events: Vec<(InputKind, u32)>,
        internals: Vec<DeviceInternals>,
        /// Pongs: the echoed ping stamp and the device time of the reply.
        pongs: Vec<(u32, u32)>,
    }

    impl Rig {
        fn new() -> Self {
            let mut rig = Self {
                sim: ModelledSimulator::spawn(),
                seq: 0,
                now: 0,
                next_ping: 1_000,
                events: vec![],
                internals: vec![],
                pongs: vec![],
            };
            let mut hello = request(0);
            hello.wire = rig.wire(&Message::Hello(Hello {
                desktop_version: VERSION,
                desktop_caps: CAPS,
                nonce: SESSION,
            }));
            rig.go(hello);
            let mut ready = request(10);
            ready.wire = rig.wire(&Message::Ready(Ready {
                negotiated_minor: PROTOCOL_MINOR,
                negotiated_caps: CAPS,
            }));
            rig.go(ready);
            rig
        }

        fn wire(&mut self, message: &Message) -> Vec<u8> {
            let mut wire = Default::default();
            encode_message(
                message,
                ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
                self.seq,
                &mut wire,
            )
            .unwrap();
            self.seq += 1;
            wire.to_vec()
        }

        fn go(&mut self, request: Request) -> Response {
            self.now = request.now_ms;
            let response = self.sim.step(request).expect("modelled step");
            for message in messages(&response) {
                match message {
                    Message::InputEvent(event) => self.events.push((event.kind, event.device_ms)),
                    Message::Internals(report) => self.internals.push(report.internals),
                    Message::Pong(pong) => self.pongs.push((pong.t_ms_echo, pong.uptime_ms)),
                    _ => {}
                }
            }
            response
        }

        /// What the app shows once connected: the buddy with clock, volume, media and a legend,
        /// the situation the unit's 79 ms figure was measured in.
        fn connect_desk(&mut self) {
            let status = DeskStatus {
                mode: DisplayMode::Buddy,
                clock: Some(ClockTime {
                    hour: 14,
                    minute: 32,
                    second: 10,
                }),
                volume_percent: Some(40),
                muted: Some(false),
                media: Some(MediaStatus::Playing),
                cpu_percent: Some(37),
                ram_percent: Some(62),
                high_load: false,
            };
            let labels = ControlLabels {
                rotate: MediaText::from_text("Volume"),
                press: MediaText::from_text("Play/Pause"),
                hold: MediaText::from_text("Mute"),
                profile: MediaText::from_text("Work"),
                ..ControlLabels::default()
            };
            let mut req = request(self.now + 5);
            let mut wire = self.wire(&Message::Status(Status {
                session: SESSION,
                status,
            }));
            wire.extend(self.wire(&Message::ControlLabels(
                kivori_protocol::ControlLabelsUpdate {
                    session: SESSION,
                    labels,
                },
            )));
            req.wire = wire;
            self.go(req);
        }

        /// Runs to `until` ms in `slice`-ms requests with a heartbeat each second, like a host
        /// polling in real time.
        fn run_to(&mut self, until: u32, slice: u32) {
            while self.now < until {
                let next = (self.now + slice).min(until);
                let mut req = request(next);
                if next >= self.next_ping {
                    self.next_ping += 1_000;
                    req.wire = self.wire(&Message::Ping(Ping { t_ms: next }));
                }
                self.go(req);
            }
        }

        /// Asks for one internals snapshot and returns it.
        fn snapshot(&mut self) -> DeviceInternals {
            let before = self.internals.len();
            let mut ask = request(self.now + 20);
            ask.wire = self.wire(&Message::InternalsRequest(InternalsRequest {
                session: SESSION,
            }));
            self.go(ask);
            // The device answers once the frame it is in has been drawn.
            self.run_to(self.now + 250, 5);
            assert!(self.internals.len() > before, "the device answered");
            *self.internals.last().expect("snapshot")
        }
    }

    #[test]
    fn the_connected_desk_reports_the_measured_unit_figures() {
        let mut rig = Rig::new();
        rig.connect_desk();
        rig.run_to(5_000, 5);
        let internals = rig.snapshot();
        // The unit (device_probe internals, connected desk): compose 79 ms, 12 fps, busy ~100%.
        let compose = i32::from(internals.compose_ms.expect("compose"));
        assert!((compose - 79).abs() <= 1, "compose {compose} ms");
        assert_eq!(internals.fps, Some(12), "fps");
        let busy = internals.busy_percent.expect("busy");
        assert!((99..=100).contains(&busy), "busy {busy}%");
        let flush = internals.flush_ms.expect("flush");
        assert!(flush <= 5, "flush {flush} ms (the unit: 0 to 5 ms)");
        // The chip probe: flash, RAM, stack and the model's chip.
        assert_eq!(internals.stack_free_min_bytes, Some(162_180 - 115_744));
        assert_eq!(internals.asset_blob_cap_bytes, Some(131_072));
        assert_eq!(internals.temperature_decicelsius, None);
    }

    /// Turns the knob one detent clockwise, its level changes stamped `offset` to `offset + 3` ms
    /// after now, while the host polls every millisecond (as fast as the stamps). Returns how long
    /// after the last level change the detent reached the host: the device's service delay.
    fn detent_latency(rig: &mut Rig, offset: u32) -> u32 {
        rig.events.clear();
        let t0 = rig.now;
        for ms in 1..=300 {
            let mut req = request(t0 + ms);
            if (offset..offset + 4).contains(&ms) {
                req.edges = vec![Edge {
                    at_ms: t0 + ms,
                    levels: [2u8, 3, 1, 0][(ms - offset) as usize],
                }];
            }
            rig.go(req);
            if rig
                .events
                .iter()
                .any(|(kind, _)| *kind == InputKind::Detent(Direction::Cw))
            {
                return ms.saturating_sub(offset + 3);
            }
        }
        panic!("no detent at offset {offset}");
    }

    #[test]
    fn a_knob_edge_stamped_mid_frame_is_handled_within_one_row() {
        let mut rig = Rig::new();
        rig.connect_desk();
        rig.run_to(3_000, 5);
        // Edges at every phase of the 79 ms frame: the worst wait is the longest tile row (23.8 ms of
        // drawing), not the frame.
        let mut worst = 0;
        for offset in (1..=80).step_by(7) {
            worst = worst.max(detent_latency(&mut rig, offset));
            // Let the gesture end and the frame cadence continue before the next one.
            let to = rig.now + 400;
            rig.run_to(to, 5);
        }
        assert!(
            worst <= 25,
            "worst service delay {worst} ms; a whole frame is 79 ms"
        );
        assert!(
            worst >= 5,
            "the sweep never caught a frame mid-row ({worst} ms)"
        );
    }

    #[test]
    fn host_bytes_are_not_seen_before_they_arrive_and_not_a_frame_late() {
        let mut rig = Rig::new();
        rig.connect_desk();
        rig.run_to(3_000, 5);
        let t0 = rig.now;
        // The Ping is stamped at the end of the interval, t0 + 200, while the device is mid-frame.
        let mut ping = request(t0 + 200);
        ping.wire = rig.wire(&Message::Ping(Ping { t_ms: MARKER }));
        rig.go(ping);
        rig.run_to(t0 + 400, 5);
        let &(_, answered) = rig
            .pongs
            .iter()
            .find(|(echo, _)| *echo == MARKER)
            .expect("pong");
        // A tick answers it at its start, so at most one frame (79 ms plus the flush) after it
        // arrived, and never before it did.
        assert!(
            (t0 + 200..=t0 + 200 + 90).contains(&answered),
            "answered at {answered} for a ping that arrived at {}",
            t0 + 200
        );
    }

    #[test]
    fn the_result_does_not_depend_on_how_requests_are_sliced() {
        let run = |slice: u32| {
            let mut rig = Rig::new();
            rig.connect_desk();
            rig.run_to(2_000, slice);
            let internals = rig.snapshot();
            (internals.compose_ms, internals.flush_ms, internals.fps)
        };
        assert_eq!(run(5), run(5), "repeatable");
        assert_eq!(run(5), run(40), "independent of the polling interval");
    }

    /// A host that turns the knob at `period_ms` per detent (alternating directions, four edges a
    /// detent) and answers every detent with one `Presentation` (about 28 bytes on the wire), with
    /// a heartbeat a second, for `secs` seconds. Returns the worst heartbeat round trip in ms, the
    /// detents turned and the detents the device reported.
    fn flood(period_ms: u32, secs: u32) -> (u32, usize, usize) {
        let mut rig = Rig::new();
        rig.connect_desk();
        rig.run_to(2_000, 5);
        rig.pongs.clear();
        rig.events.clear();
        let t0 = rig.now;
        let (mut detents, mut revision) = (0usize, 0u32);
        let mut pings = vec![];
        for ms in 1..=secs * 1_000 {
            let at = t0 + ms;
            let mut req = request(at);
            let phase = ms % period_ms;
            if (1..=4).contains(&phase) {
                let table = if detents % 2 == 0 {
                    [2u8, 3, 1, 0]
                } else {
                    [1u8, 3, 2, 0]
                };
                req.edges = vec![Edge {
                    at_ms: at,
                    levels: table[(phase - 1) as usize],
                }];
                if phase == 4 {
                    detents += 1;
                    revision += 1;
                    req.wire = rig.wire(&Message::Presentation(Presentation {
                        session: SESSION,
                        revision,
                        primary: PrimaryState::Idle,
                        value: Some(ValueDisplay {
                            kind: ValueKind::Volume,
                            current_percent: 50 + (detents % 2) as u8,
                            confidence: ValueConfidence::Confirmed,
                            at_boundary: false,
                        }),
                        transient_ms: 1_500,
                    }));
                }
            }
            if ms % 1_000 == 0 {
                pings.push(at);
                let ping = rig.wire(&Message::Ping(Ping { t_ms: at }));
                req.wire.extend(ping);
            }
            rig.go(req);
        }
        // The last ping may still be in flight; every earlier one must have been answered.
        let worst = pings[..pings.len() - 1]
            .iter()
            .map(|&sent| {
                let &(_, answered) = rig
                    .pongs
                    .iter()
                    .find(|(echo, _)| *echo == sent)
                    .expect("every heartbeat is answered");
                answered - sent
            })
            .max()
            .expect("pings");
        let reported = rig
            .events
            .iter()
            .filter(|(kind, _)| matches!(kind, InputKind::Detent(_)))
            .count();
        (worst, detents, reported)
    }

    #[test]
    fn a_detent_flood_with_presentations_back_keeps_the_heartbeat_inside_its_allowance() {
        // The endurance run's pace (250 detents a second) is a stress test; a fast human spin is
        // 50 to 100 a second for a second or two. 200 a second with a presentation for every detent
        // is about 5.6 KB/s inbound, close to the 6.4 KB/s the runtime reads (512 bytes a frame at
        // 12.5 frames a second), and the heartbeat is still answered within a frame or two. The
        // host gives up after 2 s without a reply (3 heartbeats at 1 s).
        let (worst, turned, reported) = flood(5, 20);
        // The last detent may still be in flight when the run ends.
        assert!(
            turned - reported <= 1,
            "{reported} of {turned} detents reached the host"
        );
        assert!(worst <= 250, "worst heartbeat round trip {worst} ms");
        // A realistic fast spin has a very wide margin.
        let (worst, turned, reported) = flood(10, 10);
        assert!(
            turned - reported <= 1,
            "{reported} of {turned} detents reached the host"
        );
        assert!(worst <= 250, "worst heartbeat round trip {worst} ms");
    }

    #[test]
    fn dropping_the_host_ends_the_device_thread() {
        let mut rig = Rig::new();
        rig.run_to(100, 10);
        drop(rig); // joins; a parked device thread that ignored the hang-up would block here
    }
}
