//! A blocking PulseAudio-protocol client over libpulse's threaded mainloop. PipeWire serves the same
//! protocol through `pipewire-pulse`, so this is the Linux audio API for both.
//!
//! THREADING: libpulse runs its own mainloop thread. Every libpulse object here (the `Mainloop`
//! handle, the `Context`, every `Operation`) is created on, and only ever touched from, ONE Kivori
//! thread (`kivori-audio` or `kivori-mic`), and always with the mainloop lock held, so Rust never
//! holds two overlapping references to any of them. Callbacks run on libpulse's thread and touch NONE
//! of those objects: they only store an answer in a slot they own and bump a [`Gate`] (a plain
//! `Mutex` + `Condvar`). The owner issues a request, then waits on the gate WITH A DEADLINE (it
//! releases the mainloop lock while waiting) until the callback, an operation state change or a
//! context state change wakes it. The subscribe callback is INGRESS ONLY: it posts a coarse
//! [`Event`] on a channel and returns; all reads and rebinding happen on the owning Kivori thread.
//!
//! DEADLINES: connecting is bounded by [`CONNECT_DEADLINE`] and every operation by [`OP_DEADLINE`].
//! A silent server, a cancelled operation or a lost context never leaves the owner asleep: the
//! request fails and, after a timeout, the connection is treated as lost (`is_ready` is false) so
//! the owner reconnects instead of queueing more requests behind a stalled one.
//!
//! A context that is lost (the server restarted) is reported by [`Conn::is_ready`]; the owner then
//! drops it and connects a new one. Connecting never autospawns a daemon.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::introspect::{ServerInfo, SinkInfo, SinkInputInfo, SourceInfo};
use libpulse_binding::context::subscribe::{Facility, InterestMaskSet};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::operation::{Operation, State as OperationState};
use libpulse_binding::volume::{ChannelVolumes, Volume};

/// Why a backend is unavailable when the server cannot be reached (shown in the UI).
pub(super) const UNREACHABLE: &str = "the audio server (PulseAudio or PipeWire) isn't reachable";

/// How often a lost connection is retried.
const RECONNECT: Duration = Duration::from_secs(3);
/// The longest connecting to the server may take. A local socket answers in milliseconds, so this
/// only ever fires for a server that accepts the socket and never speaks.
pub(super) const CONNECT_DEADLINE: Duration = Duration::from_millis(1500);
/// The longest one request (a read or a write) may wait for its answer.
pub(super) const OP_DEADLINE: Duration = Duration::from_secs(1);
/// The longest a caller waits for an owning thread to answer one command. A command is a few
/// requests, and a stalled one fails fast after the first timeout, so this is only a backstop. It
/// matches the macOS script timeout.
pub(super) const CALL_DEADLINE: Duration = Duration::from_secs(3);
/// The longest dropping a backend waits for its thread before detaching it.
pub(super) const JOIN_DEADLINE: Duration = Duration::from_secs(3);

/// What libpulse says changed. Coarse on purpose: the owner re-reads what it cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Event {
    Sink,
    Source,
    SinkInput,
    SourceOutput,
    Server,
}

/// An output device.
#[derive(Debug, Clone)]
pub(super) struct Sink {
    pub index: u32,
    pub name: String,
    pub volume: ChannelVolumes,
    pub mute: bool,
}

/// One app's stream on an output device.
#[derive(Debug, Clone)]
pub(super) struct SinkInput {
    pub index: u32,
    pub sink: u32,
    /// `application.process.binary`.
    pub binary: Option<String>,
    /// `application.name`.
    pub app_name: Option<String>,
    pub volume: ChannelVolumes,
    pub mute: bool,
}

/// A capture device or the monitor of an output device.
#[derive(Debug, Clone)]
pub(super) struct Source {
    pub index: u32,
    /// The server's stable name for the device: its identity across re-reads and hot-plugs.
    pub name: String,
    /// The name shown to the user (user data, never logged).
    pub description: String,
    pub mute: bool,
    /// Whether this is an output's "monitor" (what is playing), not a microphone.
    pub is_monitor: bool,
}

/// One app's stream from a capture device.
#[derive(Debug, Clone)]
pub(super) struct SourceOutput {
    pub source: u32,
    pub corked: bool,
}

