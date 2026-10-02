//! The background device thread — the native owner of device communication.
//!
//! An OS thread (not a webview worker, not a Tauri async task tied to a window) drives the connection:
//! discover → open → pump the [`Session`] → apply desired-state commands → back off + reconnect on I/O
//! loss. It survives window close-to-hide, webview reload, webview loss, and frontend navigation
//! because nothing in its lifetime is tied to the window; it stops only when `cancel` is set (explicit
//! Quit / process shutdown). Snapshot changes are published to the shared cell and emitted as
//! `connection://status`; every lifecycle transition is also recorded as typed session activity and
//! emitted as `activity-log://event`. Real serial behaviour is validated manually (no
//! hardware in host CI).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::action::gesture_value::{GestureValue, ValueUpdate};
use crate::activity::{
    ActivityEventKind, ActivityLog, ActivityMetadata, RuntimeActivityPlanner,
    RuntimeActivityRequest, SessionActivity,
};
use crate::companion::CompanionDirector;
use crate::device::discovery::DEFAULT_ALLOWLIST;
use crate::device::fsm::{ConnectionManager, ManagerEvent};
use crate::device::reconnect::base_delay_ms;
use crate::device::serial::{first_candidate, SerialPortLink};
use crate::device::session::{Session, SessionConfig};
use crate::firmware::{self, FirmwareStatus, FlashWorkflow, ResumeTarget};
use crate::input::{InputIngress, RejectReason};
use crate::ipc::dto::{connection_status, ConnectionStatusDto};
use crate::ipc::events;
use crate::orchestrator::Orchestrator;
use crate::platform::{self, ActionAvailability, VolumeBackend};
use crate::presentation::{PresentationResolver, ProductSnapshot};
use crate::runtime::state::DeviceCommand;
use kivori_model::{Capabilities, CompanionState, ConnectionState, MascotPersonality};
use kivori_protocol::{ErrorCategory, InputEvent, PlayMascotAction, Presentation};

const TICK: Duration = Duration::from_millis(50);

/// The longest a newly opened serial port may remain in `Connecting` without a valid `HelloAck`.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// How often an established session is checked for liveness.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);

/// Timer policy owned by the device task. It uses elapsed durations rather than `Instant` so the
/// transitions remain deterministic in host tests.
#[derive(Debug, Default)]
pub struct ConnectionDeadlines {
    handshake_deadline: Option<Duration>,
    next_heartbeat: Option<Duration>,
}

impl ConnectionDeadlines {
    /// Starts with no open link or scheduled work.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            handshake_deadline: None,
            next_heartbeat: None,
        }
    }

    /// Starts a new bounded handshake after the serial port opens.
    pub fn on_port_opened(&mut self, now: Duration) {
        self.handshake_deadline = Some(now + HANDSHAKE_TIMEOUT);
        self.next_heartbeat = None;
    }

    /// Replaces the handshake deadline with the first heartbeat deadline.
    pub fn on_handshake_complete(&mut self, now: Duration) {
        self.handshake_deadline = None;
        self.next_heartbeat = Some(now + HEARTBEAT_INTERVAL);
    }

    /// Schedules the next heartbeat after a successful send.
    pub fn on_heartbeat_sent(&mut self, now: Duration) {
        self.next_heartbeat = Some(now + HEARTBEAT_INTERVAL);
    }

    /// Clears timers tied to a serial link that was closed or lost.
    pub fn on_link_lost(&mut self) {
        self.handshake_deadline = None;
        self.next_heartbeat = None;
    }

    #[must_use]
    pub fn handshake_timed_out(&self, now: Duration) -> bool {
        self.handshake_deadline
            .is_some_and(|deadline| now >= deadline)
    }

    #[must_use]
    pub fn heartbeat_due(&self, now: Duration) -> bool {
        self.next_heartbeat.is_some_and(|deadline| now >= deadline)
    }

    #[must_use]
    fn awaiting_handshake(&self) -> bool {
        self.handshake_deadline.is_some()
    }
}

