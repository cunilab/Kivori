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
use crate::config::ResolvedConfig;
use crate::desk::DeskRuntime;
use crate::device::discovery::{CandidateRotator, DEFAULT_ALLOWLIST};
use crate::device::fsm::{ConnectionManager, ManagerEvent};
use crate::device::reconnect::base_delay_ms;
use crate::device::serial::{enumerate, SerialPortLink};
use crate::device::session::{Session, SessionConfig, SessionError};
use crate::firmware::{self, FirmwareStatus, FlashWorkflow, ResumeTarget};
use crate::input::{Freshness, InputIngress, LogicalInput, RejectReason};
use crate::ipc::dto::{
    availability_token, connection_status, desk_status_dto, ConnectionStatusDto, DeskStatusDto,
    DiagnosticsSnapshot, HostServicesSnapshot,
};
use crate::ipc::events;
use crate::orchestrator::Orchestrator;
use crate::platform::{self, ActionAvailability, VolumeBackend};
use crate::presentation::{PresentationResolver, ProductSnapshot};
use crate::runtime::state::DeviceCommand;
use kivori_model::desk::{ActionFeedback, ActionKind, FeedbackKind};
use kivori_model::{Capabilities, CompanionState, ConnectionState};
use kivori_protocol::ByeReason;
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
// Each argument is a distinct shared cell or channel owned by `setup`; bundling them would only move the list.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn spawn(
    app: AppHandle,
    status: Arc<Mutex<ConnectionStatusDto>>,
    desk_status: Arc<Mutex<DeskStatusDto>>,
    activity_log: Arc<ActivityLog>,
    firmware_status: Arc<Mutex<FirmwareStatus>>,
    diagnostics: Arc<Mutex<DiagnosticsSnapshot>>,
    commands: Receiver<DeviceCommand>,
    cancel: Arc<AtomicBool>,
    config: Arc<ResolvedConfig>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("kivori-device".to_string())
        .spawn(move || {
            device_loop(
                app,
                status,
                desk_status,
                activity_log,
                firmware_status,
                diagnostics,
                commands,
                cancel,
                config,
            );
        })
        .expect("spawn kivori-device thread")
}