/// `0..=100` percent to a libpulse volume. 100 is `Volume::NORMAL` (0 dB), the "100 %" of every
/// desktop mixer; the mixers' own slider is linear in this value, so the same scale is used here.
pub(super) fn volume_of(percent: u8) -> Volume {
    let percent = u64::from(percent.min(100));
    Volume(u32::try_from((percent * u64::from(Volume::NORMAL.0) + 50) / 100).unwrap_or(u32::MAX))
}

/// A libpulse volume to `0..=100` percent, rounded. Above 100 % (libpulse allows amplification)
/// reads as 100: the value range of this product is 0..=100.
pub(super) fn percent_of(volume: Volume) -> u8 {
    let normal = u64::from(Volume::NORMAL.0);
    ((u64::from(volume.0) * 100 + normal / 2) / normal).min(100) as u8
}

/// The loudest channel as a percentage: what a mixer's single slider shows.
pub(super) fn channels_percent(volume: &ChannelVolumes) -> u8 {
    percent_of(volume.max())
}

/// `volume` with its loudest channel at `percent`, keeping the balance between channels (a user's
/// left/right balance survives a volume change). Silent channels are all set to the value.
pub(super) fn with_percent(volume: &ChannelVolumes, percent: u8) -> ChannelVolumes {
    let target = volume_of(percent);
    let mut out = *volume;
    if out.scale(target).is_none() {
        out.set(out.len().max(1), target);
    }
    out
}

/// What wakes a waiting owner. Callbacks (libpulse's thread) call [`Gate::notify`]; the owner calls
/// [`Gate::wait_change`]. A generation counter, read BEFORE the state is checked (with the mainloop
/// lock held, so no callback can run in between), means a wake-up is never lost, and the wait itself
/// is timed, which `pa_threaded_mainloop_wait` is not.
#[derive(Default)]
struct Gate {
    generation: Mutex<u64>,
    changed: Condvar,
}

impl Gate {
    fn lock(&self) -> MutexGuard<'_, u64> {
        self.generation
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn generation(&self) -> u64 {
        *self.lock()
    }

    fn notify(&self) {
        *self.lock() += 1;
        self.changed.notify_all();
    }

    /// Waits until something notified after `seen` or `deadline` passes. `true` when notified.
    fn wait_change(&self, seen: u64, deadline: Instant) -> bool {
        let mut generation = self.lock();
        while *generation == seen {
            let Some(left) = deadline
                .checked_duration_since(Instant::now())
                .filter(|left| !left.is_zero())
            else {
                return false;
            };
            generation = self
                .changed
                .wait_timeout(generation, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }
}

/// How a wait for one operation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Done,
    /// libpulse cancelled it (the connection was lost).
    Cancelled,
    /// The deadline passed first; the operation was cancelled.
    TimedOut,
}

/// What the owning thread holds while the mainloop lock is held.
struct Session<'a> {
    ctx: &'a mut Context,
    ml: &'a mut Mainloop,
    gate: &'a Arc<Gate>,
    stalled: &'a mut bool,
}

impl Session<'_> {
    /// Waits (lock held, released while asleep) for `op` to finish, for at most [`OP_DEADLINE`]. The
    /// wake-ups are the callbacks, the operation's own state change (cancellation) and the context
    /// state callback. A timeout marks the connection stalled.
    fn finish<T: ?Sized>(&mut self, op: &mut Operation<T>) -> Outcome {
        let wake = Arc::clone(self.gate);
        op.set_state_callback(Some(Box::new(move || wake.notify())));
        let deadline = Instant::now() + OP_DEADLINE;
        let outcome = loop {
            let seen = self.gate.generation();
            match op.get_state() {
                OperationState::Done => break Outcome::Done,
                OperationState::Cancelled => break Outcome::Cancelled,
                OperationState::Running => {}
            }
            if Instant::now() >= deadline {
                op.cancel();
                *self.stalled = true;
                break Outcome::TimedOut;
            }
            self.ml.unlock();
            self.gate.wait_change(seen, deadline);
            self.ml.lock();
        };
        op.set_state_callback(None);
        outcome
    }
}

