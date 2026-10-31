//! Bounded USB diagnostic. Never flashes or resets. `session` uses the app's idle-state resync;
//! `desk` also runs the M1 desk pipeline on this computer's real OS services for 10 s, cycling the
//! device through every display mode, and reports what was sent (no actions are run).
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, Message, MAX_FRAME, MAX_WIRE,
    PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port_name = std::env::args()
        .nth(1)
        .ok_or("usage: device_probe PORT [timeout-ms]")?;
    if std::env::args().nth(2).as_deref() == Some("session") {
        return probe_session(&port_name);
    }
    if std::env::args().nth(2).as_deref() == Some("desk") {
        return probe_desk(&port_name, Forward::StatusOnly);
    }
    // Everything the app sends after connecting (labels, mic, media, buddy settings), like the
    // device thread; `desk-no-labels` leaves the control labels out to isolate them.
    if std::env::args().nth(2).as_deref() == Some("desk-full") {
        return probe_desk(&port_name, Forward::All);
    }
    if std::env::args().nth(2).as_deref() == Some("desk-no-labels") {
        return probe_desk(&port_name, Forward::AllButLabels);
    }
    // Prints every input event the device sends for 20 s, or KIVORI_PROBE_SECS (turn right, then left).
    if std::env::args().nth(2).as_deref() == Some("inputs") {
        return probe_inputs(&port_name);
    }
    // Samples the device's own load numbers (busy %, compose/flush time, fps) once a second.
    if std::env::args().nth(2).as_deref() == Some("internals") {
        return probe_internals(&port_name);
    }
    // The device thread's connect burst in one instant (state, status, media, labels, mic, buddy).
    if std::env::args().nth(2).as_deref() == Some("desk-burst") {
        return probe_desk(&port_name, Forward::Burst);
    }
    let timeout_ms: u64 = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "100".into())
        .parse()?;
    let mut port = serialport::new(port_name, 115_200)
        .timeout(Duration::from_millis(timeout_ms))
        .open()?;
    let nonce = 1;
    let hello = Message::Hello(Hello {
        desktop_version: FirmwareVersion {
            major: 0,
            minor: 0,
            patch: 0,
        },
        desktop_caps: Capabilities::NONE,
        nonce,
    });
    let mut wire = heapless::Vec::<u8, MAX_WIRE>::new();
    encode_message(
        &hello,
        ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
        0,
        &mut wire,
    )
    .unwrap();
    port.write_all(&wire)?;
    println!("Hello sent; waiting up to 5 seconds (serial timeout {timeout_ms} ms).");
    let started = Instant::now();
    let mut packet = Vec::new();
    let mut total = 0;
    let mut invalid = 0;
    while started.elapsed() < Duration::from_secs(5) {
        let mut bytes = [0; 256];
        let count = match port.read(&mut bytes) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => 0,
            Err(e) => return Err(e.into()),
        };
        total += count;
        for &byte in &bytes[..count] {
            if byte == 0 {
                let mut scratch = heapless::Vec::<u8, MAX_FRAME>::new();
                match decode_message(&packet, &mut scratch, &[PROTOCOL_MAJOR]) {
                    Ok((_, Message::HelloAck(ack))) if ack.nonce_echo == nonce => {
                        println!(
                            "Verified HelloAck: firmware {}.{}.{}, after {} ms.",
                            ack.firmware_version.major,
                            ack.firmware_version.minor,
                            ack.firmware_version.patch,
                            started.elapsed().as_millis()
                        );
                        return Ok(());
                    }
                    Ok((_, message)) => println!(
                        "Received {}.",
                        match message {
                            Message::HelloAck(_) => "HelloAck with a different nonce",
                            Message::StateReport(_) => "StateReport",
                            Message::Ping(_) => "Ping",
                            Message::Pong(_) => "Pong",
                            Message::Health(_) => "Health",
                            Message::Diagnostic(_) => "Diagnostic",
                            Message::Error(error) => {
                                println!(
                                    "Protocol error: {:?}, code {}",
                                    error.category, error.code
                                );
                                "Error"
                            }
                            Message::Bye(_) => "Bye",
                            Message::Hello(_) => "Hello",
                            Message::Ready(_) => "Ready",
                            Message::SetState(_) => "SetState",
                            Message::PlayMascotAction(_) => "PlayMascotAction",
                            Message::MascotActionApplied(_) => "MascotActionApplied",
                            Message::InputEvent(_) => "InputEvent",
                            Message::Presentation(_) => "Presentation",
                            Message::Status(_) => "Status",
                            Message::Feedback(_) => "Feedback",
                            Message::MediaInfo(_) => "MediaInfo",
                            Message::ControlLabels(_) => "ControlLabels",
                            Message::InternalsRequest(_) => "InternalsRequest",
                            Message::Internals(_) => "Internals",
                            Message::BuddySettings(_) => "BuddySettings",
                            Message::MicState(_) => "MicState",
                            Message::Identity(_) => "Identity",
                            Message::BuildInfo(_) => "BuildInfo",
                        }
                    ),
                    Err(_) if !packet.is_empty() => invalid += 1,
                    Err(_) => {}
                }
                packet.clear();
            } else if packet.len() < MAX_WIRE {
                packet.push(byte);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Err(format!(
        "No matching HelloAck: {total} bytes received, {invalid} invalid frames, {} pending bytes.",
        packet.len()
    )
    .into())
}

fn probe_session(port: &str) -> Result<(), Box<dyn std::error::Error>> {
    use kivori_desktop::device::{
        fsm::ConnectionManager,
        serial::SerialPortLink,
        session::{Session, SessionConfig},
    };
    use kivori_desktop::orchestrator::Orchestrator;
    struct Trace(SerialPortLink, Vec<u8>);
    impl kivori_desktop::device::transport::SerialLink for Trace {
        type Error = std::io::Error;
        fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Self::Error> {
            let count = self.0.read(bytes)?;
            if count > 0 {
                println!("Serial read: {count} bytes");
            }
            for &byte in &bytes[..count] {
                if byte == 0 {
                    let mut scratch = heapless::Vec::<u8, MAX_FRAME>::new();
                    match decode_message(&self.1, &mut scratch, &[PROTOCOL_MAJOR]) {
                        Ok((header, Message::HelloAck(ack))) => {
                            println!("HelloAck seq {}, nonce {}", header.seq, ack.nonce_echo)
                        }
                        Ok((header, _)) => println!("Valid frame seq {}", header.seq),
                        Err(e) => println!("Rejected frame: {e:?}"),
                    }
                    self.1.clear();
                } else if self.1.len() < MAX_WIRE {
                    self.1.push(byte);
                }
            }
            Ok(count)
        }
        fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
            let count = self.0.write(bytes)?;
            println!("Serial write: {count}/{} bytes", bytes.len());
            Ok(count)
        }
    }
    let mut link = Trace(SerialPortLink::open(port)?, Vec::new());
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    session
        .open(&mut link, &mut manager)
        .map_err(|e| format!("Open: {e:?}"))?;
    let start = Instant::now();
    let mut handshake_deadline = Duration::from_secs(5);
    let mut next_ping = Duration::from_secs(1);
    let mut verified = false;
    while start.elapsed() < Duration::from_secs(8) {
        session
            .pump(&mut link, &mut manager, &mut orchestrator)
            .map_err(|e| format!("Pump: {e:?}"))?;
        if !verified && start.elapsed() >= handshake_deadline {
            use kivori_desktop::device::fsm::ManagerEvent;
            manager.apply(ManagerEvent::HandshakeTimeout);
            manager.apply(ManagerEvent::BackoffElapsed);
            println!("Retrying the handshake after startup.");
            session
                .open(&mut link, &mut manager)
                .map_err(|e| format!("Retry: {e:?}"))?;
            handshake_deadline = start.elapsed() + Duration::from_secs(5);
        }
        if manager.state().can_drive_device() {
            if !verified {
                println!(
                    "Production serial session Connected after {} ms.",
                    start.elapsed().as_millis()
                );
                verified = true;
            }
            if start.elapsed() >= next_ping {
                if session.heartbeat_timed_out() {
                    return Err("Heartbeat timed out".into());
                }
                session
                    .send_ping(&mut link, start.elapsed().as_millis() as u32)
                    .map_err(|e| format!("Ping: {e:?}"))?;
                next_ping = start.elapsed() + Duration::from_secs(1);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !verified {
        return Err("Production serial session received no handshake".into());
    }
    if session.heartbeat_timed_out() {
        return Err("Heartbeat timed out".into());
    }
    println!(
        "Connection and heartbeats stable for 8 seconds; reported state: {:?}.",
        session.reported()
    );
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Forward {
    StatusOnly,
    AllButLabels,
    All,
    Burst,
}

fn probe_desk(port: &str, forward: Forward) -> Result<(), Box<dyn std::error::Error>> {
    use kivori_desktop::desk::DeskRuntime;
    use kivori_desktop::device::{
        fsm::ConnectionManager,
        serial::SerialPortLink,
        session::{Session, SessionConfig},
    };
    use kivori_desktop::orchestrator::Orchestrator;
    use kivori_model::desk::DisplayMode;

    let mut link = SerialPortLink::open(port)?;
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    session
        .open(&mut link, &mut manager)
        .map_err(|e| format!("Open: {e:?}"))?;
    let built = Instant::now();
    // No main-thread runner: this probe never synthesizes input.
    let mut desk = DeskRuntime::new(kivori_desktop::platform::os_services(None));
    println!("OS services ready in {} ms.", built.elapsed().as_millis());
    let start = Instant::now();
    let (mut sent, mut modes) = (0, DisplayMode::ALL.into_iter().cycle());
    let mut next_mode = Duration::from_secs(1);
    let mut next_ping = Duration::from_secs(1);
    let mut connected = false;
    let mut buddy_sent = false;
    while start.elapsed() < Duration::from_secs(12) {
        session
            .pump(&mut link, &mut manager, &mut orchestrator)
            .map_err(|e| format!("Pump: {e:?}"))?;
        if !manager.state().can_drive_device() {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        if !connected {
            connected = true;
            desk.on_session_begin();

            println!(
                "Connected after {} ms; negotiated caps {:#x}.",
                start.elapsed().as_millis(),
                session.negotiated_caps().bits()
            );
        }
        let now = start.elapsed();
        if now >= next_mode {
            next_mode = now + Duration::from_secs(2);
            let mode = modes.next().expect("cycle");
            desk.set_mode(mode, &mut |_| {});
            println!("Mode -> {mode:?}");
        }
        let out = desk.tick(now, &mut |_| {});
        // Like the device thread: buddy/display settings once the session has negotiated them
        // (in `Burst` mode only together with the first labels, as the app does).
        if forward != Forward::StatusOnly && forward != Forward::Burst && !buddy_sent {
            buddy_sent = session
                .send_buddy_settings(
                    &mut link,
                    kivori_model::mascot::MascotPersonality::default(),
                    true,
                    5,
                )
                .map_err(|e| format!("Buddy settings: {e:?}"))?;
            if buddy_sent {
                println!("Buddy settings sent at {} ms", now.as_millis());
            }
        }
        if forward != Forward::StatusOnly {
            if let Some(media) = out.media_info {
                session
                    .send_media_info(&mut link, media)
                    .map_err(|e| format!("Media: {e:?}"))?;
                println!("Media info sent");
            }
            if let Some(mic) = out.mic {
                session
                    .send_mic_state(&mut link, mic)
                    .map_err(|e| format!("Mic: {e:?}"))?;
                println!("Mic state sent");
            }
            if let Some(labels) = out.controls.filter(|_| forward != Forward::AllButLabels) {
                session
                    .send_control_labels(&mut link, labels)
                    .map_err(|e| format!("Labels: {e:?}"))?;
                println!("Control labels sent");
                if forward == Forward::Burst && !buddy_sent {
                    buddy_sent = session
                        .send_buddy_settings(
                            &mut link,
                            kivori_model::mascot::MascotPersonality::default(),
                            true,
                            5,
                        )
                        .map_err(|e| format!("Buddy settings: {e:?}"))?;
                    println!("Buddy settings sent in the burst: {buddy_sent}");
                }
            }
        }
        if let Some(status) = out.status {
            if session
                .send_status(&mut link, status)
                .map_err(|e| format!("Status: {e:?}"))?
            {
                sent += 1;
                println!("Status sent: {status:?}");
            }
        }
        if now >= next_ping {
            if session.heartbeat_timed_out() {
                return Err("Heartbeat timed out".into());
            }
            session
                .send_ping(&mut link, now.as_millis() as u32)
                .map_err(|e| format!("Ping: {e:?}"))?;
            next_ping = now + Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if sent == 0 {
        return Err(format!(
            "No desk status was sent; connection state {:?}, caps {:#x}",
            manager.state(),
            session.negotiated_caps().bits()
        )
        .into());
    }
    println!("{sent} statuses sent; link healthy throughout.");
    Ok(())
}

/// Records the device's input events through the production session, so a person can check which
/// way a physical turn is reported (`Detent(Cw)` means clockwise to the firmware).
fn probe_inputs(port: &str) -> Result<(), Box<dyn std::error::Error>> {
    use kivori_desktop::device::{
        fsm::ConnectionManager,
        serial::SerialPortLink,
        session::{Session, SessionConfig},
    };
    use kivori_desktop::orchestrator::Orchestrator;

    let mut link = SerialPortLink::open(port)?;
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    session
        .open(&mut link, &mut manager)
        .map_err(|e| format!("Open: {e:?}"))?;
    let start = Instant::now();
    let mut next_ping = Duration::from_secs(1);
    let mut announced = false;
    let (mut cw, mut ccw) = (0i32, 0i32);
    let secs = std::env::var("KIVORI_PROBE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    while start.elapsed() < Duration::from_secs(secs) {
        session
            .pump(&mut link, &mut manager, &mut orchestrator)
            .map_err(|e| format!("Pump: {e:?}"))?;
        if manager.state().can_drive_device() && !announced {
            announced = true;
            println!("Connected. Turn the knob RIGHT 3 clicks, wait, then LEFT 3 clicks.");
        }
        for event in session.take_input_events() {
            match event.kind {
                kivori_protocol::InputKind::Detent(kivori_model::input::Direction::Cw) => cw += 1,
                kivori_protocol::InputKind::Detent(kivori_model::input::Direction::Ccw) => ccw += 1,
                _ => {}
            }
            println!(
                "{:>6} ms  {:?} {:?}",
                start.elapsed().as_millis(),
                event.control,
                event.kind
            );
        }
        let now = start.elapsed();
        if manager.state().can_drive_device() && now >= next_ping {
            session
                .send_ping(&mut link, now.as_millis() as u32)
                .map_err(|e| format!("Ping: {e:?}"))?;
            next_ping = now + Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    println!("Detent totals: firmware clockwise (Cw) {cw}, counter-clockwise (Ccw) {ccw}.");
    Ok(())
}

/// Prints the device's load figures once a second for `KIVORI_PROBE_SECS` (default 12) and their
/// average, so a firmware change can be measured on the real unit without the app.
fn probe_internals(port: &str) -> Result<(), Box<dyn std::error::Error>> {
    use kivori_desktop::device::{
        fsm::ConnectionManager,
        serial::SerialPortLink,
        session::{Session, SessionConfig},
    };
    use kivori_desktop::orchestrator::Orchestrator;

    let mut link = SerialPortLink::open(port)?;
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    session
        .open(&mut link, &mut manager)
        .map_err(|e| format!("Open: {e:?}"))?;
    let secs = std::env::var("KIVORI_PROBE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    let start = Instant::now();
    let (mut next, mut samples) = (Duration::from_secs(2), Vec::new());
    while start.elapsed() < Duration::from_secs(secs) {
        session
            .pump(&mut link, &mut manager, &mut orchestrator)
            .map_err(|e| format!("Pump: {e:?}"))?;
        let now = start.elapsed();
        if manager.state().can_drive_device() && now >= next {
            next = now + Duration::from_secs(1);
            if let Some(i) = session.internals() {
                println!(
                    "busy {:?}%  compose {:?} ms  flush {:?} ms  fps {:?}  edge high-water {:?}  \
                     stack min free {:?}  ram free {:?} static {:?} total {:?}",
                    i.busy_percent,
                    i.compose_ms,
                    i.flush_ms,
                    i.fps,
                    i.edge_queue_high_water,
                    i.stack_free_min_bytes,
                    i.ram_free_bytes,
                    i.ram_static_bytes,
                    i.ram_total_bytes
                );
                if let (Some(c), Some(f)) = (i.compose_ms, i.fps) {
                    samples.push((u32::from(c), u32::from(f)));
                }
            }
            session
                .send_ping(&mut link, now.as_millis() as u32)
                .map_err(|e| format!("Ping: {e:?}"))?;
            session
                .request_internals(&mut link)
                .map_err(|e| format!("Internals: {e:?}"))?;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let n = u32::try_from(samples.len()).unwrap_or(1).max(1);
    println!(
        "average over {} samples: compose {} ms, fps {}",
        samples.len(),
        samples.iter().map(|s| s.0).sum::<u32>() / n,
        samples.iter().map(|s| s.1).sum::<u32>() / n
    );
    Ok(())
}