#[allow(clippy::too_many_arguments)]
fn device_loop(
    app: AppHandle,
    status: Arc<Mutex<ConnectionStatusDto>>,
    desk_status: Arc<Mutex<DeskStatusDto>>,
    activity_log: Arc<ActivityLog>,
    firmware_status: Arc<Mutex<FirmwareStatus>>,
    diagnostics: Arc<Mutex<DiagnosticsSnapshot>>,
    commands: Receiver<DeviceCommand>,
    cancel: Arc<AtomicBool>,
    config: Arc<ResolvedConfig>,
) {
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    let mut session = Session::new(SessionConfig::default());
    let mut companion = CompanionDirector::new(
        config.buddy.intensity.personality(),
        config.buddy.reactions,
        0x4B49_564F,
        0,
    );
    // Windows and macOS have real backends; every other target gets the honest "not implemented
    // yet" services so this crate always compiles (`platform::os_services`).
    let main_app = app.clone();
    let services = platform::os_services(Some(Arc::new(move |run| {
        let _ = main_app.run_on_main_thread(run);
    })));
    let backend = Arc::clone(&services.volume);
    // The diagnostics page reads these without going through the desk runtime that owns them.
    let app_volume = Arc::clone(&services.app_volume);
    let media = Arc::clone(&services.media);
    let foreground = Arc::clone(&services.foreground);
    let mut diagnostics_publisher = DiagnosticsPublisher::default();
    let mut desk = DeskRuntime::new(services).with_config(&config);
    let mut last_desk: Option<DeskStatusDto> = None;
    let mut rotary = RotaryPipeline::new(&*backend);
    let mut link: Option<SerialPortLink> = None;
    let mut connected_port: Option<String> = None;
    let mut rotator = CandidateRotator::new();
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
    let mut active_session = None;
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
                            )
                            .with_rotator(&mut rotator, started.elapsed()),
                            |observation| {
                                record_observations(&app, &activity_log, [observation]);
                            },
                        );
                    }
                }
                DeviceCommand::ApplyConfig(config) => {
                    let now = elapsed_ms(started.elapsed());
                    let (personality, self_play) =
                        (config.buddy.intensity.personality(), config.buddy.reactions);
                    // Restarting the cadence on an unrelated save would delay the next reaction.
                    if (personality, self_play) != (companion.personality(), companion.self_play())
                    {
                        companion.set_personality(personality, now);
                        companion.set_self_play(self_play, now);
                        record_observations(
                            &app,
                            &activity_log,
                            activity_planner.requests(
                                RuntimeActivityRequest::CompanionConfiguration {
                                    personality,
                                    self_play,
                                },
                            ),
                        );
                    }
                    desk.apply_config(&config, &mut |observation| {
                        record_observations(&app, &activity_log, [observation]);
                    });
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
                            )
                            .with_rotator(&mut rotator, started.elapsed()),
                            |observation| {
                                record_observations(&app, &activity_log, [observation]);
                            },
                        );
                    }
                }
                DeviceCommand::Refresh => {}
                DeviceCommand::SetDisplayMode(mode) => {
                    desk.set_mode(mode, &mut |observation| {
                        record_observations(&app, &activity_log, [observation]);
                    });
                }
                DeviceCommand::TestAction(action) => {
                    desk.test(action, started.elapsed(), &mut |observation| {
                        record_observations(&app, &activity_log, [observation]);
                    });
                }
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
                        _ => rotator.next(&enumerate(), DEFAULT_ALLOWLIST, started.elapsed()),
                    };
                    if candidate.is_none()
                        && manager.state() == ConnectionState::Incompatible
                        && !rotator.any_skipped()
                    {
                        // The incompatible board was unplugged and nothing else is left to try.
                        manager.apply(ManagerEvent::PortRemoved);
                    }
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
                            // Recovery attributes the failure to `connected_port` and clears it.
                            connected_port = Some(name);
                            recover_discovery_open_failure(
                                &mut activity_planner,
                                &mut manager,
                                LinkRecovery::new(
                                    &mut link,
                                    &mut connected_port,
                                    &mut retry_at,
                                    &mut deadlines,
                                    &flash,
                                )
                                .with_rotator(&mut rotator, started.elapsed()),
                                |observation| {
                                    record_observations(&app, &activity_log, [observation]);
                                },
                            );
                        }
                    }
                }
            }
            Some(open_link) => {
                session.set_host_ms(elapsed_ms(started.elapsed()));
                let pump_error = session
                    .pump(open_link, &mut manager, &mut orchestrator)
                    .err();
                let now = started.elapsed();
                let event = if let Some(error) = pump_error {
                    Some(if matches!(error, SessionError::PeerClosed) {
                        ManagerEvent::PortRemoved
                    } else {
                        ManagerEvent::IoError
                    })
                } else if manager.state() == kivori_model::ConnectionState::Connecting
                    && deadlines.handshake_timed_out(now)
                {
                    Some(ManagerEvent::HandshakeTimeout)
                } else if manager.state().can_drive_device() {
                    if deadlines.awaiting_handshake() {
                        deadlines.on_handshake_complete(now);
                        if let Some(port) = connected_port.as_deref() {
                            rotator.record_success(port);
                        }
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

                if event.is_none()
                    && manager.state() == ConnectionState::Incompatible
                    && flash.status().phase != crate::firmware::FirmwarePhase::Reconnecting
                {
                    // The UI keeps showing Incompatible; drop this link and skip the port until it
                    // is unplugged so other candidates (a real Kivori) get their turn.
                    if let Some(port) = connected_port.take() {
                        rotator.mark_skipped(&port);
                    }
                    link = None;
                    deadlines.on_link_lost();
                } else if let Some(event) = event {
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
                        )
                        .with_rotator(&mut rotator, started.elapsed()),
                        |observation| {
                            record_observations(&app, &activity_log, [observation]);
                        },
                    );
                }
            }
        }

        synchronize_session(
            &mut active_session,
            &session,
            &manager,
            &mut rotary,
            &mut desk,
        );

        // Slice 002: rotary input -> volume -> transient `Presentation` overlay. Paused while a
        // firmware flash owns the serial session: queued input is discarded unexecuted.
        let mut presentations = Vec::new();
        let inputs = session.take_input_events();
        if !flash.is_busy() {
            let now = started.elapsed();
            let freshness = Freshness {
                clock: session.device_clock(),
                host_now_ms: elapsed_ms(now),
            };
            rotary.accept_inputs_fresh(
                &inputs,
                freshness,
                &*backend,
                &mut presentations,
                |input, remaining| {
                    desk.on_input_within(&input, now, remaining, &mut |observation| {
                        record_observations(&app, &activity_log, [observation]);
                    })
                },
                |observation| {
                    record_observations(&app, &activity_log, [observation]);
                },
            );
            if rotary.take_volume_failure() {
                desk.report(ActionFeedback {
                    action: ActionKind::Volume,
                    kind: FeedbackKind::Error,
                });
            }
        }
        while let Some(change) = backend.try_recv_change() {
            desk.on_audio_change(&change);
            rotary.on_backend_change(change, &mut presentations, |observation| {
                record_observations(&app, &activity_log, [observation]);
            });
        }
        rotary.observe_backend_availability(&*backend, |observation| {
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
                        )
                        .with_rotator(&mut rotator, started.elapsed()),
                        |observation| {
                            record_observations(&app, &activity_log, [observation]);
                        },
                    );
                }
            }
        }

        // M1 desk: status and action feedback, each sent only if the session negotiated it.
        let desk_out = desk.tick(started.elapsed(), &mut |observation| {
            record_observations(&app, &activity_log, [observation]);
        });
        if !flash.is_busy() && manager.state().can_drive_device() {
            if let Some(open_link) = link.as_mut() {
                let mut write_failed = desk_out.status.is_some_and(|desk_status| {
                    session.send_status(open_link, desk_status).is_err()
                });
                for feedback in desk_out.feedback {
                    write_failed |= session.send_feedback(open_link, feedback).is_err();
                }
                if let Some(info) = desk_out.media_info {
                    write_failed |= session.send_media_info(open_link, info).is_err();
                }
                if let Some(labels) = desk_out.controls {
                    write_failed |= session.send_control_labels(open_link, labels).is_err();
                }
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
                        )
                        .with_rotator(&mut rotator, started.elapsed()),
                        |observation| {
                            record_observations(&app, &activity_log, [observation]);
                        },
                    );
                }
            }
        }
        let desk_snapshot = desk_status_dto(&desk);
        if last_desk.as_ref() != Some(&desk_snapshot) {
            *desk_status.lock().expect("desk status lock") = desk_snapshot.clone();
            events::emit_desk_status(&app, &desk_snapshot);
            last_desk = Some(desk_snapshot);
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
                    )
                    .with_rotator(&mut rotator, started.elapsed()),
                    |observation| {
                        record_observations(&app, &activity_log, [observation]);
                    },
                );
            }
        }

        // A later write failure must also cancel queued actions before the loop waits.
        synchronize_session(
            &mut active_session,
            &session,
            &manager,
            &mut rotary,
            &mut desk,
        );

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

        diagnostics_publisher.publish(
            &diagnostics,
            &manager,
            &session,
            HostServicesSnapshot {
                system_volume: availability_token(&backend.availability()),
                app_volume: availability_token(&app_volume.availability()),
                media: if media.status().is_some() {
                    "observable"
                } else {
                    "notObservable"
                },
                focus: if matches!(foreground.foreground(), platform::Foreground::Unknown) {
                    "unknown"
                } else {
                    "detecting"
                },
                input_permission: if desk
                    .last_action()
                    .is_some_and(|last| last.permission_required)
                {
                    "required"
                } else if cfg!(target_os = "macos") {
                    "unknown"
                } else {
                    "notNeeded"
                },
            },
        );

        // 4. Record typed activity for every lifecycle transition (connect, incompatible,
        //    disconnect, recoverable error, reconnect attempt) — T105.
        let current_state = manager.state();
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

    // Quit: tell the device this is on purpose, so it shows Offline at once instead of waiting
    // out its host-silence timeout. Best effort; `AppState::shutdown` is joining this thread.
    if let Some(open_link) = link.as_mut() {
        let _ = session.close(open_link, ByeReason::Shutdown);
    }
}