/// One connection to the audio server, owned by one thread.
pub(super) struct Conn {
    ctx: Option<Context>,
    ml: Mainloop,
    gate: Arc<Gate>,
    /// A request timed out: the server is not answering, so the connection counts as lost.
    stalled: bool,
}

fn event_of(facility: Facility) -> Option<Event> {
    match facility {
        Facility::Sink => Some(Event::Sink),
        Facility::Source => Some(Event::Source),
        Facility::SinkInput => Some(Event::SinkInput),
        Facility::SourceOutput => Some(Event::SourceOutput),
        Facility::Server => Some(Event::Server),
        _ => None,
    }
}

fn interest_of(events: &[Event]) -> InterestMaskSet {
    events.iter().fold(InterestMaskSet::NULL, |mask, event| {
        mask | match event {
            Event::Sink => InterestMaskSet::SINK,
            Event::Source => InterestMaskSet::SOURCE,
            Event::SinkInput => InterestMaskSet::SINK_INPUT,
            Event::SourceOutput => InterestMaskSet::SOURCE_OUTPUT,
            Event::Server => InterestMaskSet::SERVER,
        }
    })
}

impl Conn {
    /// Connects and subscribes to `events`, calling `on_event` (on libpulse's thread: post and
    /// return) for each. `Err` carries a short reason.
    pub fn connect(
        client: &str,
        events: &[Event],
        on_event: impl Fn(Event) + 'static,
    ) -> Result<Self, String> {
        Self::connect_to(None, client, events, on_event)
    }

    /// [`Conn::connect`] to a specific server (`None` is the default one), within
    /// [`CONNECT_DEADLINE`].
    pub fn connect_to(
        server: Option<&str>,
        client: &str,
        events: &[Event],
        on_event: impl Fn(Event) + 'static,
    ) -> Result<Self, String> {
        let mut ml = Mainloop::new().ok_or("cannot create the audio mainloop")?;
        let gate = Arc::new(Gate::default());
        let mut ctx = Context::new(&ml, client).ok_or("cannot create the audio context")?;
        // Kept for the life of the connection: a later FAILED or TERMINATED wakes any waiter.
        let wake = Arc::clone(&gate);
        ctx.set_state_callback(Some(Box::new(move || wake.notify())));

        let deadline = Instant::now() + CONNECT_DEADLINE;
        ml.lock();
        let started = ctx
            .connect(server, FlagSet::NOAUTOSPAWN, None)
            .map_err(|e| format!("{e}"))
            .and_then(|()| ml.start().map_err(|e| format!("{e}")));
        let result = started.and_then(|()| loop {
            let seen = gate.generation();
            match ctx.get_state() {
                State::Ready => break Ok(()),
                State::Failed | State::Terminated => {
                    break Err("the audio server is not reachable".to_string())
                }
                _ if Instant::now() >= deadline => {
                    break Err("the audio server did not answer in time".to_string())
                }
                _ => {
                    ml.unlock();
                    gate.wait_change(seen, deadline);
                    ml.lock();
                }
            }
        });
        if let Err(reason) = result {
            ctx.set_state_callback(None);
            ctx.disconnect();
            drop(ctx);
            ml.unlock();
            ml.stop();
            return Err(reason);
        }
        ctx.set_subscribe_callback(Some(Box::new(move |facility, _, _| {
            if let Some(event) = facility.and_then(event_of) {
                on_event(event);
            }
        })));
        // The reply only says the subscription was registered; events follow it.
        drop(ctx.subscribe(interest_of(events), |_| {}));
        ml.unlock();
        Ok(Self {
            ctx: Some(ctx),
            ml,
            gate,
            stalled: false,
        })
    }

    /// Whether the context is still connected and answering.
    pub fn is_ready(&mut self) -> bool {
        self.locked(|session| session.ctx.get_state() == State::Ready)
            .unwrap_or(false)
    }

