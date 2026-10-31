//! Product-owned raw-input scenarios. Firmware time is virtual; desktop deadlines are not tested.
mod replay;
pub use replay::run as replay;

use super::*;
use kivori_model::{input::Direction, Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, ControlId, Hello, InputEvent, InputKind, Message, Ping, Ready,
    PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

#[derive(Debug, PartialEq, Eq)]
struct Seen(ControlId, InputKind, u32);

struct Rig {
    sim: Simulator,
    seq: u16,
    nonce: u32,
    events: Vec<Seen>,
    frame_hash: u64,
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.into())
}

fn request(now_ms: u32, edges: &[(u32, u8)]) -> Request {
    Request {
        now_ms,
        wire: vec![],
        edges: edges
            .iter()
            .map(|&(at_ms, levels)| Edge { at_ms, levels })
            .collect(),
        capture: true,
        fault: Fault::Normal,
    }
}

impl Rig {
    fn new(double: bool) -> Result<Box<Self>, String> {
        let mut rig = Box::new(Self {
            sim: Simulator::new(),
            seq: 0,
            nonce: 42,
            events: vec![],
            frame_hash: 0,
        });
        rig.connect(0, double)?;
        Ok(rig)
    }

    fn connect(&mut self, now: u32, double: bool) -> Result<(), String> {
        let caps = if double {
            CAPS
        } else {
            Capabilities::from_bits(CAPS.bits() & !Capabilities::DOUBLE_PRESS_V1.bits())
        };
        let response = self.send(
            now,
            Message::Hello(Hello {
                desktop_version: VERSION,
                desktop_caps: caps,
                nonce: self.nonce,
            }),
        )?;
        require(
            response
                .wire
                .split(|&b| b == 0)
                .filter(|p| !p.is_empty())
                .any(|packet| {
                    matches!(
                        decode_message(packet, &mut Default::default(), &[PROTOCOL_MAJOR]),
                        Ok((_, Message::HelloAck(_)))
                    )
                }),
            "raw input scenario missing real HelloAck",
        )?;
        self.send(
            now + 1,
            Message::Ready(Ready {
                negotiated_minor: PROTOCOL_MINOR,
                negotiated_caps: caps,
            }),
        )?;
        require(self.events.is_empty(), "handshake emitted an input action")
    }

    fn send(&mut self, now: u32, message: Message) -> Result<Response, String> {
        let mut encoded = Default::default();
        encode_message(
            &message,
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            self.seq,
            &mut encoded,
        )
        .map_err(|e| format!("scenario encode: {e:?}"))?;
        self.seq = self.seq.wrapping_add(1);
        let mut req = request(now, &[]);
        req.wire = encoded.to_vec();
        let response = self.sim.step(req)?;
        self.observe(&response)?;
        Ok(response)
    }

    fn step(&mut self, now: u32, edges: &[(u32, u8)]) -> Result<bool, String> {
        let response = self.sim.step(request(now, edges))?;
        self.observe(&response)?;
        Ok(response.reboot)
    }

    fn observe(&mut self, response: &Response) -> Result<(), String> {
        for packet in response.wire.split(|&b| b == 0).filter(|p| !p.is_empty()) {
            let (_, message) = decode_message(packet, &mut Default::default(), &[PROTOCOL_MAJOR])
                .map_err(|e| format!("scenario response decode: {e:?}"))?;
            if let Message::InputEvent(InputEvent {
                session,
                control,
                kind,
                device_ms,
                ..
            }) = message
            {
                require(
                    session == self.nonce,
                    "input crossed the current session boundary",
                )?;
                self.events.push(Seen(control, kind, device_ms));
            }
        }
        let frame = response.frame.as_ref().ok_or("scenario capture missing")?;
        require(
            frame.len() == 240 * 240 && frame.iter().any(|&p| p != 0),
            "invalid or blank firmware capture",
        )?;
        self.frame_hash = frame.iter().fold(0xcbf29ce484222325, |hash, pixel| {
            pixel.to_le_bytes().iter().fold(hash, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            })
        });
        Ok(())
    }

    fn expect(&mut self, name: &str, expected: Vec<Seen>, settle_at: u32) -> Result<(), String> {
        self.step(settle_at, &[])?;
        require(
            self.events == expected,
            &format!("{name}: expected {expected:?}, actual {:?}", self.events),
        )?;
        println!(
            "PASS S5.3 {name}: trace={:?}; frame-fnv64={:016x}; virtual firmware ms={settle_at}",
            self.events, self.frame_hash
        );
        Ok(())
    }
}

fn seen(control: ControlId, kind: InputKind, ms: u32) -> Seen {
    Seen(control, kind, ms)
}
fn button(kind: InputKind, ms: u32) -> Seen {
    seen(ControlId::Button, kind, ms)
}
fn rotary(kind: InputKind, ms: u32) -> Seen {
    seen(ControlId::Rotary, kind, ms)
}

pub fn run() -> Result<(), String> {
    // Repeated Runtime construction needs the same stack budget on every host, including Windows.
    std::thread::Builder::new()
        .name("raw-input-scenarios".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(scenarios)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "raw-input scenario thread panicked".to_string())?
}