/// Synchronizes session-owned actions and input before execution, and after any link failure.
pub fn synchronize_session(
    active: &mut Option<(u32, u32)>,
    session: &Session,
    manager: &ConnectionManager,
    rotary: &mut RotaryPipeline,
    desk: &mut DeskRuntime,
) {
    let current = session
        .current_session()
        .filter(|_| manager.state().can_drive_device())
        .map(|nonce| (session.connection_generation(), nonce));
    if *active == current {
        return;
    }
    if active.is_some() {
        desk.on_session_end();
        rotary.on_connection_state(
            ConnectionState::Connected,
            ConnectionState::Disconnected,
            None,
        );
    }
    rotary.on_connection_state(
        if active.is_some() {
            ConnectionState::Connected
        } else {
            ConnectionState::Disconnected
        },
        manager.state(),
        current.map(|(_, nonce)| nonce),
    );
    if current.is_some() {
        desk.on_session_begin();
    }
    *active = current;
}

/// The Slice 002 rotary pipeline owned by the device task: input ingress -> gesture value ->
/// presentation resolver, plus the typed activity for its failures.
///
/// `Presentation` carries only the transient volume overlay. Its `primary` is the interaction
/// channel (`Idle` unless a volume write is known to have failed); the companion director and the
/// device's mascot state still own the screen.
pub struct RotaryPipeline {
    session: Option<u32>,
    ingress: InputIngress,
    gesture_value: GestureValue,
    resolver: PresentationResolver,
    volume_failed: bool,
    audio_available: bool,
    /// A new volume-failure streak began since the last [`Self::take_volume_failure`].
    failure_started: bool,
}