    /// Runs `f` with the mainloop lock held. `None` when the connection is gone or has stalled.
    fn locked<R>(&mut self, f: impl FnOnce(&mut Session<'_>) -> R) -> Option<R> {
        if self.stalled {
            return None;
        }
        let ctx = self.ctx.as_mut()?;
        self.ml.lock();
        let mut session = Session {
            ctx,
            ml: &mut self.ml,
            gate: &self.gate,
            stalled: &mut self.stalled,
        };
        let out = f(&mut session);
        self.ml.unlock();
        Some(out)
    }

    /// The default output device, `None` when there is none or the server does not answer.
    pub fn default_sink(&mut self) -> Option<Sink> {
        // `@DEFAULT_SINK@` is resolved by the server (PulseAudio and pipewire-pulse): one request.
        self.sink("@DEFAULT_SINK@").or_else(|| {
            let name = self.default_sink_name()?;
            self.sink(&name)
        })
    }

    fn default_sink_name(&mut self) -> Option<String> {
        let slot: Arc<Mutex<Option<String>>> = Arc::default();
        let out = Arc::clone(&slot);
        let outcome = self.locked(|session| {
            let signal = Signal(Arc::clone(session.gate));
            let mut op = session
                .ctx
                .introspect()
                .get_server_info(move |info: &ServerInfo<'_>| {
                    *out.lock().expect("server info slot") =
                        info.default_sink_name.as_ref().map(ToString::to_string);
                    signal.signal();
                });
            session.finish(&mut op)
        })?;
        let name = slot.lock().expect("server info slot").take();
        name.filter(|n| !n.is_empty())
            .filter(|_| outcome == Outcome::Done)
    }

    /// An output device by name (`@DEFAULT_SINK@` is accepted).
    pub fn sink(&mut self, name: &str) -> Option<Sink> {
        let slot: Arc<Mutex<Option<Sink>>> = Arc::default();
        let out = Arc::clone(&slot);
        let name = name.to_string();
        let outcome = self.locked(|session| {
            let signal = Signal(Arc::clone(session.gate));
            let mut op = session
                .ctx
                .introspect()
                .get_sink_info_by_name(&name, move |item| match item {
                    ListResult::Item(info) => *out.lock().expect("sink slot") = Some(sink_of(info)),
                    ListResult::End | ListResult::Error => signal.signal(),
                });
            session.finish(&mut op)
        })?;
        let found = slot.lock().expect("sink slot").take();
        found.filter(|_| outcome == Outcome::Done)
    }

    /// Every app stream on any output device.
    pub fn sink_inputs(&mut self) -> Option<Vec<SinkInput>> {
        self.list(|intro, push, signal| {
            intro.get_sink_input_info_list(move |item| match item {
                ListResult::Item(info) => push(sink_input_of(info)),
                ListResult::End | ListResult::Error => signal.signal(),
            })
        })
    }

    /// Every capture device and monitor.
    pub fn sources(&mut self) -> Option<Vec<Source>> {
        self.list(|intro, push, signal| {
            intro.get_source_info_list(move |item| match item {
                ListResult::Item(info) => push(source_of(info)),
                ListResult::End | ListResult::Error => signal.signal(),
            })
        })
    }

    /// Every app stream from a capture device.
    pub fn source_outputs(&mut self) -> Option<Vec<SourceOutput>> {
        self.list(|intro, push, signal| {
            intro.get_source_output_info_list(move |item| match item {
                ListResult::Item(info) => push(SourceOutput {
                    source: info.source,
                    corked: info.corked,
                }),
                ListResult::End | ListResult::Error => signal.signal(),
            })
        })
    }