fn scenarios() -> Result<(), String> {
    for (name, phases, direction) in [
        ("clockwise quadrature", [2, 3, 1, 0], Direction::Cw),
        ("counterclockwise quadrature", [1, 3, 2, 0], Direction::Ccw),
    ] {
        let mut rig = Rig::new(false)?;
        let edges: Vec<_> = phases
            .into_iter()
            .enumerate()
            .map(|(i, phase)| (100 + i as u32, phase))
            .collect();
        rig.step(104, &edges)?;
        rig.expect(
            name,
            vec![
                rotary(InputKind::GestureStarted, 103),
                rotary(InputKind::Detent(direction), 103),
                rotary(InputKind::GestureEnded, 354),
            ],
            354,
        )?;
    }
    for (name, phases) in [
        ("invalid two-bit transitions", vec![3, 1, 0]),
        ("partial-cycle bounce", vec![2, 0, 2, 0]),
        ("invalid gap then rest bounce", vec![2, 1, 0, 2, 0]),
    ] {
        let mut rig = Rig::new(false)?;
        let edges: Vec<_> = phases
            .into_iter()
            .enumerate()
            .map(|(i, phase)| (100 + i as u32, phase))
            .collect();
        rig.step(110, &edges)?;
        rig.expect(name, vec![], 400)?;
        rig.step(504, &[(500, 2), (501, 3), (502, 1), (503, 0)])?;
        rig.expect(
            "valid detent after rejected motion",
            vec![
                rotary(InputKind::GestureStarted, 503),
                rotary(InputKind::Detent(Direction::Cw), 503),
                rotary(InputKind::GestureEnded, 754),
            ],
            754,
        )?;
    }
    for (duration, kind) in [
        (19, None),
        (20, Some(InputKind::Press)),
        (499, Some(InputKind::Press)),
        (500, Some(InputKind::Hold)),
        (1_999, Some(InputKind::Hold)),
        (2_000, None),
    ] {
        let mut rig = Rig::new(false)?;
        let released = 100 + duration;
        rig.step(released + 20, &[(100, 4), (released, 0)])?;
        rig.expect(
            &format!("switch duration {duration}ms"),
            kind.map(|kind| vec![button(kind, released + 20)])
                .unwrap_or_default(),
            released + 400,
        )?;
    }
    let mut rig = Rig::new(false)?;
    rig.step(
        250,
        &[
            (100, 4),
            (105, 0),
            (109, 4),
            (115, 0),
            (120, 4),
            (200, 0),
            (205, 4),
            (210, 0),
        ],
    )?;
    rig.expect(
        "switch bounce yields one press",
        vec![button(InputKind::Press, 250)],
        600,
    )?;

    for gap in [249, 250, 251] {
        let mut rig = Rig::new(true)?;
        rig.step(220, &[(100, 4), (200, 0)])?;
        require(
            rig.events.is_empty(),
            "negotiated lone press fired before double window",
        )?;
        rig.step(200 + gap + 20, &[(200 + gap, 4)])?;
        rig.step(300 + gap + 20, &[(300 + gap, 0)])?;
        let expected = if gap < 250 {
            vec![button(InputKind::DoublePress, 320 + gap)]
        } else {
            vec![
                button(InputKind::Press, 200 + gap),
                button(InputKind::Press, 700 + gap),
            ]
        };
        rig.expect(&format!("double-press gap {gap}ms"), expected, 700 + gap)?;
    }
    for (mask, control) in [
        (4, ControlId::Button),
        (8, ControlId::ContextButton(0)),
        (16, ControlId::ContextButton(1)),
        (32, ControlId::ContextButton(2)),
    ] {
        let mut rig = Rig::new(false)?;
        rig.step(1_000, &[(100, mask), (520, 0), (550, mask), (970, 0)])?;
        rig.expect(
            &format!("delayed buffered {control:?}"),
            vec![
                seen(control, InputKind::Press, 550),
                seen(control, InputKind::Press, 1_000),
            ],
            1_400,
        )?;
    }

    let mut rig = Rig::new(false)?;
    rig.step(
        230,
        &[(100, 4), (110, 6), (111, 7), (112, 5), (113, 4), (200, 0)],
    )?;
    rig.expect(
        "switch owns rotation",
        vec![button(InputKind::Press, 230)],
        600,
    )?;
    let mut rig = Rig::new(false)?;
    rig.step(230, &[(100, 24), (200, 0)])?;
    rig.expect("simultaneous contextual buttons do not chord", vec![], 600)?;
    let mut rig = Rig::new(false)?;
    rig.step(320, &[(100, 8), (130, 12), (250, 8), (300, 0)])?;
    rig.expect(
        "earlier contextual button owns knob press",
        vec![seen(ControlId::ContextButton(0), InputKind::Press, 320)],
        600,
    )?;
    let mut rig = Rig::new(false)?;
    rig.step(
        240,
        &[(100, 2), (101, 3), (102, 1), (103, 0), (120, 4), (220, 0)],
    )?;
    rig.step(500, &[])?;
    rig.step(720, &[(600, 4), (700, 0)])?;
    rig.expect(
        "rotary ownership expires before fresh press",
        vec![
            rotary(InputKind::GestureStarted, 103),
            rotary(InputKind::Detent(Direction::Cw), 103),
            rotary(InputKind::GestureEnded, 500),
            button(InputKind::Press, 720),
        ],
        1_100,
    )?;

    capacity_and_overflow()?;
    session_and_recovery()?;
    Ok(())
}

