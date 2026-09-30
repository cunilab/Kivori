//! Bounded USB diagnostic. Never flashes or resets. `session` uses the app's idle-state resync.
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, Message, MAX_FRAME, MAX_WIRE,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port_name = std::env::args()
        .nth(1)
        .ok_or("usage: device_probe PORT [timeout-ms]")?;
    if std::env::args().nth(2).as_deref() == Some("session") {
        return probe_session(&port_name);
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
    encode_message(&hello, ProtocolVersion::new(1, 0), 0, &mut wire).unwrap();
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
                match decode_message(&packet, &mut scratch, &[1]) {
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
                    match decode_message(&self.1, &mut scratch, &[1]) {
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