    /// Runs a list request: `start` gets the introspector, a function that stores one item, and a
    /// handle to signal completion. `None` when the request was cancelled (connection lost) or
    /// timed out.
    fn list<T: Send + 'static, O: ?Sized>(
        &mut self,
        start: impl FnOnce(
            &libpulse_binding::context::introspect::Introspector,
            Box<dyn Fn(T)>,
            Signal,
        ) -> Operation<O>,
    ) -> Option<Vec<T>> {
        let slot: Arc<Mutex<Vec<T>>> = Arc::default();
        let out = Arc::clone(&slot);
        let outcome = self.locked(|session| {
            let push: Box<dyn Fn(T)> =
                Box::new(move |item| out.lock().expect("list slot").push(item));
            let mut op = start(
                &session.ctx.introspect(),
                push,
                Signal(Arc::clone(session.gate)),
            );
            session.finish(&mut op)
        })?;
        let items = std::mem::take(&mut *slot.lock().expect("list slot"));
        (outcome == Outcome::Done).then_some(items)
    }

    /// Runs a write request and waits for the server to answer it. The answer only says whether the
    /// server accepted it: callers read the state back instead of trusting it.
    fn write(
        &mut self,
        start: impl FnOnce(
            &mut libpulse_binding::context::introspect::Introspector,
            Option<Box<dyn FnMut(bool)>>,
        ) -> Operation<dyn FnMut(bool)>,
    ) -> bool {
        self.locked(|session| {
            let signal = Signal(Arc::clone(session.gate));
            let mut op = start(
                &mut session.ctx.introspect(),
                Some(Box::new(move |_| signal.signal())),
            );
            session.finish(&mut op) == Outcome::Done
        })
        .unwrap_or(false)
    }

    pub fn set_sink_volume(&mut self, name: &str, volume: &ChannelVolumes) -> bool {
        self.write(|intro, cb| intro.set_sink_volume_by_name(name, volume, cb))
    }

    pub fn set_sink_mute(&mut self, name: &str, mute: bool) -> bool {
        self.write(|intro, cb| intro.set_sink_mute_by_name(name, mute, cb))
    }

    pub fn set_sink_input_volume(&mut self, index: u32, volume: &ChannelVolumes) -> bool {
        self.write(|intro, cb| intro.set_sink_input_volume(index, volume, cb))
    }

    pub fn set_sink_input_mute(&mut self, index: u32, mute: bool) -> bool {
        self.write(|intro, cb| intro.set_sink_input_mute(index, mute, cb))
    }

    pub fn set_source_mute(&mut self, index: u32, mute: bool) -> bool {
        self.write(|intro, cb| intro.set_source_mute_by_index(index, mute, cb))
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        if let Some(mut ctx) = self.ctx.take() {
            self.ml.lock();
            ctx.set_state_callback(None);
            ctx.set_subscribe_callback(None);
            ctx.disconnect();
            // Releases the callbacks while the lock is held.
            drop(ctx);
            self.ml.unlock();
        }
        // The lock must not be held to stop the mainloop thread. Nothing blocks inside a callback,
        // so the thread always reaches its quit check and this returns.
        self.ml.stop();
    }
}

/// A connection that is rebuilt when lost: the owner calls [`Link::ensure`] before each use.
pub(super) struct Link {
    conn: Option<Conn>,
    last_attempt: Option<Instant>,
    events: &'static [Event],
    on_event: Arc<dyn Fn(Event) + Send + Sync>,
}

impl Link {
    pub fn new(events: &'static [Event], on_event: Arc<dyn Fn(Event) + Send + Sync>) -> Self {
        Self {
            conn: None,
            last_attempt: None,
            events,
            on_event,
        }
    }

    /// Drops a lost connection and, at most every few seconds, tries to make a new one. `true` when
    /// a new connection was just made (the owner must rebind what it had).
    pub fn ensure(&mut self) -> bool {
        if self.conn.as_mut().is_some_and(|c| !c.is_ready()) {
            self.conn = None;
        }
        if self.conn.is_some() || self.last_attempt.is_some_and(|at| at.elapsed() < RECONNECT) {
            return false;
        }
        self.last_attempt = Some(Instant::now());
        let on_event = Arc::clone(&self.on_event);
        self.conn = Conn::connect("Kivori", self.events, move |event| on_event(event)).ok();
        self.conn.is_some()
    }

    pub fn conn(&mut self) -> Option<&mut Conn> {
        self.conn.as_mut()
    }

    pub fn is_down(&self) -> bool {
        self.conn.is_none()
    }
}

/// A handle callbacks use to wake the waiting owner.
#[derive(Clone)]
pub(super) struct Signal(Arc<Gate>);

impl Signal {
    pub fn signal(&self) {
        self.0.notify();
    }
}

/// Waits for `thread` to finish for at most `within`; past that it is left running (detached) and
/// `false` is returned, so dropping a backend can never hang on a stuck thread.
pub(super) fn join_within(thread: JoinHandle<()>, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while !thread.is_finished() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = thread.join();
    true
}

