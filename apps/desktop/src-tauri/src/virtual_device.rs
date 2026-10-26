//! Explicit opt-in development sandbox. No USB discovery, real OS services or updater is started.
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::desk::DeskRuntime;
use crate::device::session::{Session, SessionConfig};
use crate::device::transport::SerialLink;
use crate::device::ConnectionManager;
use crate::input::freshness::Freshness;
use crate::orchestrator::Orchestrator;
use crate::platform::{self, FakeInputSynth, FakeVolumeBackend, OsServices, VolumeBackend};
use crate::runtime::device_task::{synchronize_session, RotaryPipeline};

/// Reuse the single embedded app context, avoiding duplicate macOS plist symbols.
pub fn context() -> tauri::Context<tauri::Wry> {
    super::application_context()
}

const MAX_LINE: usize = 400_000;
const WIRE_CAPACITY: usize = 8192;
const EDGE_CAPACITY: usize = 128;
const FRAME_PIXELS: usize = 240 * 240;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize)]
struct Edge {
    at_ms: u32,
    levels: u8,
}

#[derive(Serialize)]
struct Request {
    now_ms: u32,
    wire: Vec<u8>,
    edges: Vec<Edge>,
    capture: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    wire: Vec<u8>,
    frame: Option<Vec<u16>>,
    brightness: Option<u8>,
    reboot: bool,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_response(reader: &mut impl BufRead) -> io::Result<Response> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_LINE + 2) as u64)
        .read_until(b'\n', &mut bytes)?;
    if bytes.last() != Some(&b'\n') || bytes.len() > MAX_LINE + 1 {
        return Err(invalid("virtual response truncated or oversized"));
    }
    let response: Response =
        serde_json::from_slice(&bytes).map_err(|_| invalid("invalid virtual response"))?;
    if response.wire.len() > WIRE_CAPACITY
        || response
            .frame
            .as_ref()
            .is_some_and(|frame| frame.len() != FRAME_PIXELS)
        || response
            .brightness
            .is_some_and(|step| !(1..=kivori_model::desk::BRIGHTNESS_STEPS).contains(&step))
    {
        return Err(invalid("virtual response exceeds contract"));
    }
    if response.reboot {
        return Err(invalid(
            "virtual firmware requested reboot; restart the sandbox",
        ));
    }
    Ok(response)
}

/// Owns exactly one local child. The I/O worker has one bounded request and response slot.
struct ProcessLink {
    child: Child,
    requests: Option<SyncSender<Request>>,
    responses: Receiver<io::Result<Response>>,
    worker: Option<JoinHandle<()>>,
    pending: Option<Instant>,
    capture_wanted: bool,
    outbound: Vec<u8>,
    inbound: VecDeque<u8>,
}

impl ProcessLink {
    fn open(executable: &Path) -> io::Result<Self> {
        // Only native CLI supplies this executable; the webview cannot select commands or arguments.
        let executable = executable.canonicalize()?;
        if !executable.is_file() {
            return Err(invalid("virtual runner is not a file"));
        }
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (responses, rx) = mpsc::sync_channel(1);
        let worker = match std::thread::Builder::new()
            .name("virtual-pipe".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    let result = (|| {
                        let bytes = serde_json::to_vec(&request)?;
                        if bytes.len() > MAX_LINE {
                            return Err(invalid("virtual request oversized"));
                        }
                        stdin.write_all(&bytes)?;
                        stdin.write_all(b"\n")?;
                        stdin.flush()?;
                        read_response(&mut stdout)
                    })();
                    let failed = result.is_err();
                    if responses.try_send(result).is_err() || failed {
                        break;
                    }
                }
            }) {
            Ok(worker) => worker,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        Ok(Self {
            child,
            requests: Some(tx),
            responses: rx,
            worker: Some(worker),
            pending: None,
            capture_wanted: false,
            outbound: Vec::new(),
            inbound: VecDeque::new(),
        })
    }

    fn exchange(
        &mut self,
        now_ms: u32,
        edges: &mut VecDeque<Edge>,
        capture: bool,
    ) -> io::Result<Option<Vec<u16>>> {
        self.capture_wanted |= capture;
        let mut frame = None;
        match self.responses.try_recv() {
            Ok(result) => {
                self.pending = None;
                let response = result?;
                if self.inbound.len() + response.wire.len() > WIRE_CAPACITY {
                    return Err(invalid("virtual receive queue full"));
                }
                self.inbound.extend(response.wire);
                frame = response.frame;
            }
            Err(TryRecvError::Disconnected) => return Err(invalid("virtual child I/O closed")),
            Err(TryRecvError::Empty) => {}
        }
        if self.pending.is_some_and(|at| at.elapsed() > IO_TIMEOUT) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "virtual child did not respond",
            ));
        }
        if self.pending.is_none() {
            let mut due = Vec::new();
            while edges.front().is_some_and(|edge| edge.at_ms <= now_ms) {
                due.push(edges.pop_front().expect("front checked"));
            }
            self.requests
                .as_ref()
                .expect("open sender")
                .try_send(Request {
                    now_ms,
                    wire: std::mem::take(&mut self.outbound),
                    edges: due,
                    capture: self.capture_wanted,
                })
                .map_err(|_| invalid("virtual request queue unavailable"))?;
            self.capture_wanted = false;
            self.pending = Some(Instant::now());
        }
        Ok(frame)
    }
}