impl RotaryPipeline {
    /// Starts with no session. The resolver's initial nonce is a placeholder: every entry to
    /// `Connected` rebinds it (and restarts `revision`) before any `Presentation` is resolved.
    #[must_use]
    pub fn new(backend: &dyn VolumeBackend) -> Self {
        Self {
            session: None,
            ingress: InputIngress::new(),
            gesture_value: GestureValue::new(),
            resolver: PresentationResolver::new(0),
            volume_failed: false,
            audio_available: is_available(backend),
            failure_started: false,
        }
    }

    /// Scopes input to the accepted nonce, including reconnects between observed state changes.
    pub fn on_connection_state(
        &mut self,
        _previous: ConnectionState,
        current: ConnectionState,
        session: Option<u32>,
    ) {
        let session = session.filter(|_| current.can_drive_device());
        if self.session == session {
            return;
        }
        self.ingress.end_session();
        self.gesture_value.end_session();
        if let Some(nonce) = session {
            self.ingress.begin_session(nonce);
            self.resolver.begin_session(nonce);
        }
        self.session = session;
    }

    /// Validates decoded input in order and hands each one to `desk` (the desk pipeline, which
    /// owns the active profile). Input `desk` returns `true` for belongs to a Volume gesture and
    /// also drives the volume loop, queueing any resulting presentations. Rejected input is
    /// dropped unexecuted and recorded by safe category only (never its payload). Input age is
    /// not judged here; see [`Self::accept_inputs_fresh`].
    pub fn accept_inputs(
        &mut self,
        events: &[InputEvent],
        backend: &dyn VolumeBackend,
        presentations: &mut Vec<Presentation>,
        mut desk: impl FnMut(LogicalInput) -> bool,
        observe: impl FnMut(SessionActivity),
    ) {
        self.accept_inputs_fresh(
            events,
            Freshness::default(),
            backend,
            presentations,
            |input, _remaining| desk(input),
            observe,
        );
    }