fn sink_of(info: &SinkInfo<'_>) -> Sink {
    Sink {
        index: info.index,
        name: info.name.as_deref().unwrap_or_default().to_string(),
        volume: info.volume,
        mute: info.mute,
    }
}

fn sink_input_of(info: &SinkInputInfo<'_>) -> SinkInput {
    SinkInput {
        index: info.index,
        sink: info.sink,
        binary: info.proplist.get_str("application.process.binary"),
        app_name: info.proplist.get_str("application.name"),
        volume: info.volume,
        mute: info.mute,
    }
}

fn source_of(info: &SourceInfo<'_>) -> Source {
    let name = info.name.as_deref().unwrap_or_default().to_string();
    Source {
        index: info.index,
        description: info
            .description
            .as_deref()
            .filter(|d| !d.is_empty())
            .map_or_else(|| name.clone(), ToString::to_string),
        is_monitor: info.monitor_of_sink.is_some() || name.ends_with(".monitor"),
        mute: info.mute,
        name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal fake PulseAudio server: it speaks just enough of the native protocol (AUTH and
    // SET_CLIENT_NAME) to bring a client to READY, then behaves as asked. This is the "stalled
    // transport" the deadlines are tested against, using the real libpulse client.
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;

    #[derive(Clone, Copy, PartialEq)]
    enum Then {
        /// Answer nothing more: every request hangs.
        Stall,
        /// Close the socket after the handshake: the context fails.
        HangUp,
        /// Never even answer the handshake.
        Silent,
    }

    struct FakeServer {
        path: String,
        _dir: tempfile::TempDir,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl FakeServer {
        fn start(then: Then) -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("pulse.sock");
            let listener = UnixListener::bind(&path).expect("bind");
            let thread = std::thread::spawn(move || {
                let Ok((mut socket, _)) = listener.accept() else {
                    return;
                };
                let mut answered = 0;
                loop {
                    let mut head = [0u8; 20];
                    if socket.read_exact(&mut head).is_err() {
                        return;
                    }
                    let length = u32::from_be_bytes(head[0..4].try_into().unwrap()) as usize;
                    let mut body = vec![0u8; length];
                    if socket.read_exact(&mut body).is_err() {
                        return;
                    }
                    if then == Then::Silent || answered >= 2 {
                        continue;
                    }
                    // Tagstruct: 'L' command, 'L' tag. Reply: 'L' REPLY(2), 'L' tag, 'L' value
                    // (the protocol version for AUTH, a client index for SET_CLIENT_NAME).
                    let tag = &body[6..10];
                    let value: u32 = if answered == 0 { 13 } else { 1 };
                    let mut reply = Vec::new();
                    reply.extend_from_slice(&15u32.to_be_bytes());
                    reply.extend_from_slice(&u32::MAX.to_be_bytes());
                    reply.extend_from_slice(&[0; 12]);
                    reply.extend_from_slice(b"L\0\0\0\x02L");
                    reply.extend_from_slice(tag);
                    reply.push(b'L');
                    reply.extend_from_slice(&value.to_be_bytes());
                    if socket.write_all(&reply).is_err() {
                        return;
                    }
                    answered += 1;
                    if answered == 2 && then == Then::HangUp {
                        // Long enough for the client to take its READY, short of the deadline.
                        std::thread::sleep(Duration::from_millis(200));
                        return;
                    }
                }
            });
            Self {
                path: format!("unix:{}", path.display()),
                _dir: dir,
                thread: Some(thread),
            }
        }
    }

    impl Drop for FakeServer {
        fn drop(&mut self) {
            // The server thread ends when the client closes its socket (the connection is
            // dropped before the server in every test).
            if let Some(thread) = self.thread.take() {
                let _ = join_within(thread, Duration::from_secs(2));
            }
        }
    }

    #[test]
    fn a_silent_server_fails_the_connect_within_its_deadline() {
        let server = FakeServer::start(Then::Silent);
        let started = Instant::now();
        let result = Conn::connect_to(Some(&server.path), "Kivori test", &[], |_| {});
        assert!(result.is_err(), "no READY from a server that never speaks");
        assert!(
            started.elapsed() < CONNECT_DEADLINE + Duration::from_secs(1),
            "took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_stalled_operation_returns_an_error_within_the_deadline_and_the_connection_is_lost() {
        let server = FakeServer::start(Then::Stall);
        let mut conn = Conn::connect_to(Some(&server.path), "Kivori test", &[], |_| {})
            .expect("the fake server completes the handshake");
        assert!(conn.is_ready());

        let started = Instant::now();
        assert!(
            conn.sink("@DEFAULT_SINK@").is_none(),
            "no answer is no sink"
        );
        let took = started.elapsed();
        assert!(took >= OP_DEADLINE - Duration::from_millis(100), "{took:?}");
        assert!(took < OP_DEADLINE + Duration::from_secs(1), "{took:?}");

        // The stalled connection counts as lost, so nothing queues behind it: instant failures.
        assert!(!conn.is_ready());
        let again = Instant::now();
        assert!(conn.sources().is_none());
        assert!(!conn.set_source_mute(1, true));
        assert!(again.elapsed() < Duration::from_millis(100));

        let dropped = Instant::now();
        drop(conn);
        assert!(dropped.elapsed() < Duration::from_secs(1), "drop is prompt");
    }

    #[test]
    fn a_lost_connection_wakes_the_waiter_before_the_deadline() {
        let server = FakeServer::start(Then::HangUp);
        let mut conn = Conn::connect_to(Some(&server.path), "Kivori test", &[], |_| {})
            .expect("the fake server completes the handshake");
        let started = Instant::now();
        // The server hung up: the context fails and the operation is cancelled, which must wake
        // the waiter (state callbacks stay installed) rather than leave it asleep to the deadline.
        assert!(conn.sources().is_none());
        assert!(
            started.elapsed() < OP_DEADLINE,
            "woken by the failure after {:?}",
            started.elapsed()
        );
        assert!(!conn.is_ready());
    }

    #[test]
    fn the_gate_wakes_on_notify_and_times_out_without_one() {
        let gate = Arc::new(Gate::default());
        let seen = gate.generation();
        let started = Instant::now();
        assert!(!gate.wait_change(seen, started + Duration::from_millis(60)));
        assert!(started.elapsed() >= Duration::from_millis(50));

        // A notification that lands before the wait is not lost.
        gate.notify();
        assert!(gate.wait_change(seen, Instant::now() + Duration::from_secs(5)));

        let seen = gate.generation();
        let waker = Arc::clone(&gate);
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            waker.notify();
        });
        assert!(gate.wait_change(seen, Instant::now() + Duration::from_secs(5)));
        thread.join().unwrap();
    }

    #[test]
    fn percent_and_volume_round_trip_exactly_for_every_percent() {
        for percent in 0..=100u8 {
            assert_eq!(percent_of(volume_of(percent)), percent, "{percent}");
        }
        assert_eq!(volume_of(100), Volume::NORMAL);
        assert_eq!(volume_of(0), Volume::MUTED);
        assert_eq!(volume_of(200), Volume::NORMAL, "clamped");
    }

    #[test]
    fn amplified_volume_reads_as_100() {
        assert_eq!(
            percent_of(Volume(Volume::NORMAL.0 + Volume::NORMAL.0 / 2)),
            100
        );
    }

    #[test]
    fn a_volume_change_keeps_the_balance() {
        let mut stereo = ChannelVolumes::default();
        stereo.set(2, Volume::NORMAL);
        // Left at 100 %, right at 50 %.
        stereo.get_mut()[1] = volume_of(50);
        let quieter = with_percent(&stereo, 60);
        assert_eq!(percent_of(quieter.get()[0]), 60);
        assert_eq!(percent_of(quieter.get()[1]), 30, "right stays half of left");
        assert_eq!(channels_percent(&quieter), 60);
    }

    #[test]
    fn a_silent_device_is_raised_on_every_channel() {
        let mut silent = ChannelVolumes::default();
        silent.set(2, Volume::MUTED);
        let raised = with_percent(&silent, 40);
        assert_eq!(raised.len(), 2);
        assert_eq!(percent_of(raised.get()[0]), 40);
        assert_eq!(percent_of(raised.get()[1]), 40);
    }
}