impl SerialLink for ProcessLink {
    type Error = io::Error;
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = buf.len().min(self.inbound.len());
        for byte in &mut buf[..count] {
            *byte = self.inbound.pop_front().expect("bounded count");
        }
        Ok(count)
    }
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let count = buf.len().min(WIRE_CAPACITY - self.outbound.len());
        self.outbound.extend_from_slice(&buf[..count]);
        Ok(count)
    }
}

impl Drop for ProcessLink {
    fn drop(&mut self) {
        self.requests.take();
        // Kill closes blocked pipe reads/writes before joining the worker; always reap the child.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub connected: bool,
    pub volume: u8,
    pub media_triggers: usize,
    pub detents: usize,
    pub frame: Option<Vec<u16>>,
    pub error: Option<String>,
}

/// A dedicated development owner, never the shipping app's managed state or real device task.
/// Config is in memory only: the tool neither reads nor writes the user's settings.
pub struct Sandbox {
    link: Option<ProcessLink>,
    session: Session,
    manager: ConnectionManager,
    orchestrator: Orchestrator,
    rotary: RotaryPipeline,
    desk: DeskRuntime,
    volume: Arc<FakeVolumeBackend>,
    synth: Arc<FakeInputSynth>,
    active_session: Option<(u32, u32)>,
    started: Instant,
    next_ping: Duration,
    next_capture: Duration,
    edges: VecDeque<Edge>,
    held: bool,
    release_by: Option<Duration>,
    snapshot: Snapshot,
}

impl Sandbox {
    pub fn open(executable: &Path) -> io::Result<Self> {
        let mut link = ProcessLink::open(executable)?;
        let mut session = Session::new(SessionConfig::default());
        let mut manager = ConnectionManager::new();
        session
            .open(&mut link, &mut manager)
            .map_err(|_| invalid("virtual handshake could not start"))?;
        let volume = Arc::new(FakeVolumeBackend::new(50));
        let synth = Arc::new(FakeInputSynth::new(Ok(())));
        let services = OsServices {
            volume: volume.clone(),
            app_volume: Arc::new(platform::FakeAppVolumeBackend::new()),
            mic: Arc::new(platform::FakeMicBackend::new()),
            synth: synth.clone(),
            media: Arc::new(platform::FakeMediaObserver::default()),
            system: Box::new(platform::system::NoSystemProbe),
            clock: || None,
            foreground: Arc::new(platform::FakeForeground::default()),
        };
        Ok(Self {
            link: Some(link),
            session,
            manager,
            orchestrator: Orchestrator::new(),
            rotary: RotaryPipeline::new(volume.as_ref()),
            desk: DeskRuntime::new_simulated(services),
            volume,
            synth,
            active_session: None,
            started: Instant::now(),
            next_ping: Duration::ZERO,
            next_capture: Duration::ZERO,
            edges: VecDeque::from([Edge {
                at_ms: 0,
                levels: 0,
            }]),
            held: false,
            release_by: None,
            snapshot: Snapshot {
                connected: false,
                volume: 50,
                media_triggers: 0,
                detents: 0,
                frame: None,
                error: None,
            },
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.clone()
    }

    /// UI supplies control names, never semantic protocol messages or executable commands.
    pub fn input(&mut self, control: &str) -> Result<(), String> {
        if !self.snapshot.connected {
            return Err("Virtual device is not connected".into());
        }
        if self.edges.len() + 5 > EDGE_CAPACITY {
            return Err("Virtual input queue is full".into());
        }
        let now = u32::try_from(self.started.elapsed().as_millis())
            .map_err(|_| "Restart virtual device after clock limit")?;
        let at = self.edges.back().map_or(now, |edge| edge.at_ms.max(now));
        let held = if self.held { 4 } else { 0 };
        match control {
            "clockwise" | "counterclockwise" => {
                let phases = if control == "clockwise" {
                    [2, 3, 1, 0]
                } else {
                    [1, 3, 2, 0]
                };
                for (i, phase) in phases.into_iter().enumerate() {
                    self.edges.push_back(Edge {
                        at_ms: at + i as u32 + 1,
                        levels: held | phase,
                    });
                }
            }
            "tap" if !self.held => {
                self.edges.push_back(Edge {
                    at_ms: at + 1,
                    levels: 4,
                });
                self.edges.push_back(Edge {
                    at_ms: at + 121,
                    levels: 0,
                });
            }
            "tap" => return Err("Release the held knob first".into()),
            "press" if !self.held => {
                self.held = true;
                self.release_by = Some(self.started.elapsed() + Duration::from_secs(2));
                self.edges.push_back(Edge {
                    at_ms: at + 1,
                    levels: 4,
                });
            }
            "release" => {
                self.held = false;
                self.release_by = None;
                self.edges.push_back(Edge {
                    at_ms: at + 1,
                    levels: 0,
                });
            }
            "press" => {}
            _ => return Err("Unknown virtual control".into()),
        }
        Ok(())
    }

    pub fn tick(&mut self) {
        if self.snapshot.error.is_some() {
            return;
        }
        if self
            .release_by
            .is_some_and(|deadline| self.started.elapsed() >= deadline)
        {
            let _ = self.input("release");
        }
        if let Err(error) = self.tick_inner() {
            self.snapshot.connected = false;
            self.snapshot.error = Some(error.to_string());
            self.desk.on_session_end();
            self.link = None;
            self.edges.clear();
        }
        self.snapshot.volume = self.volume.read().unwrap_or(0);
        self.snapshot.media_triggers = self.synth.sent.lock().expect("fake synth").len();
    }

    fn tick_inner(&mut self) -> io::Result<()> {
        let now = self.started.elapsed();
        let ms = u32::try_from(now.as_millis())
            .map_err(|_| invalid("restart virtual device after clock limit"))?;
        let link = self
            .link
            .as_mut()
            .ok_or_else(|| invalid("virtual link closed"))?;
        let capture = now >= self.next_capture;
        if capture {
            self.next_capture = now + Duration::from_millis(100);
        }
        if let Some(frame) = link.exchange(ms, &mut self.edges, capture)? {
            self.snapshot.frame = Some(frame);
        }
        self.session.set_host_ms(ms);
        self.session.set_host_instant(Instant::now());
        self.session
            .pump(link, &mut self.manager, &mut self.orchestrator)
            .map_err(|_| invalid("virtual protocol failure"))?;
        synchronize_session(
            &mut self.active_session,
            &self.session,
            &self.manager,
            &mut self.rotary,
            &mut self.desk,
        );
        self.snapshot.connected = self.manager.state().can_drive_device()
            && self.session.identity().is_some()
            && self.session.can_sync_state();
        if !self.snapshot.connected {
            if now > Duration::from_secs(5) {
                return Err(invalid("virtual handshake rejected or timed out"));
            }
            return Ok(());
        }
        if now >= self.next_ping {
            self.session
                .send_ping(link, ms)
                .map_err(|_| invalid("virtual ping failed"))?;
            self.next_ping = now + Duration::from_secs(1);
        }
        if self.session.heartbeat_timed_out() {
            return Err(invalid("virtual heartbeat timeout"));
        }
        let inputs = self.session.take_input_events();
        self.snapshot.detents += inputs
            .iter()
            .filter(|event| matches!(event.kind, kivori_protocol::InputKind::Detent(_)))
            .count();
        let mut presentations = Vec::new();
        let freshness = Freshness {
            clock: self.session.device_clock(),
            host_now_ms: ms,
        };
        self.rotary.accept_inputs_fresh(
            &inputs,
            freshness,
            self.volume.as_ref(),
            &mut presentations,
            |input, remaining| {
                self.desk
                    .on_input_within(&input, now, remaining, &mut |_| {})
            },
            |_| {},
        );
        for presentation in presentations {
            self.session
                .send_presentation(link, presentation)
                .map_err(|_| invalid("virtual presentation failed"))?;
        }
        let output = self.desk.tick(now, &mut |_| {});
        if let Some(status) = output.status {
            self.session
                .send_status(link, status)
                .map_err(|_| invalid("virtual status failed"))?;
        }
        for feedback in output.feedback {
            self.session
                .send_feedback(link, feedback)
                .map_err(|_| invalid("virtual feedback failed"))?;
        }
        if let Some(labels) = output.controls {
            self.session
                .send_control_labels(link, labels)
                .map_err(|_| invalid("virtual labels failed"))?;
        }
        if let Some(mic) = output.mic {
            self.session
                .send_mic_state(link, mic)
                .map_err(|_| invalid("virtual mic state failed"))?;
        }
        if let Some(media) = output.media_info {
            self.session
                .send_media_info(link, media)
                .map_err(|_| invalid("virtual media failed"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_due_during_pending_io_is_latched_until_enqueued() {
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let (tx, requests) = mpsc::sync_channel(1);
        let (responses, rx) = mpsc::sync_channel(1);
        let mut link = ProcessLink {
            child,
            requests: Some(tx),
            responses: rx,
            worker: None,
            pending: None,
            capture_wanted: false,
            outbound: Vec::new(),
            inbound: VecDeque::new(),
        };
        let mut edges = VecDeque::new();
        link.exchange(0, &mut edges, false).unwrap();
        assert!(!requests.try_recv().unwrap().capture);
        link.exchange(100, &mut edges, true).unwrap();
        assert!(requests.try_recv().is_err());
        responses
            .send(Ok(Response {
                wire: Vec::new(),
                frame: None,
                brightness: None,
                reboot: false,
            }))
            .unwrap();
        link.exchange(105, &mut edges, false).unwrap();
        assert!(
            requests.try_recv().unwrap().capture,
            "capture demand must survive a busy pipe"
        );
        assert!(!link.capture_wanted);
    }

    #[cfg(unix)]
    #[test]
    fn hung_child_times_out_and_drop_unblocks_and_reaps_it() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temporary runner directory");
        let script = dir.path().join("hung-runner");
        std::fs::write(&script, "#!/bin/sh\nexec sleep 30\n").expect("test runner");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))
            .expect("executable");
        let mut link = ProcessLink::open(&script).expect("spawn hung child");
        link.exchange(0, &mut VecDeque::new(), false)
            .expect("queued tick");
        link.pending = Some(Instant::now() - IO_TIMEOUT - Duration::from_millis(1));
        assert_eq!(
            link.exchange(1, &mut VecDeque::new(), false)
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        let at = Instant::now();
        drop(link);
        assert!(
            at.elapsed() < Duration::from_secs(2),
            "child termination must unblock I/O worker"
        );
    }

    #[test]
    fn pipe_contract_rejects_oversize_truncation_and_wrong_frame() {
        for bytes in [
            vec![b'x'; MAX_LINE + 2],
            b"{}".to_vec(),
            b"{\"wire\":[],\"frame\":[0],\"brightness\":null,\"reboot\":false}\n".to_vec(),
        ] {
            assert!(read_response(&mut io::Cursor::new(bytes)).is_err());
        }
        let good = b"{\"wire\":[],\"frame\":null,\"brightness\":null,\"reboot\":false}\n";
        assert!(read_response(&mut io::Cursor::new(good)).is_ok());
    }
}