    /// [`Self::accept_inputs`] plus the age rule (issue #26): after the session and gesture
    /// checks, a discrete action older than the freshness limit is dropped and recorded as
    /// `InputStale`. Detents and gesture boundaries are always delivered. `desk` also receives
    /// how much longer the action may wait in the action queue.
    pub fn accept_inputs_fresh(
        &mut self,
        events: &[InputEvent],
        freshness: Freshness,
        backend: &dyn VolumeBackend,
        presentations: &mut Vec<Presentation>,
        mut desk: impl FnMut(LogicalInput, Duration) -> bool,
        mut observe: impl FnMut(SessionActivity),
    ) {
        for event in events {
            match self.ingress.accept(event) {
                Ok(Some(input)) => {
                    if input.is_discrete() && freshness.is_stale(event.device_ms) {
                        observe(SessionActivity::new(ActivityEventKind::InputStale, None));
                        continue;
                    }
                    if !desk(input, freshness.remaining(event.device_ms)) {
                        continue;
                    }
                    // Value detents of one gesture are staged and written once per pass, so a fast
                    // spin costs one OS round trip and one `Presentation`, not one per detent.
                    if let LogicalInput::Detent {
                        gesture_id,
                        direction,
                    } = input
                    {
                        self.gesture_value.stage_detent(gesture_id, direction);
                        continue;
                    }
                    self.flush_staged(backend, presentations, &mut observe);
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
        self.flush_staged(backend, presentations, &mut observe);
    }

    fn flush_staged(
        &mut self,
        backend: &dyn VolumeBackend,
        presentations: &mut Vec<Presentation>,
        observe: &mut impl FnMut(SessionActivity),
    ) {
        if let Some(update) = self.gesture_value.flush(backend) {
            self.push_update(update, presentations, observe);
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
            self.failure_started = true;
        }
        self.volume_failed = update.failed;
        presentations.push(self.resolver.resolve(&ProductSnapshot::with_value(update)));
    }
}

impl RotaryPipeline {
    /// Whether a new streak of failed volume writes began since the last call: shown once on the
    /// device as a volume Error, so a failed turn is never silent (gate 9).
    pub fn take_volume_failure(&mut self) -> bool {
        std::mem::take(&mut self.failure_started)
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
        DeviceCommand::PlayMascotAction(_) => {
            social_cue.map(|cue| RuntimeActivityRequest::SocialAction {
                action: cue.action,
                personality: cue.personality,
                seed: cue.seed,
                autonomous: false,
            })
        }
        DeviceCommand::FlashFirmware
        | DeviceCommand::Refresh
        | DeviceCommand::SetDisplayMode(_)
        | DeviceCommand::TestAction(_)
        | DeviceCommand::ApplyConfig(_) => None,
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
    /// Candidate bookkeeping plus the elapsed time to stamp failures with; absent in pinned or
    /// host-test recoveries, which then attribute nothing to a port.
    rotator: Option<(&'a mut CandidateRotator, Duration)>,
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
            rotator: None,
        }
    }

    /// Attributes the failure to the port that was being tried, so discovery rotates past it.
    #[must_use]
    pub fn with_rotator(mut self, rotator: &'a mut CandidateRotator, now: Duration) -> Self {
        self.rotator = Some((rotator, now));
        self
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
    let handshake_timeout = matches!(event, ManagerEvent::HandshakeTimeout);
    manager.apply(event);
    *recovery.link = None;
    let failed_port = recovery.connected_port.take();
    if let (Some((rotator, now)), Some(port)) = (recovery.rotator, failed_port) {
        if handshake_timeout {
            rotator.record_handshake_timeout(&port, now);
        } else {
            rotator.record_failure(&port, now);
        }
    }
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

/// Publishes [`DiagnosticsSnapshot`] at most once a second and remembers when each connection
/// came up.
#[derive(Default)]
struct DiagnosticsPublisher {
    last_published: Option<Instant>,
    connected_since: Option<Instant>,
    connections: u32,
}

impl DiagnosticsPublisher {
    const INTERVAL: Duration = Duration::from_secs(1);

    fn publish(
        &mut self,
        cell: &Mutex<DiagnosticsSnapshot>,
        manager: &ConnectionManager,
        session: &Session,
        host: HostServicesSnapshot,
    ) {
        let now = Instant::now();
        // Track the connection every loop so a short blip is not missed between publishes.
        let connected = manager.state().can_drive_device();
        if connected && self.connected_since.is_none() {
            self.connected_since = Some(now);
            self.connections = self.connections.saturating_add(1);
        } else if !connected {
            self.connected_since = None;
        }
        if self
            .last_published
            .is_some_and(|at| now.duration_since(at) < Self::INTERVAL)
        {
            return;
        }
        self.last_published = Some(now);
        let device = manager.device().filter(|_| connected);
        *cell.lock().expect("diagnostics lock") = DiagnosticsSnapshot {
            connection: crate::ipc::dto::connection_token(manager.state()),
            firmware_version: device.map(|d| {
                format!(
                    "{}.{}.{}",
                    d.firmware_version.major, d.firmware_version.minor, d.firmware_version.patch
                )
            }),
            device_hash: device.map(|d| d.device_id_hash_short.clone()),
            negotiated_caps: session.negotiated_caps(),
            negotiated_minor: device.map(|d| {
                d.protocol_version
                    .minor
                    .min(kivori_protocol::PROTOCOL_MINOR)
            }),
            connected_since: self.connected_since,
            reconnects: self.connections.saturating_sub(1),
            retry_count: manager.retry_count(),
            session: session.diagnostics(),
            host,
        };
    }
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