/// Spawns the background device thread and returns its join handle.
#[must_use]
pub fn spawn(
    app: AppHandle,
    status: Arc<Mutex<ConnectionStatusDto>>,
    activity_log: Arc<ActivityLog>,
    firmware_status: Arc<Mutex<FirmwareStatus>>,
    commands: Receiver<DeviceCommand>,
    cancel: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("kivori-device".to_string())
        .spawn(move || device_loop(app, status, activity_log, firmware_status, commands, cancel))
        .expect("spawn kivori-device thread")
}

fn device_loop(
    app: AppHandle,
    status: Arc<Mutex<ConnectionStatusDto>>,
    activity_log: Arc<ActivityLog>,
    firmware_status: Arc<Mutex<FirmwareStatus>>,
    commands: Receiver<DeviceCommand>,
    cancel: Arc<AtomicBool>,
) {
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    let mut session = Session::new(SessionConfig::default());
    let mut companion = CompanionDirector::new(MascotPersonality::Cozy, true, 0x4B49_564F, 0);
    // Windows has a real backend (`platform::windows`); every other target falls back to the
    // honest "not implemented yet" backend so this crate always compiles. Kept concrete (not
    // boxed) so the Windows branch can reach the Windows-only `try_recv_change` below.
    #[cfg(windows)]
    let backend = platform::windows::WindowsVolumeBackend::new();
    #[cfg(not(windows))]
    let backend = platform::unimplemented::UnimplementedVolumeBackend::new(std::env::consts::OS);
    let mut rotary = RotaryPipeline::new(&backend);
    let mut link: Option<SerialPortLink> = None;
    let mut connected_port: Option<String> = None;
    let mut retry_at: Option<Instant> = None;
    let mut reconnect_deadline: Option<Instant> = None;
    let mut deadlines = ConnectionDeadlines::default();
    let mut flash = FlashWorkflow::new(
        firmware::bundled_image_available(),
        firmware::BUNDLED_FIRMWARE.len() as u64,
    );
    publish_firmware_status(&firmware_status, &flash);
    drain_firmware_activity(&app, &activity_log, &mut flash);
    let started = Instant::now();

    let mut last = connection_status(
        &manager,
        &orchestrator,
        session.reported(),
        session.supports_mascot_interaction(),
        session.last_mascot_action_applied(),
        session.connection_generation(),
    );
    *status.lock().expect("status lock") = last.clone();
    events::emit_status(&app, &last);
    let mut previous_state = manager.state();
    let mut activity_planner = RuntimeActivityPlanner::new();
    record(&app, &activity_log, &manager, started);

    while !cancel.load(Ordering::SeqCst) {
        // 1. Apply queued UI commands.
        while let Ok(command) = commands.try_recv() {
            record_observations(
                &app,
                &activity_log,
                plan_device_request(&activity_planner, &command, None),
            );
            match command {
                DeviceCommand::SetDesired(_) | DeviceCommand::MirrorDesired(_)
                    if flash.is_busy() =>
                {
                    // The public command rejects new changes while busy; discard anything queued just
                    // before the device thread reserved the serial session.
                }
                DeviceCommand::SetDesired(state) | DeviceCommand::MirrorDesired(state) => {
                    let write_failed = match link.as_mut() {
                        Some(open_link) => session
                            .set_desired(open_link, &manager, &mut orchestrator, state)
                            .is_err(),
                        None => {
                            orchestrator.set_desired(state);
                            false
                        }
                    };
                    if write_failed {
                        recover_link(
                            &mut activity_planner,
                            &mut manager,
                            ManagerEvent::IoError,
                            LinkRecovery::new(
                                &mut link,
                                &mut connected_port,
                                &mut retry_at,
                                &mut deadlines,
                                &flash,
                            ),
                            |observation| {
                                record_observations(&app, &activity_log, [observation]);
                            },
                        );
                    }
                }
                DeviceCommand::ConfigureCompanion {
                    personality,
                    self_play,
                } => {
                    let now = elapsed_ms(started.elapsed());
                    companion.set_personality(personality, now);
                    companion.set_self_play(self_play, now);
                }
                DeviceCommand::PlayMascotAction(_) if flash.is_busy() => {}
                DeviceCommand::PlayMascotAction(action) => {
                    let cue = companion.manual(action);
                    record_observations(
                        &app,
                        &activity_log,
                        plan_device_request(
                            &activity_planner,
                            &DeviceCommand::PlayMascotAction(action),
                            Some(&cue),
                        ),
                    );
                    let write_failed = match link.as_mut() {
                        Some(open_link) => session
                            .play_mascot_action(
                                open_link,
                                &manager,
                                cue.action,
                                cue.personality,
                                cue.seed,
                            )
                            .is_err(),
                        None => false,
                    };
                    if write_failed {
                        recover_link(
                            &mut activity_planner,
                            &mut manager,
                            ManagerEvent::IoError,
                            LinkRecovery::new(
                                &mut link,
                                &mut connected_port,
                                &mut retry_at,
                                &mut deadlines,
                                &flash,
                            ),
                            |observation| {
                                record_observations(&app, &activity_log, [observation]);
                            },
                        );
                    }
                }
                DeviceCommand::Refresh => {}
                DeviceCommand::FlashFirmware => {
                    let requested = flash.request(
                        manager.state().can_drive_device(),
                        connected_port.as_deref(),
                        manager
                            .device()
                            .map(|device| device.device_id_hash_short.as_str()),
                    );
                    let Ok(port) = requested else {
                        if !flash.is_busy() {
                            flash.fail_preparation();
                        }
                        drain_firmware_activity(&app, &activity_log, &mut flash);
                        publish_firmware_status(&firmware_status, &flash);
                        continue;
                    };
                    drain_firmware_activity(&app, &activity_log, &mut flash);
                    publish_firmware_status(&firmware_status, &flash);

                    // This drop closes the serial handle before `espflash` opens the same port.
                    link = None;
                    connected_port = None;
                    deadlines.on_link_lost();
                    manager.apply(ManagerEvent::PortRemoved);

                    let resume = run_accepted_firmware_flash(
                        &mut flash,
                        |flashing_status| {
                            *firmware_status.lock().expect("firmware status lock") =
                                flashing_status.clone();
                            firmware::flash_bundled(&port, &cancel)
                        },
                        |observation| {
                            record_observations(&app, &activity_log, [observation]);
                        },
                    );
                    publish_firmware_status(&firmware_status, &flash);
                    retry_at = None;
                    match resume {
                        ResumeTarget::Discovery => reconnect_deadline = None,
                        ResumeTarget::SamePort(_) => {
                            reconnect_deadline = Some(Instant::now() + firmware::RECONNECT_TIMEOUT);
                        }
                    }
                }
            }
            drain_firmware_activity(&app, &activity_log, &mut flash);
        }

        // 2. Drive the link: discover + handshake when down (honoring backoff), pump when up.
        // The deadline applies even if the selected port opened but never sends a valid HelloAck.
        if reconnect_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            link = None;
            connected_port = None;
            deadlines.on_link_lost();
            manager.apply(ManagerEvent::PortRemoved);
            flash.reconnect_timed_out();
            drain_firmware_activity(&app, &activity_log, &mut flash);
            publish_firmware_status(&firmware_status, &flash);
            reconnect_deadline = None;
            retry_at = None;
        }
        match link.as_mut() {
            None => {
                let ready = retry_at.is_none_or(|deadline| Instant::now() >= deadline);
                if ready {
                    retry_at = None;
                    if manager.state() == kivori_model::ConnectionState::Error {
                        manager.apply(ManagerEvent::BackoffElapsed);
                    }
                    let candidate = match flash.status().phase {
                        crate::firmware::FirmwarePhase::Reconnecting => {
                            flash_target(&flash).map(str::to_string)
                        }
                        _ => first_candidate(DEFAULT_ALLOWLIST),
                    };
                    if let Some(name) = candidate {
                        let attempt = activity_planner.attempt();
                        record_with_metadata(&app, &activity_log, attempt.kind, attempt.metadata);
                        let opened = SerialPortLink::open(&name).ok().and_then(|mut opened| {
                            session
                                .open(&mut opened, &mut manager)
                                .is_ok()
                                .then_some(opened)
                        });
                        if let Some(opened) = opened {
                            link = Some(opened);
                            connected_port = Some(name);
                            deadlines.on_port_opened(started.elapsed());
                        } else {
                            recover_discovery_open_failure(
                                &mut activity_planner,
                                &mut manager,
                                LinkRecovery::new(
                                    &mut link,
                                    &mut connected_port,
                                    &mut retry_at,
                                    &mut deadlines,
                                    &flash,
                                ),
                                |observation| {
                                    record_observations(&app, &activity_log, [observation]);
                                },
                            );
                        }
                    }
                }
            }
            Some(open_link) => {
                let pump_failed = session
                    .pump(open_link, &mut manager, &mut orchestrator)
                    .is_err();
                let now = started.elapsed();
                let event = if pump_failed {
                    Some(ManagerEvent::IoError)
                } else if manager.state() == kivori_model::ConnectionState::Connecting
                    && deadlines.handshake_timed_out(now)
                {
                    Some(ManagerEvent::HandshakeTimeout)
                } else if manager.state().can_drive_device() {
                    if deadlines.awaiting_handshake() {
                        deadlines.on_handshake_complete(now);
                    }
                    if deadlines.heartbeat_due(now) {
                        if session.send_ping(open_link, elapsed_ms(now)).is_err() {
                            Some(ManagerEvent::IoError)
                        } else {
                            deadlines.on_heartbeat_sent(now);
                            session
                                .heartbeat_timed_out()
                                .then_some(ManagerEvent::HeartbeatTimeout)
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(event) = event {
                    recover_link(
                        &mut activity_planner,
                        &mut manager,
                        event,
                        LinkRecovery::new(
                            &mut link,
                            &mut connected_port,
                            &mut retry_at,
                            &mut deadlines,
                            &flash,
                        ),
                        |observation| {
                            record_observations(&app, &activity_log, [observation]);
                        },
                    );
                }
            }
        }

        // Slice 002: rotary input -> volume -> transient `Presentation` overlay. Paused while a
        // firmware flash owns the serial session: queued input is discarded unexecuted.
        let mut presentations = Vec::new();
        let inputs = session.take_input_events();
        if !flash.is_busy() {
            rotary.accept_inputs(&inputs, &backend, &mut presentations, |observation| {
                record_observations(&app, &activity_log, [observation]);
            });
        }
        while let Some(change) = backend.try_recv_change() {
            rotary.on_backend_change(change, &mut presentations, |observation| {
                record_observations(&app, &activity_log, [observation]);
            });
        }
        rotary.observe_backend_availability(&backend, |observation| {
            record_observations(&app, &activity_log, [observation]);
        });
        if !flash.is_busy()
            && session
                .negotiated_caps()
                .contains(Capabilities::PRESENTATION_V1)
        {
            if let Some(open_link) = link.as_mut() {
                let write_failed = presentations.into_iter().any(|presentation| {
                    session.send_presentation(open_link, presentation).is_err()
                });
                if write_failed {
                    recover_link(
                        &mut activity_planner,
                        &mut manager,
                        ManagerEvent::IoError,
                        LinkRecovery::new(
                            &mut link,
                            &mut connected_port,
                            &mut retry_at,
                            &mut deadlines,
                            &flash,
                        ),
                        |observation| {
                            record_observations(&app, &activity_log, [observation]);
                        },
                    );
                }
            }
        }

        drain_session_activity(&app, &activity_log, &mut session);

        if flash.status().phase == crate::firmware::FirmwarePhase::Reconnecting
            && manager.state().can_drive_device()
        {
            if let (Some(port), Some(device)) = (connected_port.as_deref(), manager.device()) {
                if flash.handshake(port, &device.device_id_hash_short, true) {
                    drain_firmware_activity(&app, &activity_log, &mut flash);
                    publish_firmware_status(&firmware_status, &flash);
                    reconnect_deadline = None;
                }
            }
        }

        // Desktop owns ambient behavior. Suppressed/disconnected states consume due opportunities,
        // preventing stale antics from firing immediately after focus work or reconnect.
        let companion_state = if manager.state().can_drive_device() {
            session
                .reported()
                .unwrap_or_else(|| orchestrator.desired().to_companion())
        } else {
            CompanionState::Offline
        };
        if let Some(cue) = companion.poll(elapsed_ms(started.elapsed()), companion_state) {
            record_observations(
                &app,
                &activity_log,
                activity_planner.requests(RuntimeActivityRequest::SocialAction {
                    action: cue.action,
                    personality: cue.personality,
                    seed: cue.seed,
                    autonomous: true,
                }),
            );
            let write_failed = match link.as_mut() {
                Some(open_link) => session
                    .play_mascot_action(open_link, &manager, cue.action, cue.personality, cue.seed)
                    .is_err(),
                None => false,
            };
            if write_failed {
                recover_link(
                    &mut activity_planner,
                    &mut manager,
                    ManagerEvent::IoError,
                    LinkRecovery::new(
                        &mut link,
                        &mut connected_port,
                        &mut retry_at,
                        &mut deadlines,
                        &flash,
                    ),
                    |observation| {
                        record_observations(&app, &activity_log, [observation]);
                    },
                );
            }
        }

        // 3. Publish + emit the snapshot on change.
        let snapshot = connection_status(
            &manager,
            &orchestrator,
            session.reported(),
            session.supports_mascot_interaction(),
            session.last_mascot_action_applied(),
            session.connection_generation(),
        );
        if snapshot != last {
            *status.lock().expect("status lock") = snapshot.clone();
            events::emit_status(&app, &snapshot);
            last = snapshot;
        }

        // 4. Record typed activity for every lifecycle transition (connect, incompatible,
        //    disconnect, recoverable error, reconnect attempt) — T105.
        let current_state = manager.state();
        rotary.on_connection_state(previous_state, current_state, session.current_session());
        if observe_connection_transition(
            &mut activity_planner,
            &mut previous_state,
            current_state,
            |observation| record_observations(&app, &activity_log, [observation]),
        ) {
            record(&app, &activity_log, &manager, started);
        }

        // With a link open, wake as soon as device bytes arrive (TICK is only the upper bound);
        // otherwise pace discovery and backoff with a plain sleep.
        if link
            .as_mut()
            .is_none_or(|open_link| open_link.wait(TICK).is_err())
        {
            std::thread::sleep(TICK);
        }
    }
}

/// The Slice 002 rotary pipeline owned by the device task: input ingress -> gesture value ->
/// presentation resolver, plus the typed activity for its failures.
///
/// `Presentation` carries only the transient volume overlay. Its `primary` is the interaction
/// channel (`Idle` unless a volume write is known to have failed); the companion director and the
/// device's mascot state still own the screen.
pub struct RotaryPipeline {
    ingress: InputIngress,
    gesture_value: GestureValue,
    resolver: PresentationResolver,
    volume_failed: bool,
    audio_available: bool,
}

impl RotaryPipeline {
    /// Starts with no session. The resolver's initial nonce is a placeholder: every entry to
    /// `Connected` rebinds it (and restarts `revision`) before any `Presentation` is resolved.
    #[must_use]
    pub fn new(backend: &dyn VolumeBackend) -> Self {
        Self {
            ingress: InputIngress::new(),
            gesture_value: GestureValue::new(),
            resolver: PresentationResolver::new(0),
            volume_failed: false,
            audio_available: is_available(backend),
        }
    }

    /// (Re)scopes session state on a connection-state transition. Keyed off the manager state,
    /// not `Session::current_session()`, because heartbeat timeouts and port removal are applied
    /// outside `Session` — the connection state is the only place that sees every exit.
    pub fn on_connection_state(
        &mut self,
        previous: ConnectionState,
        current: ConnectionState,
        session: Option<u32>,
    ) {
        if previous == current {
            return;
        }
        if current == ConnectionState::Connected {
            if let Some(nonce) = session {
                self.ingress.begin_session(nonce);
                self.resolver.begin_session(nonce);
            }
        } else if previous == ConnectionState::Connected {
            self.ingress.end_session();
            self.gesture_value.end_session();
        }
    }

    /// Validates and executes decoded input, queueing any resulting presentations. Rejected input
    /// is dropped unexecuted and recorded by safe category only (never its payload).
    pub fn accept_inputs(
        &mut self,
        events: &[InputEvent],
        backend: &dyn VolumeBackend,
        presentations: &mut Vec<Presentation>,
        mut observe: impl FnMut(SessionActivity),
    ) {
        for event in events {
            match self.ingress.accept(event) {
                Ok(Some(input)) => {
                    if let Some(update) = self.gesture_value.on_input(input, backend) {
                        self.push_update(update, presentations, &mut observe);
                    }
                }
                Ok(None) => {}
                Err(reason) => observe(SessionActivity::new(
                    match reason {
                        RejectReason::NoSession | RejectReason::StaleSession => {
                            ActivityEventKind::InputStaleSessionRejected
                        }
                        // Both are input that fits no gesture the device could have produced.
                        RejectReason::UnknownGesture | RejectReason::ControlMismatch => {
                            ActivityEventKind::InputUnstartedGestureRejected
                        }
                    },
                    Some(ActivityMetadata::HostDiagnostic {
                        category: ErrorCategory::BadPayload,
                    }),
                )),
            }
        }
    }

    /// Routes one backend-originated volume change. Every percent here was read from the OS by the
    /// owning audio thread, so `Confirmed` still only ever follows a real backend read.
    pub fn on_backend_change(
        &mut self,
        change: platform::VolumeChange,
        presentations: &mut Vec<Presentation>,
        mut observe: impl FnMut(SessionActivity),
    ) {
        let update = match change.origin {
            platform::ChangeOrigin::External => {
                self.gesture_value.on_external_change(change.percent)
            }
            platform::ChangeOrigin::EndpointRebind => {
                observe(SessionActivity::new(
                    ActivityEventKind::AudioEndpointChanged,
                    None,
                ));
                self.gesture_value.on_endpoint_rebind(change.percent)
            }
            // Kivori's own write, echoed back; `set()` already confirmed it via its read-back.
            platform::ChangeOrigin::Kivori => None,
        };
        if let Some(update) = update {
            self.push_update(update, presentations, &mut observe);
        }
    }

    /// Records `AudioEndpointLost` when the backend stops being available.
    pub fn observe_backend_availability(
        &mut self,
        backend: &dyn VolumeBackend,
        mut observe: impl FnMut(SessionActivity),
    ) {
        let available = is_available(backend);
        if self.audio_available && !available {
            observe(SessionActivity::new(
                ActivityEventKind::AudioEndpointLost,
                None,
            ));
        }
        self.audio_available = available;
    }

    fn push_update(
        &mut self,
        update: ValueUpdate,
        presentations: &mut Vec<Presentation>,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        // One entry per failure streak: a failing backend fails every detent of a gesture.
        if update.failed && !self.volume_failed {
            observe(SessionActivity::new(
                ActivityEventKind::VolumeWriteFailed,
                None,
            ));
        }
        self.volume_failed = update.failed;
        presentations.push(self.resolver.resolve(&ProductSnapshot::with_value(update)));
    }
}

fn is_available(backend: &dyn VolumeBackend) -> bool {
    matches!(backend.availability(), ActionAvailability::Available { .. })
}

fn elapsed_ms(elapsed: Duration) -> u32 {
    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
}

/// Maps the real device-command boundary through the shared runtime activity planner.
///
/// A social command is observable only after the director has supplied its closed personality and
/// seed values, so callers pass that cue on the accepted command path.
#[must_use]
pub fn plan_device_request(
    activity_planner: &RuntimeActivityPlanner,
    command: &DeviceCommand,
    social_cue: Option<&PlayMascotAction>,
) -> Vec<SessionActivity> {
    let request = match command {
        DeviceCommand::SetDesired(state) => Some(RuntimeActivityRequest::State {
            state: *state,
            mirrored: false,
        }),
        DeviceCommand::MirrorDesired(state) => Some(RuntimeActivityRequest::State {
            state: *state,
            mirrored: true,
        }),
        DeviceCommand::ConfigureCompanion {
            personality,
            self_play,
        } => Some(RuntimeActivityRequest::CompanionConfiguration {
            personality: *personality,
            self_play: *self_play,
        }),
        DeviceCommand::PlayMascotAction(_) => {
            social_cue.map(|cue| RuntimeActivityRequest::SocialAction {
                action: cue.action,
                personality: cue.personality,
                seed: cue.seed,
                autonomous: false,
            })
        }
        DeviceCommand::FlashFirmware | DeviceCommand::Refresh => None,
    };
    request.map_or_else(Vec::new, |request| activity_planner.requests(request))
}

/// Runs the accepted synchronous flash path and delivers queued observations after each workflow
/// transition, including before the potentially blocking flasher callback.
pub fn run_accepted_firmware_flash<E>(
    flash: &mut FlashWorkflow,
    run_flash: impl FnOnce(&FirmwareStatus) -> Result<(), E>,
    mut observe: impl FnMut(SessionActivity),
) -> ResumeTarget
where
    E: AsRef<str>,
{
    flash.drain_activity().into_iter().for_each(&mut observe);
    flash.mark_serial_released();
    flash.drain_activity().into_iter().for_each(&mut observe);
    flash.mark_flashing();
    flash.drain_activity().into_iter().for_each(&mut observe);
    let result = run_flash(flash.status());
    let resume = flash.finish(result);
    flash.drain_activity().into_iter().for_each(observe);
    resume
}

/// The device-loop state a link recovery tears down and reschedules.
pub struct LinkRecovery<'a> {
    link: &'a mut Option<SerialPortLink>,
    connected_port: &'a mut Option<String>,
    retry_at: &'a mut Option<Instant>,
    deadlines: &'a mut ConnectionDeadlines,
    flash: &'a FlashWorkflow,
}

impl<'a> LinkRecovery<'a> {
    pub fn new(
        link: &'a mut Option<SerialPortLink>,
        connected_port: &'a mut Option<String>,
        retry_at: &'a mut Option<Instant>,
        deadlines: &'a mut ConnectionDeadlines,
        flash: &'a FlashWorkflow,
    ) -> Self {
        Self {
            link,
            connected_port,
            retry_at,
            deadlines,
            flash,
        }
    }
}

/// Applies the real link-recovery state change and yields its closed activity observations.
///
/// Tauri recording remains in the caller, allowing host tests to exercise this exact production
/// adapter with an absent serial link.
pub fn recover_link(
    activity_planner: &mut RuntimeActivityPlanner,
    manager: &mut ConnectionManager,
    event: ManagerEvent,
    recovery: LinkRecovery<'_>,
    observe: impl FnMut(SessionActivity),
) {
    let failure_kind = connection_event_kind(&event);
    manager.apply(event);
    *recovery.link = None;
    *recovery.connected_port = None;
    recovery.deadlines.on_link_lost();
    if recovery.flash.status().phase == crate::firmware::FirmwarePhase::Reconnecting {
        *recovery.retry_at = None;
    } else {
        let backoff = base_delay_ms(manager.retry_count());
        *recovery.retry_at = Some(Instant::now() + Duration::from_millis(backoff));
    }
    activity_planner
        .failure(failure_kind, recovery.retry_at.is_some())
        .into_iter()
        .for_each(observe);
}

/// Recovers a discovery attempt whose serial open or `Hello` write failed.
///
/// A failed serial open never reached `Connecting`; it is still a failed connection attempt, so it
/// enters the same I/O-failure backoff as a failed `Hello` write instead of retrying every tick.
pub fn recover_discovery_open_failure(
    activity_planner: &mut RuntimeActivityPlanner,
    manager: &mut ConnectionManager,
    recovery: LinkRecovery<'_>,
    observe: impl FnMut(SessionActivity),
) {
    if manager.state() == ConnectionState::Disconnected {
        manager.apply(ManagerEvent::PortOpened);
    }
    recover_link(
        activity_planner,
        manager,
        ManagerEvent::IoError,
        recovery,
        observe,
    );
}

/// Observes the exact lifecycle transition used by the device loop.
///
/// Recovery stays pending through intermediate states and is emitted only when the new state is
/// connected. The return value tells the caller whether it must record the manager snapshot.
pub fn observe_connection_transition(
    activity_planner: &mut RuntimeActivityPlanner,
    previous_state: &mut ConnectionState,
    current_state: ConnectionState,
    observe: impl FnMut(SessionActivity),
) -> bool {
    if current_state == *previous_state {
        return false;
    }
    if current_state == ConnectionState::Connected {
        activity_planner.recovered().into_iter().for_each(observe);
    }
    *previous_state = current_state;
    true
}

fn publish_firmware_status(status: &Mutex<FirmwareStatus>, workflow: &FlashWorkflow) {
    *status.lock().expect("firmware status lock") = workflow.status().clone();
}

fn flash_target(workflow: &FlashWorkflow) -> Option<&str> {
    // The workflow's status deliberately hides the port from IPC. The device task needs it to reopen
    // exactly the same native-owned port after programming.
    workflow.target_port()
}

/// Records the manager's lifecycle transition as typed activity and emits it to the webview.
fn record(
    app: &AppHandle,
    activity_log: &ActivityLog,
    manager: &ConnectionManager,
    started: Instant,
) {
    tracing::info!(connection = ?manager.state(), retries = manager.retry_count(), "device connection changed");
    let elapsed_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
    let event = activity_log.record(
        ActivityEventKind::ConnectionStateChanged,
        Some(ActivityMetadata::Connection {
            state: manager.state(),
            retry_count: manager.retry_count(),
            elapsed_ms,
        }),
    );
    events::emit_activity_log(app, &event);
}

fn drain_session_activity(app: &AppHandle, activity_log: &ActivityLog, session: &mut Session) {
    record_observations(app, activity_log, session.drain_activity());
}

fn drain_firmware_activity(app: &AppHandle, activity_log: &ActivityLog, flash: &mut FlashWorkflow) {
    record_observations(app, activity_log, flash.drain_activity());
}

fn record_observations(
    app: &AppHandle,
    activity_log: &ActivityLog,
    observations: impl IntoIterator<Item = SessionActivity>,
) {
    for observation in observations {
        let event = activity_log.record(observation.kind, observation.metadata);
        events::emit_activity_log(app, &event);
    }
}

fn record_with_metadata(
    app: &AppHandle,
    activity_log: &ActivityLog,
    kind: ActivityEventKind,
    metadata: Option<ActivityMetadata>,
) {
    let event = activity_log.record(kind, metadata);
    events::emit_activity_log(app, &event);
}

fn connection_event_kind(event: &ManagerEvent) -> ActivityEventKind {
    match event {
        ManagerEvent::IoError => ActivityEventKind::ConnectionIoFailure,
        ManagerEvent::HandshakeTimeout => ActivityEventKind::HandshakeTimedOut,
        ManagerEvent::HeartbeatTimeout => ActivityEventKind::HeartbeatTimedOut,
        ManagerEvent::PortRemoved => ActivityEventKind::ConnectionDisconnected,
        ManagerEvent::BackoffElapsed => ActivityEventKind::ConnectionRetryScheduled,
        ManagerEvent::PortOpened | ManagerEvent::HandshakeOk(_) => {
            ActivityEventKind::ConnectionOpened
        }
        ManagerEvent::HandshakeIncompatible { .. } => ActivityEventKind::IncompatibleFirmware,
    }
}
