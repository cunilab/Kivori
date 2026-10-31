//! Explicit opt-in development sandbox. No USB discovery, real OS services or updater is started.
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::desk::DeskRuntime;
use crate::device::discovery::CandidateRotator;
use crate::device::session::{Session, SessionConfig};
use crate::device::transport::SerialLink;
use crate::device::{ConnectionManager, ManagerEvent};
use crate::input::freshness::Freshness;
use crate::ipc::dto::{
    connection_token, device_internals_dto, DeviceInternalsDto, DiagnosticsSnapshot,
};
use crate::orchestrator::Orchestrator;
use crate::platform::host_events::{HostEvent, HostPresenceTracker, PresenceAction};
use crate::platform::{self, FakeInputSynth, FakeVolumeBackend, OsServices, VolumeBackend};
use crate::runtime::device_task::{
    apply_presence_action, synchronize_session, ConnectionDeadlines, PresenceTargets,
    RotaryPipeline,
};

/// Reuse the single embedded app context, avoiding duplicate macOS plist symbols.
pub fn context() -> tauri::Context<tauri::Wry> {
    super::application_context()
}

mod scenarios;
pub use scenarios::connection_smoke;
mod desktop_scenarios;
pub use desktop_scenarios::desktop_smoke;
mod recovery_scenarios;
pub use recovery_scenarios::recovery_smoke;
mod presence_scenarios;
pub use presence_scenarios::presence_smoke;
mod endurance_scenarios;
pub use endurance_scenarios::endurance_smoke;

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
    fault: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    wire: Vec<u8>,
    frame: Option<Vec<u16>>,
    brightness: Option<u8>,
    reboot: bool,
    state: String,
}

