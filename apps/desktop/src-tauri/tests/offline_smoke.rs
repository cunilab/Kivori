//! Offline host smoke test (T109; FR-029, SC-006).
//!
//! Exercises the whole host-side product path with **no external services and no network**: the native
//! core state, the host-sim transport, the bundled preview renderer, the handshake, a desired-state
//! update, and the reconnect/resync path. Everything runs in-process over an in-memory byte pipe and
//! against the asset blob compiled into the binary, so nothing here can reach a socket. The test also
//! asserts startup does not wait on a remote timeout, and that the frontend references no remote assets.

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kivori_desktop::activity::{ActivityEventKind, ActivityLog, ActivityMetadata};
use kivori_desktop::device::fsm::{ConnectionManager, ManagerEvent};
use kivori_desktop::device::session::{Session, SessionConfig};
use kivori_desktop::device::transport::SerialLink;
use kivori_desktop::ipc::dto;
use kivori_desktop::orchestrator::Orchestrator;
// The preview renderer ships only with Device Studio; the release feature set skips step 2 below.
#[cfg(feature = "device-studio")]
use kivori_desktop::render::render_preview_bundled;
use kivori_desktop::runtime::state::{AppState, DeviceCommand};
#[cfg(feature = "device-studio")]
use kivori_model::CompanionState;
use kivori_model::{Capabilities, ConnectionState, ProtocolVersion, SendableState};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, HelloAck, Message, MAX_FRAME, MAX_WIRE,
    PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

/// Anything slower than this at startup would imply waiting on something remote. A real DNS/TCP
/// timeout is seconds; the whole offline path must complete in a fraction of that.
const NO_REMOTE_WAIT: Duration = Duration::from_secs(3);

/// In-memory host-sim link — the offline stand-in for the USB serial port.
#[derive(Default)]
struct SimLink {
    to_device: Vec<u8>,
    from_device: Vec<u8>,
}

impl SerialLink for SimLink {
    type Error = Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Infallible> {
        let n = buf.len().min(self.from_device.len());
        buf[..n].copy_from_slice(&self.from_device[..n]);
        self.from_device.drain(..n);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        self.to_device.extend_from_slice(buf);
        Ok(buf.len())
    }
}

fn wire_version() -> ProtocolVersion {
    ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
}

/// The simulated device answers with a `HelloAck` echoing `nonce`.
fn device_reply(link: &mut SimLink, nonce: u32, seq: u16) {
    let ack = Message::HelloAck(HelloAck {
        device_caps: Capabilities::NONE,
        device_id: [0x33; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        nonce_echo: nonce,
    });
    let mut wire: heapless::Vec<u8, MAX_WIRE> = heapless::Vec::new();
    encode_message(&ack, wire_version(), seq, &mut wire).expect("encode");
    link.from_device.extend_from_slice(&wire);
}

/// Decodes everything the desktop transmitted, clearing the buffer.
fn desktop_sent(link: &mut SimLink) -> Vec<Message> {
    let bytes = std::mem::take(&mut link.to_device);
    let mut scratch: heapless::Vec<u8, MAX_FRAME> = heapless::Vec::new();
    let mut out = Vec::new();
    for packet in bytes.split(|&b| b == 0) {
        if packet.is_empty() {
            continue;
        }
        if let Ok((_, msg)) = decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
            out.push(msg);
        }
    }
    out
}

fn hello_nonce(msgs: &[Message]) -> u32 {
    msgs.iter()
        .find_map(|m| match m {
            Message::Hello(h) => Some(h.nonce),
            _ => None,
        })
        .expect("desktop sent Hello")
}

fn set_state_desired(msgs: &[Message]) -> Option<SendableState> {
    msgs.iter().find_map(|m| match m {
        Message::SetState(s) => Some(s.desired),
        _ => None,
    })
}

/// Builds managed app state around a stand-in device thread (no serial port, no network).
fn offline_app_state() -> AppState {
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel::<DeviceCommand>();
    let thread_cancel = Arc::clone(&cancel);
    let thread = std::thread::spawn(move || {
        while !thread_cancel.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = rx.recv_timeout(Duration::from_millis(5));
        }
    });
    AppState::new(
        true,
        Arc::new(Mutex::new(dto::initial_status())),
        Arc::new(ActivityLog::new(64)),
        tx,
        cancel,
        thread,
    )
}