fn capacity_and_overflow() -> Result<(), String> {
    let mut rig = Rig::new(false)?;
    rig.step(120, &[(100, 4)])?;
    let edges: Vec<_> = (0..128)
        .map(|i| (200 + i, if i < 127 { 4 } else { 0 }))
        .collect();
    rig.step(350, &edges)?;
    let before = rig.sim.now;
    let rejected: Vec<_> = (0..129).map(|i| (400 + i, 4)).collect();
    require(
        rig.sim.step(request(550, &rejected)).is_err() && rig.sim.now == before,
        "oversized edge request mutated the simulator",
    )?;
    rig.step(730, &[(600, 4), (700, 0)])?;
    rig.expect(
        "128-edge capacity, 129-edge rejection and retry",
        vec![button(InputKind::Press, 350), button(InputKind::Press, 730)],
        1_000,
    )?;

    // Bypass only the JSON bound to exercise Runtime's defensive InputSource overflow branch.
    let mut rig = Rig::new(false)?;
    rig.step(120, &[(100, 4)])?;
    rig.sim.input.edges = (0..160)
        .map(|i| Edge {
            at_ms: 200 + i,
            levels: if i < 159 { 4 } else { 0 },
        })
        .collect();
    rig.sim.shared.clock.set_ms(400);
    rig.sim.now = 400;
    let tick = rig.sim.run_tick(Fault::Normal);
    let response = Response {
        wire: rig.sim.shared.link.borrow_mut().host_recv().to_vec(),
        frame: Some(
            rig.sim
                .shared
                .capture
                .borrow()
                .frame()
                .iter()
                .map(|p| p.raw())
                .collect(),
        ),
        brightness: rig.sim.shared.capture.borrow().brightness,
        reboot: tick.reboot,
        state: super::state_token(rig.sim.runtime.state()),
    };
    rig.observe(&response)?;
    require(!tick.reboot, "overflow port injection rebooted")?;
    rig.step(630, &[(500, 4), (600, 0)])?;
    rig.expect(
        "input-port overflow retains final release and next press",
        vec![button(InputKind::Press, 400), button(InputKind::Press, 630)],
        1_000,
    )
}

fn session_and_recovery() -> Result<(), String> {
    let mut rig = Rig::new(false)?;
    rig.step(110, &[(100, 4)])?;
    rig.nonce += 1;
    rig.connect(150, false)?;
    rig.step(230, &[(200, 0)])?;
    rig.step(430, &[(300, 4), (400, 0)])?;
    rig.expect(
        "session boundary cancels partial press",
        vec![button(InputKind::Press, 430)],
        800,
    )?;

    let mut rig = Rig::new(false)?;
    rig.step(2_100, &[(100, 4)])?;
    let recovery_hash = rig.frame_hash;
    let mut idle = Rig::new(false)?;
    idle.step(2_100, &[])?;
    require(
        recovery_hash != idle.frame_hash,
        "recovery raw hold did not causally change firmware pixels",
    )?;
    require(
        !rig.step(10_200, &[(2_200, 0)])?,
        "buffered release failed to cancel virtual reboot",
    )?;
    require(
        rig.frame_hash != recovery_hash,
        "release did not clear recovery frame",
    )?;
    rig.expect("buffered recovery release cancels reboot", vec![], 10_500)?;

    let mut rig = Rig::new(false)?;
    rig.step(2_100, &[(100, 4)])?;
    rig.nonce += 1;
    rig.connect(2_200, false)?;
    rig.send(10_099, Message::Ping(Ping { t_ms: 10_099 }))?;
    require(
        !rig.sim.rebooted,
        "virtual reboot occurred before ten-second boundary",
    )?;
    require(
        rig.step(10_100, &[])?,
        "recovery did not survive session renewal to exact reboot boundary",
    )?;
    require(
        rig.events.is_empty(),
        "recovery emitted a normal input action",
    )?;
    require(
        rig.sim.step(request(10_101, &[])).is_err(),
        "rebooted runtime accepted another tick",
    )?;
    let mut restarted = Rig::new(false)?;
    restarted.step(230, &[(100, 4), (200, 0)])?;
    restarted.expect(
        "virtual reboot requires fresh runtime; fresh press works",
        vec![button(InputKind::Press, 230)],
        600,
    )?;
    println!("PASS S5.3 recovery survives session renewal and virtual reboot at 10000ms; no physical reset claim");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn raw_input_scenarios_use_the_real_runtime_and_wire() {
        super::run().unwrap();
    }
}