/// The runner's firmware companion states; anything else breaks the contract.
const DEVICE_STATES: [&str; 6] = ["booting", "idle", "happy", "busy", "sleeping", "offline"];

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_response(reader: &mut impl BufRead) -> io::Result<Response> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_LINE + 2) as u64)
        .read_until(b'\n', &mut bytes)?;
    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "virtual child closed its output",
        ));
    }
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
        || !DEVICE_STATES.contains(&response.state.as_str())
    {
        return Err(invalid("virtual response exceeds contract"));
    }
    if response.reboot {
        return Err(invalid(
            "virtual firmware requested reboot; reconnect the virtual device",
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
    fault: &'static str,
    chunk: usize,
    read_remaining: usize,
    outbound: Vec<u8>,
    inbound: VecDeque<u8>,
    /// The firmware companion state from the latest response.
    state: Option<&'static str>,
}

impl ProcessLink {
    fn open(executable: &Path, args: &[&str]) -> io::Result<Self> {
        // Only native CLI supplies this executable; the webview cannot select commands or arguments.
        let executable = executable.canonicalize()?;
        if !executable.is_file() {
            return Err(invalid("virtual runner is not a file"));
        }
        let mut child = Command::new(executable)
            .args(args)
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
                    let result: io::Result<Response> = (|| {
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
            fault: "normal",
            chunk: WIRE_CAPACITY,
            read_remaining: WIRE_CAPACITY,
            outbound: Vec::new(),
            inbound: VecDeque::new(),
            state: None,
        })
    }

    fn exchange(
        &mut self,
        now_ms: u32,
        edges: &mut VecDeque<Edge>,
        capture: bool,
    ) -> io::Result<Option<Vec<u16>>> {
        self.read_remaining = self.chunk;
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
                self.state = DEVICE_STATES
                    .into_iter()
                    .find(|state| *state == response.state);
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
                    wire: self
                        .outbound
                        .drain(..self.outbound.len().min(self.chunk))
                        .collect(),
                    edges: due,
                    capture: self.capture_wanted,
                    fault: self.fault,
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
        let count = buf.len().min(self.inbound.len()).min(self.read_remaining);
        self.read_remaining -= count;
        for byte in &mut buf[..count] {
            *byte = self.inbound.pop_front().expect("bounded count");
        }
        Ok(count)
    }
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let count = buf
            .len()
            .min(WIRE_CAPACITY - self.outbound.len())
            .min(self.chunk);
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
    pub connection_state: &'static str,
    pub generation: u32,
    pub mode: &'static str,
    pub volume: u8,
    /// Fake output mute as read back; `None` when the backend cannot read it.
    pub muted: Option<bool>,
    pub media_triggers: usize,
    pub detents: usize,
    /// The firmware companion state the runner reported last (`None` before any response).
    pub device_state: Option<&'static str>,
    /// The simulated host presence: `active`, `locked` or `sleeping`.
    pub presence: &'static str,
    pub frame: Option<Vec<u16>>,
    pub error: Option<String>,
}

/// A dedicated development owner, never the shipping app's managed state or real device task.
/// Config is in memory only: the tool neither reads nor writes the user's settings.
pub struct Sandbox {
    executable: PathBuf,
    runner_args: &'static [&'static str],
    connection_started: Instant,
    link: Option<ProcessLink>,
    session: Session,
    manager: ConnectionManager,
    orchestrator: Orchestrator,
    rotary: RotaryPipeline,
    desk: DeskRuntime,
    volume: Arc<FakeVolumeBackend>,
    synth: Arc<FakeInputSynth>,
    active_session: Option<(u32, u32)>,
    rejected_inputs: usize,
    started: Instant,
    next_ping: Duration,
    next_capture: Duration,
    /// The Device internals card asked: keep requesting a snapshot about once a second.
    internals_polled: bool,
    next_internals: Duration,
    edges: VecDeque<Edge>,
    held: bool,
    release_by: Option<Duration>,
    snapshot: Snapshot,
    presence: HostPresenceTracker,
    /// The device process while the host has released it (asleep, console lost): it keeps running,
    /// like a unit that stays powered over USB, but no session reads it.
    parked: Option<ProcessLink>,
    // Discovery bookkeeping production presence actions update; the sandbox has one fixed runner.
    connected_port: Option<String>,
    retry_at: Option<Instant>,
    rotator: CandidateRotator,
    deadlines: ConnectionDeadlines,
}

impl Sandbox {
    pub fn open(executable: &Path) -> io::Result<Self> {
        Self::open_with(
            executable,
            &[],
            Arc::new(FakeVolumeBackend::new(50)),
            Arc::new(FakeInputSynth::new(Ok(()))),
            Arc::new(platform::FakeMicBackend::new()),
            Arc::new(platform::FakeMediaObserver::default()),
            Arc::new(platform::FakeForeground::default()),
        )
    }

    fn open_with(
        executable: &Path,
        runner_args: &'static [&'static str],
        volume: Arc<FakeVolumeBackend>,
        synth: Arc<FakeInputSynth>,
        mic: Arc<platform::FakeMicBackend>,
        media: Arc<platform::FakeMediaObserver>,
        foreground: Arc<platform::FakeForeground>,
    ) -> io::Result<Self> {
        let mut link = ProcessLink::open(executable, runner_args)?;
        let mut session = Session::new(SessionConfig::default());
        let mut manager = ConnectionManager::new();
        session
            .open(&mut link, &mut manager)
            .map_err(|_| invalid("virtual handshake could not start"))?;
        let services = OsServices {
            volume: volume.clone(),
            app_volume: Arc::new(platform::FakeAppVolumeBackend::new()),
            mic,
            synth: synth.clone(),
            media,
            system: Box::new(platform::system::NoSystemProbe),
            clock: || None,
            foreground,
        };
        let session_generation = session.connection_generation();
        Ok(Self {
            executable: executable.canonicalize()?,
            runner_args,
            connection_started: Instant::now(),
            link: Some(link),
            session,
            manager,
            orchestrator: Orchestrator::new(),
            rotary: RotaryPipeline::new(volume.as_ref()),
            desk: DeskRuntime::new_simulated(services),
            volume,
            synth,
            active_session: None,
            rejected_inputs: 0,
            started: Instant::now(),
            next_ping: Duration::ZERO,
            next_capture: Duration::ZERO,
            internals_polled: false,
            next_internals: Duration::ZERO,
            edges: VecDeque::from([Edge {
                at_ms: 0,
                levels: 0,
            }]),
            held: false,
            release_by: None,
            snapshot: Snapshot {
                connected: false,
                connection_state: "connecting",
                generation: session_generation,
                mode: "normal",
                volume: 50,
                muted: None,
                media_triggers: 0,
                detents: 0,
                device_state: None,
                presence: "active",
                frame: None,
                error: None,
            },
            presence: HostPresenceTracker::new(),
            parked: None,
            connected_port: None,
            retry_at: None,
            rotator: CandidateRotator::new(),
            deadlines: ConnectionDeadlines::default(),
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.clone()
    }

    /// The Device internals card's data, projected from this session exactly as the shipping app
    /// does. Asking also starts the once-a-second `InternalsRequest`, so nothing is sent until the
    /// card is open.
    pub fn device_internals(&mut self) -> DeviceInternalsDto {
        self.internals_polled = true;
        let mut diagnostics = DiagnosticsSnapshot::initial();
        if self.snapshot.connected {
            diagnostics.connection = connection_token(kivori_model::ConnectionState::Connected);
            diagnostics.connection_generation = self.snapshot.generation;
            diagnostics.effective_caps = self.session.effective_caps();
            diagnostics.internals = self.session.internals();
            diagnostics.internals_received_at = self.session.internals_received_at();
        }
        device_internals_dto(&diagnostics, Instant::now())
    }

    fn end_link(&mut self, error: Option<String>) {
        if let Some(link) = self.link.as_mut() {
            let _ = self
                .session
                .close(link, kivori_protocol::ByeReason::Shutdown);
        }
        self.manager.apply(ManagerEvent::PortRemoved);
        synchronize_session(
            &mut self.active_session,
            &self.session,
            &self.manager,
            &mut self.rotary,
            &mut self.desk,
        );
        self.link = None;
        self.parked = None;
        self.edges.clear();
        self.held = false;
        self.release_by = None;
        self.snapshot.connected = false;
        self.snapshot.connection_state = if error.is_some() {
            "error"
        } else {
            "disconnected"
        };
        self.snapshot.error = error;
        self.snapshot.frame = None;
    }

    /// Fixed development actions only; never accepts a path, process arguments or physical ports.
    pub fn connection(&mut self, action: &str) -> Result<(), String> {
        match action {
            "disconnect" => {
                self.end_link(None);
                return Ok(());
            }
            "silent" | "exit" => {
                if !self.snapshot.connected {
                    return Err("Connect the virtual device first".into());
                }
                let mode = if action == "silent" { "silent" } else { "exit" };
                self.link.as_mut().ok_or("Virtual link closed")?.fault = mode;
                self.snapshot.mode = mode;
                return Ok(());
            }
            _ => {}
        }
        let mode = match action {
            "reconnect" => "normal",
            "fragmented" => "fragmented",
            "wrong-major" => "wrong-major",
            "unknown-product" => "unknown-product",
            "unknown-board" => "unknown-board",
            _ => return Err("Unknown virtual connection action".into()),
        };
        self.end_link(None);
        self.snapshot.mode = mode;
        let result: Result<(), String> = (|| {
            let mut link = ProcessLink::open(&self.executable, self.runner_args)
                .map_err(|error| error.to_string())?;
            if mode == "fragmented" {
                link.chunk = 7;
                link.read_remaining = 7;
            } else {
                link.fault = mode;
            }
            self.manager = ConnectionManager::new();
            self.session
                .open(&mut link, &mut self.manager)
                .map_err(|_| "Virtual handshake could not start")?;
            self.link = Some(link);
            self.connection_started = Instant::now();
            self.next_ping = Duration::ZERO;
            self.next_internals = Duration::ZERO;
            self.next_capture = Duration::ZERO;
            self.snapshot.generation = self.session.connection_generation();
            self.snapshot.connection_state = "connecting";
            Ok(())
        })();
        if let Err(error) = &result {
            self.end_link(Some(error.clone()));
        }
        result
    }

    /// UI supplies control names, never semantic protocol messages or executable commands.
    pub fn input_for_generation(&mut self, control: &str, generation: u32) -> Result<(), String> {
        if generation != self.snapshot.generation {
            return Err("Virtual input belongs to an ended connection".into());
        }
        self.input(control)
    }

    /// Synchronous native harness input; webview commands use the generation guard above.
    pub fn input(&mut self, control: &str) -> Result<(), String> {
        // A released device still has a knob: its edges reach firmware with no session to read them.
        if !self.snapshot.connected && self.parked.is_none() {
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

    /// Feeds one host event through the production presence tracker and actions. Releasing keeps
    /// the device process running (parked); rediscovery reattaches it, or boots a fresh one when
    /// the parked device lost power.
    pub fn host_event(&mut self, event: HostEvent) -> Result<(), String> {
        for action in self.presence.on_event(event) {
            match action {
                // The sandbox has no companion director to pause.
                PresenceAction::PauseSelfPlay(_) => {}
                PresenceAction::ReleaseLink => {
                    // A second release (console lost, then sleep) must keep the parked device, not
                    // drop it and fake a power cut.
                    if let Some(mut link) = self.link.take() {
                        link.inbound.clear();
                        self.parked = Some(link);
                    }
                    self.apply_presence(action);
                    synchronize_session(
                        &mut self.active_session,
                        &self.session,
                        &self.manager,
                        &mut self.rotary,
                        &mut self.desk,
                    );
                    self.held = false;
                    self.release_by = None;
                    self.snapshot.connected = false;
                    self.snapshot.connection_state = "disconnected";
                }
                PresenceAction::RediscoverNow => {
                    self.apply_presence(action);
                    if self.presence.may_use_port() && self.link.is_none() {
                        self.reattach()?;
                    }
                }
                other => {
                    if self.apply_presence(other) {
                        self.end_link(Some("virtual presence write failed".into()));
                    }
                }
            }
        }
        self.snapshot.presence = self.presence.presence().token();
        Ok(())
    }

    fn apply_presence(&mut self, action: PresenceAction) -> bool {
        apply_presence_action(
            action,
            &mut PresenceTargets {
                session: &mut self.session,
                manager: &mut self.manager,
                orchestrator: &mut self.orchestrator,
                link: &mut self.link,
                connected_port: &mut self.connected_port,
                retry_at: &mut self.retry_at,
                rotator: &mut self.rotator,
                deadlines: &mut self.deadlines,
            },
        )
    }

    fn reattach(&mut self) -> Result<(), String> {
        let mut link = match self.parked.take() {
            Some(link) => link,
            None => ProcessLink::open(&self.executable, self.runner_args)
                .map_err(|error| error.to_string())?,
        };
        link.inbound.clear();
        self.manager = ConnectionManager::new();
        self.session
            .open(&mut link, &mut self.manager)
            .map_err(|_| "Virtual handshake could not start")?;
        self.link = Some(link);
        self.connection_started = Instant::now();
        self.next_ping = Duration::ZERO;
        self.next_internals = Duration::ZERO;
        self.next_capture = Duration::ZERO;
        self.snapshot.generation = self.session.connection_generation();
        self.snapshot.connection_state = "connecting";
        Ok(())
    }

    /// Runs the released device with no session: its output goes nowhere, as with a closed port.
    fn tick_parked(&mut self) -> io::Result<()> {
        let now = self.started.elapsed();
        let ms = u32::try_from(now.as_millis())
            .map_err(|_| invalid("restart virtual device after clock limit"))?;
        let capture = now >= self.next_capture;
        if capture {
            self.next_capture = now + Duration::from_millis(100);
        }
        let parked = self
            .parked
            .as_mut()
            .ok_or_else(|| invalid("no parked device"))?;
        if let Some(frame) = parked.exchange(ms, &mut self.edges, capture)? {
            self.snapshot.frame = Some(frame);
        }
        parked.inbound.clear();
        self.snapshot.device_state = parked.state;
        Ok(())
    }

    pub fn tick(&mut self) {
        if self.link.is_none() {
            if self.parked.is_some() {
                if let Err(error) = self.tick_parked() {
                    self.end_link(Some(error.to_string()));
                }
            }
            return;
        }
        if self
            .release_by
            .is_some_and(|deadline| self.started.elapsed() >= deadline)
        {
            let _ = self.input("release");
        }
        if let Err(error) = self.tick_inner() {
            self.end_link(Some(error.to_string()));
        }
        self.snapshot.volume = self.volume.read().unwrap_or(0);
        self.snapshot.muted = self.volume.read_mute().ok();
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
        self.snapshot.device_state = link.state;
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
        if self.manager.state() == kivori_model::ConnectionState::Incompatible {
            return Err(invalid(
                self.manager
                    .incompatible_reason()
                    .unwrap_or("virtual protocol incompatible"),
            ));
        }
        if self
            .session
            .identity()
            .is_some_and(|identity| identity.resolve().is_none())
        {
            return Err(invalid("virtual product or board identity is unknown"));
        }
        self.snapshot.connected = self.manager.state().can_drive_device()
            && self.session.identity().is_some()
            && self.session.can_sync_state();
        if !self.snapshot.connected {
            if self.connection_started.elapsed() > Duration::from_secs(5) {
                return Err(invalid("virtual handshake rejected or timed out"));
            }
            return Ok(());
        }
        self.snapshot.connection_state = "connected";
        if now >= self.next_ping {
            self.session
                .send_ping(link, ms)
                .map_err(|_| invalid("virtual ping failed"))?;
            self.next_ping = now + Duration::from_secs(1);
        }
        if self.internals_polled && now >= self.next_internals {
            self.session
                .request_internals(link)
                .map_err(|_| invalid("virtual internals request failed"))?;
            self.next_internals = now + Duration::from_secs(1);
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
            |activity| {
                if activity.kind == crate::activity::ActivityEventKind::InputStaleSessionRejected {
                    self.rejected_inputs += 1;
                }
            },
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
            fault: "normal",
            chunk: WIRE_CAPACITY,
            read_remaining: WIRE_CAPACITY,
            outbound: Vec::new(),
            inbound: VecDeque::new(),
            state: None,
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
                state: "idle".into(),
            }))
            .unwrap();
        link.exchange(105, &mut edges, false).unwrap();
        assert!(
            requests.try_recv().unwrap().capture,
            "capture demand must survive a busy pipe"
        );
        assert!(!link.capture_wanted);
    }

    #[test]
    fn fragmentation_and_backpressure_preserve_bounded_ordered_bytes() {
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
            fault: "normal",
            chunk: 7,
            read_remaining: 7,
            outbound: Vec::new(),
            inbound: VecDeque::new(),
            state: None,
        };
        let bytes: Vec<_> = (0..WIRE_CAPACITY).map(|n| (n % 251) as u8).collect();
        let mut sent = 0;
        while sent < bytes.len() {
            sent += link.write(&bytes[sent..]).unwrap();
        }
        assert_eq!(link.write(&[99]).unwrap(), 0);
        assert_eq!(link.outbound.len(), WIRE_CAPACITY);
        let mut session = Session::new(SessionConfig::default());
        assert!(matches!(
            session.open(&mut link, &mut ConnectionManager::new()),
            Err(crate::device::SessionError::WriteZero)
        ));
        assert!(session.current_session().is_none());
        let mut edges = VecDeque::new();
        link.exchange(0, &mut edges, false).unwrap();
        assert_eq!(requests.try_recv().unwrap().wire, bytes[..7]);
        assert_eq!(link.outbound.len(), WIRE_CAPACITY - 7);
        responses
            .send(Ok(Response {
                wire: (0..100).collect(),
                frame: None,
                brightness: None,
                reboot: false,
                state: "idle".into(),
            }))
            .unwrap();
        link.exchange(1, &mut edges, false).unwrap();
        assert_eq!(requests.try_recv().unwrap().wire, bytes[7..14]);
        let mut read = [0; 16];
        assert_eq!(link.read(&mut read).unwrap(), 7);
        assert_eq!(&read[..7], &[0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(
            link.read(&mut read).unwrap(),
            0,
            "per-tick receive budget is bounded"
        );
        link.exchange(2, &mut edges, false).unwrap();
        assert_eq!(link.read(&mut read).unwrap(), 7);
        assert_eq!(&read[..7], &[7, 8, 9, 10, 11, 12, 13]);
        responses
            .send(Ok(Response {
                wire: vec![0; WIRE_CAPACITY],
                frame: None,
                brightness: None,
                reboot: false,
                state: "idle".into(),
            }))
            .unwrap();
        assert_eq!(
            link.exchange(3, &mut edges, false).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            link.inbound.len(),
            86,
            "overflow must fail before extending the queue"
        );
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
        let mut link = ProcessLink::open(&script, &[]).expect("spawn hung child");
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
            b"{\"wire\":[],\"frame\":[0],\"brightness\":null,\"reboot\":false,\"state\":\"idle\"}\n"
                .to_vec(),
            // Missing or unknown firmware state.
            b"{\"wire\":[],\"frame\":null,\"brightness\":null,\"reboot\":false}\n".to_vec(),
            b"{\"wire\":[],\"frame\":null,\"brightness\":null,\"reboot\":false,\"state\":\"asleep\"}\n"
                .to_vec(),
        ] {
            assert!(read_response(&mut io::Cursor::new(bytes)).is_err());
        }
        let good =
            b"{\"wire\":[],\"frame\":null,\"brightness\":null,\"reboot\":false,\"state\":\"sleeping\"}\n";
        assert!(read_response(&mut io::Cursor::new(good)).is_ok());
    }
}