#[test]
fn offline_core_starts_connects_updates_and_reconnects_without_network() {
    let start = Instant::now();

    // 1. Native core construction — managed state + initial snapshot, no I/O of any kind.
    let app = offline_app_state();
    let snapshot = app.status_snapshot();
    assert_eq!(snapshot.connection, "disconnected");
    assert_eq!(
        snapshot.desired, "idle",
        "cold start is idle (no persistence)"
    );

    // 2. Preview renderer — every scene renders from the blob compiled into this binary. No SVG parsed
    //    at runtime, no asset fetched (Constitution XI, FR-021).
    #[cfg(feature = "device-studio")]
    for state in CompanionState::ALL {
        let rgba = render_preview_bundled(state, 0);
        assert_eq!(rgba.len(), 240 * 240 * 4, "full frame for {state:?}");
    }

    // 3. Handshake over the host-sim transport.
    let mut link = SimLink::default();
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    let mut session = Session::new(SessionConfig::default());

    session.open(&mut link, &mut manager).expect("open session");
    assert_eq!(manager.state(), ConnectionState::Connecting);
    let nonce = hello_nonce(&desktop_sent(&mut link));
    device_reply(&mut link, nonce, 0);
    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump handshake");
    assert_eq!(
        manager.state(),
        ConnectionState::Connected,
        "connected offline"
    );
    assert!(manager.device().is_some(), "identity via handshake only");
    let _ = desktop_sent(&mut link);

    // 4. Desired-state update reaches the wire as a semantic SetState (never pixels; FR-015).
    session
        .set_desired(&mut link, &manager, &mut orchestrator, SendableState::Busy)
        .expect("set desired");
    assert_eq!(
        set_state_desired(&desktop_sent(&mut link)),
        Some(SendableState::Busy)
    );

    // 5. Reconnect path — unplug, reopen, and the desired state is resynchronised (FR-009).
    manager.apply(ManagerEvent::PortRemoved);
    assert_eq!(manager.state(), ConnectionState::Disconnected);
    session.open(&mut link, &mut manager).expect("reopen");
    let nonce = hello_nonce(&desktop_sent(&mut link));
    device_reply(&mut link, nonce, 0);
    session
        .pump(&mut link, &mut manager, &mut orchestrator)
        .expect("pump reconnect");
    assert_eq!(manager.state(), ConnectionState::Connected);
    assert_eq!(
        set_state_desired(&desktop_sent(&mut link)),
        Some(SendableState::Busy),
        "reconnect resyncs the desired state"
    );

    // 6. Activity is recorded in process memory only.
    app.activity_log.record(
        ActivityEventKind::ConnectionStateChanged,
        Some(ActivityMetadata::Connection {
            state: manager.state(),
            retry_count: manager.retry_count(),
            elapsed_ms: 10,
        }),
    );
    assert_eq!(app.activity_log.recent(8).len(), 1);

    app.shutdown();

    // 7. No step waited on a remote timeout.
    let elapsed = start.elapsed();
    assert!(
        elapsed < NO_REMOTE_WAIT,
        "offline path must not wait on anything remote (took {elapsed:?})"
    );
}

#[test]
fn frontend_assets_are_local_only() {
    // Fonts, icons, scripts, styles, and images must be bundled — no remote origin may appear in the
    // shipped webview sources. Mirrors scripts/check-frontend-offline.mjs so the guarantee also holds
    // inside the Rust smoke test.
    let frontend = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("apps/desktop")
        .to_path_buf();

    let mut checked = 0usize;
    let mut offenders = Vec::new();
    let mut scan = |path: &Path| {
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        checked += 1;
        for (lineno, line) in text.lines().enumerate() {
            for token in line.split_whitespace() {
                let Some(idx) = token.find("http") else {
                    continue;
                };
                let url = &token[idx..];
                if !url.starts_with("http") {
                    continue;
                }
                let local = url.contains("localhost") || url.contains("127.0.0.1");
                // Doc links inside comments are not shipped assets; only real asset/URL references are.
                let is_reference = line.contains("src=")
                    || line.contains("href=")
                    || line.contains("url(")
                    || line.contains("fetch(");
                if is_reference && !local {
                    offenders.push(format!("{}:{}: {url}", path.display(), lineno + 1));
                }
            }
        }
    };

    scan(&frontend.join("index.html"));
    let mut stack = vec![frontend.join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ts" | "tsx" | "css" | "html")
            ) {
                scan(&path);
            }
        }
    }

    assert!(checked > 0, "frontend sources were found and scanned");
    assert!(
        offenders.is_empty(),
        "frontend must reference only local assets: {offenders:?}"
    );
}
