//! End-to-end host-sim loop (T097; US1 + US2 + US3). The real desktop `Session` and the real firmware
//! `Dispatcher` exchange encoded frames over an in-memory duplex pipe. This proves the two independently
//! tested state machines actually interoperate: the device converges to the desktop's desired state,
//! and reconnect resynchronizes it — all with no Tauri, no serial port, and no hardware.

use std::collections::VecDeque;
use std::convert::Infallible;

use kivori_desktop::device::fsm::{ConnectionManager, ManagerEvent};
use kivori_desktop::device::session::{Session, SessionConfig};
use kivori_desktop::device::transport::SerialLink;
use kivori_desktop::orchestrator::Orchestrator;
use kivori_firmware::ports::Transport;
use kivori_firmware::proto::{DeviceIdentity, Dispatcher};
use kivori_firmware::state::{DeviceEvent, DeviceState};
use kivori_model::presentation::{PrimaryState, ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Capabilities, CompanionState, ConnectionState, SendableState};
use kivori_protocol::message::Presentation;
use kivori_protocol::{ControlId, FirmwareVersion, InputEvent, InputKind};

/// A shared in-memory duplex link: two byte queues between the desktop and the firmware.
#[derive(Default)]
struct Wire {
    desktop_to_device: VecDeque<u8>,
    device_to_desktop: VecDeque<u8>,
}

fn drain_into(src: &mut VecDeque<u8>, buf: &mut [u8]) -> usize {
    let mut n = 0;
    while n < buf.len() {
        match src.pop_front() {
            Some(byte) => {
                buf[n] = byte;
                n += 1;
            }
            None => break,
        }
    }
    n
}

/// Desktop end of the wire (the desktop's `SerialLink`).
struct HostEnd<'a>(&'a mut Wire);
impl SerialLink for HostEnd<'_> {
    type Error = Infallible;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Infallible> {
        Ok(drain_into(&mut self.0.device_to_desktop, buf))
    }
    fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        self.0.desktop_to_device.extend(buf.iter().copied());
        Ok(buf.len())
    }
}

/// Firmware end of the wire (the firmware's `Transport`).
struct DeviceEnd<'a>(&'a mut Wire);
impl Transport for DeviceEnd<'_> {
    type Error = Infallible;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Infallible> {
        Ok(drain_into(&mut self.0.desktop_to_device, buf))
    }
    fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        self.0.device_to_desktop.extend(buf.iter().copied());
        Ok(buf.len())
    }
}

fn device_identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: [0x11; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        // Matches the real firmware identities (Slice 002): both bits are implemented, so both
        // are advertised — negotiation (the desktop side's own `SessionConfig::default()`
        // advertises both too) is what actually gates behaviour.
        capabilities: Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::PRESENTATION_V1),
    }
}

/// Pumps both sides until no bytes remain in flight (or a bounded number of rounds elapses).
#[allow(clippy::too_many_arguments)]
fn settle(
    wire: &mut Wire,
    session: &mut Session,
    manager: &mut ConnectionManager,
    orch: &mut Orchestrator,
    dispatcher: &mut Dispatcher,
    device: &mut DeviceState,
    now_ms: u32,
) {
    for _ in 0..8 {
        session
            .pump(&mut HostEnd(&mut *wire), manager, orch)
            .expect("desktop pump");
        dispatcher
            .poll(&mut DeviceEnd(&mut *wire), device, now_ms)
            .expect("firmware poll");
        if wire.desktop_to_device.is_empty() && wire.device_to_desktop.is_empty() {
            break;
        }
    }
}

#[test]
fn full_mvp_loop_connects_converges_and_reconnects() {
    let mut wire = Wire::default();

    // Desktop side.
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orch = Orchestrator::new();

    // Firmware side: boots to `offline` before any host drives it (device-originated).
    let mut dispatcher = Dispatcher::new(device_identity());
    let mut device = DeviceState::new();
    device.apply(DeviceEvent::BootComplete);
    assert_eq!(device.current(), CompanionState::Offline);

    // US1 + US2: the desktop wants `busy`; on connect the device must converge to it via resync.
    orch.set_desired(SendableState::Busy);
    session
        .open(&mut HostEnd(&mut wire), &mut manager)
        .expect("open");
    settle(
        &mut wire,
        &mut session,
        &mut manager,
        &mut orch,
        &mut dispatcher,
        &mut device,
        100,
    );

    assert_eq!(
        manager.state(),
        ConnectionState::Connected,
        "desktop connected"
    );
    assert!(
        manager.device().is_some(),
        "device identity captured via handshake"
    );
    assert_eq!(
        device.current(),
        CompanionState::Busy,
        "device converged to desired (resync)"
    );

    // US2: change the state → the device follows.
    session
        .set_desired(
            &mut HostEnd(&mut wire),
            &manager,
            &mut orch,
            SendableState::Happy,
        )
        .expect("set_desired");
    settle(
        &mut wire,
        &mut session,
        &mut manager,
        &mut orch,
        &mut dispatcher,
        &mut device,
        200,
    );
    assert_eq!(
        device.current(),
        CompanionState::Happy,
        "device followed the state change"
    );

    // US3: unplug → reconnect resynchronizes the current desired state (still `happy`).
    manager.apply(ManagerEvent::PortRemoved);
    device.apply(DeviceEvent::LinkDown);
    assert_eq!(
        device.current(),
        CompanionState::Offline,
        "device offline on link loss"
    );

    session
        .open(&mut HostEnd(&mut wire), &mut manager)
        .expect("reopen");
    settle(
        &mut wire,
        &mut session,
        &mut manager,
        &mut orch,
        &mut dispatcher,
        &mut device,
        300,
    );
    assert_eq!(
        manager.state(),
        ConnectionState::Connected,
        "desktop reconnected"
    );
    assert_eq!(
        device.current(),
        CompanionState::Happy,
        "reconnect resynced the within-process desired state"
    );
}

#[test]
fn booting_and_offline_are_never_transmitted() {
    // Type-level guarantee, asserted at the seam: the desktop can only ever send a `SendableState`,
    // and after a full connect the device shows a driven state — never `booting`/`offline` from the host.
    let mut wire = Wire::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orch = Orchestrator::new();
    let mut dispatcher = Dispatcher::new(device_identity());
    let mut device = DeviceState::new();
    device.apply(DeviceEvent::BootComplete);

    for state in [
        SendableState::Idle,
        SendableState::Happy,
        SendableState::Busy,
        SendableState::Sleeping,
    ] {
        orch.set_desired(state);
        session
            .open(&mut HostEnd(&mut wire), &mut manager)
            .expect("open");
        settle(
            &mut wire,
            &mut session,
            &mut manager,
            &mut orch,
            &mut dispatcher,
            &mut device,
            10,
        );
        assert_eq!(device.current(), state.to_companion());
        assert!(
            !device.current().is_device_originated(),
            "a host-driven state is never booting/offline"
        );
        manager.apply(ManagerEvent::PortRemoved);
        device.apply(DeviceEvent::LinkDown);
    }
}

#[test]
fn negotiated_capabilities_actually_carry_an_input_event_and_a_presentation() {
    // Task-11 review round 1, finding 2: capability bits being allocated and gated is worthless
    // if nothing ever advertises them — this proves the real desktop `Session` and real firmware
    // `Dispatcher` not only negotiate PHYSICAL_INPUT_V1 + PRESENTATION_V1 with each other, but
    // that a real InputEvent and a real Presentation actually cross the wire once negotiated.
    let mut wire = Wire::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orch = Orchestrator::new();
    let mut dispatcher = Dispatcher::new(device_identity());
    let mut device = DeviceState::new();
    device.apply(DeviceEvent::BootComplete);

    session
        .open(&mut HostEnd(&mut wire), &mut manager)
        .expect("open");
    settle(
        &mut wire,
        &mut session,
        &mut manager,
        &mut orch,
        &mut dispatcher,
        &mut device,
        100,
    );
    assert_eq!(manager.state(), ConnectionState::Connected);
    assert!(
        session
            .negotiated_caps()
            .contains(Capabilities::PHYSICAL_INPUT_V1),
        "the desktop must negotiate PHYSICAL_INPUT_V1 with a device that advertises it"
    );
    assert!(
        session
            .negotiated_caps()
            .contains(Capabilities::PRESENTATION_V1),
        "the desktop must negotiate PRESENTATION_V1 with a device that advertises it"
    );

    // Firmware -> desktop: a real InputEvent, gated on the negotiated capability and the accepted
    // session, must actually reach the desktop's `Session`.
    let nonce = dispatcher.accepted_session().expect("accepted session");
    let sent =
        dispatcher.send_input_event(&mut DeviceEnd(&mut wire), 1, InputKind::GestureStarted, 100);
    assert!(
        sent,
        "a negotiated capability and an accepted session must emit"
    );
    session
        .pump(&mut HostEnd(&mut wire), &mut manager, &mut orch)
        .expect("desktop pump");
    assert_eq!(
        session.take_input_events(),
        vec![InputEvent {
            session: nonce,
            gesture_id: 1,
            control: ControlId::Rotary,
            kind: InputKind::GestureStarted,
            device_ms: 100,
        }],
        "the InputEvent must actually reach the desktop's Session"
    );

    // Desktop -> firmware: a real Presentation, gated on the negotiated capability, must actually
    // reach the firmware's `Dispatcher`.
    let presentation = Presentation {
        session: nonce,
        revision: 1,
        primary: PrimaryState::Idle,
        value: Some(ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: 42,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
        }),
        transient_ms: 800,
    };
    session
        .send_presentation(&mut HostEnd(&mut wire), presentation)
        .expect("send_presentation");
    dispatcher
        .poll(&mut DeviceEnd(&mut wire), &mut device, 200)
        .expect("firmware poll");
    assert_eq!(
        dispatcher.take_presentation(),
        Some(presentation),
        "the Presentation must actually reach the firmware's Dispatcher"
    );
}

/// M1 across the real boundary: a push-switch Press and Hold, and a contextual-button Press, leave
/// the real firmware dispatcher,
/// pass the desktop's session freshness checks, run their bound actions against fake OS services,
/// and the classified outcome plus the desk status arrive back at the firmware dispatcher.
#[test]
fn a_button_press_round_trips_to_an_honest_feedback_and_a_desk_status() {
    use kivori_desktop::desk::DeskRuntime;
    use kivori_desktop::platform::system::NoSystemProbe;
    use kivori_desktop::platform::{
        FakeInputSynth, FakeMediaObserver, FakeVolumeBackend, OsServices,
    };
    use kivori_desktop::runtime::device_task::RotaryPipeline;
    use kivori_model::desk::{ActionKind, FeedbackKind};
    use kivori_protocol::Feedback;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let mut wire = Wire::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orch = Orchestrator::new();
    let identity = DeviceIdentity {
        capabilities: Capabilities::PHYSICAL_INPUT_V1
            .union(Capabilities::BUTTON_INPUT_V1)
            .union(Capabilities::DESK_STATUS_V1)
            .union(Capabilities::ACTION_FEEDBACK_V1)
            .union(Capabilities::CONTEXT_BUTTONS_V1),
        ..device_identity()
    };
    let mut dispatcher = Dispatcher::new(identity);
    let mut device = DeviceState::new();
    device.apply(DeviceEvent::BootComplete);
    session
        .open(&mut HostEnd(&mut wire), &mut manager)
        .expect("open");
    settle(
        &mut wire,
        &mut session,
        &mut manager,
        &mut orch,
        &mut dispatcher,
        &mut device,
        100,
    );
    assert_eq!(manager.state(), ConnectionState::Connected);
    let nonce = dispatcher.accepted_session().expect("accepted session");

    let volume = Arc::new(FakeVolumeBackend::new(30));
    let mut rotary = RotaryPipeline::new(volume.as_ref());
    rotary.on_connection_state(
        ConnectionState::Connecting,
        ConnectionState::Connected,
        session.current_session(),
    );
    let mut desk = DeskRuntime::new(OsServices {
        volume: volume.clone(),
        synth: Arc::new(FakeInputSynth::new(Ok(()))),
        media: Arc::new(FakeMediaObserver::default()),
        system: Box::new(NoSystemProbe),
        clock: || None,
        foreground: Arc::new(kivori_desktop::platform::FakeForeground::default()),
    });
    desk.on_session_begin();

    let started = Instant::now();
    let mut received = Vec::new();
    for (control, id, kind) in [
        (ControlId::Button, 1, InputKind::Press),
        (ControlId::Button, 2, InputKind::Hold),
        // The right contextual button: Next track by default.
        (ControlId::ContextButton(2), 3, InputKind::Press),
    ] {
        assert!(dispatcher.send_button_event(&mut DeviceEnd(&mut wire), control, id, kind, 500));
        session
            .pump(&mut HostEnd(&mut wire), &mut manager, &mut orch)
            .expect("desktop pump");
        let inputs = session.take_input_events();
        rotary.accept_inputs(
            &inputs,
            volume.as_ref(),
            &mut Vec::new(),
            |input| desk.on_input(&input, started.elapsed(), &mut |_| {}),
            |_| {},
        );
        // The action runs on the worker thread; collect its outcome as the device task would.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let out = desk.tick(started.elapsed(), &mut |_| {});
            if let Some(status) = out.status {
                assert!(session
                    .send_status(&mut HostEnd(&mut wire), status)
                    .expect("send_status"));
            }
            let done = !out.feedback.is_empty();
            for feedback in out.feedback {
                assert!(session
                    .send_feedback(&mut HostEnd(&mut wire), feedback)
                    .expect("send_feedback"));
            }
            dispatcher
                .poll(&mut DeviceEnd(&mut wire), &mut device, 600)
                .expect("firmware poll");
            if let Some(feedback) = dispatcher.take_feedback() {
                received.push(feedback);
            }
            if done {
                break;
            }
            assert!(Instant::now() < deadline, "no outcome for {kind:?}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    assert_eq!(
        received,
        [
            // A media key's effect cannot be observed here: never shown as success.
            Feedback {
                session: nonce,
                action: ActionKind::PlayPause,
                kind: FeedbackKind::Unverified,
            },
            // Mute was read back from the (fake) OS: observed state.
            Feedback {
                session: nonce,
                action: ActionKind::Mute,
                kind: FeedbackKind::StateConfirmed,
            },
            Feedback {
                session: nonce,
                action: ActionKind::NextTrack,
                kind: FeedbackKind::Unverified,
            },
        ]
    );
    assert_eq!(
        kivori_desktop::platform::VolumeBackend::read_mute(volume.as_ref()),
        Ok(true)
    );
    let status = dispatcher
        .take_status()
        .expect("a desk status reached the device");
    assert_eq!(status.session, nonce);
    assert_eq!(status.status.volume_percent, Some(30));
}

#[test]
fn reboot_without_bye_expires_the_old_session_and_restores_the_unchanged_mode() {
    use kivori_desktop::desk::DeskRuntime;
    use kivori_desktop::input::LogicalInput;
    use kivori_desktop::platform::system::NoSystemProbe;
    use kivori_desktop::platform::{
        FakeForeground, FakeInputSynth, FakeMediaObserver, FakeVolumeBackend, OsServices,
    };
    use kivori_desktop::runtime::device_task::{synchronize_session, RotaryPipeline};
    use kivori_model::desk::DisplayMode;
    use std::sync::Arc;
    use std::time::Duration;

    let mut wire = Wire::default();
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orch = Orchestrator::new();
    let identity = DeviceIdentity {
        capabilities: device_identity()
            .capabilities
            .union(Capabilities::DESK_STATUS_V1),
        ..device_identity()
    };
    let mut dispatcher = Dispatcher::new(identity);
    let mut device = DeviceState::new();
    device.apply(DeviceEvent::BootComplete);
    let volume = Arc::new(FakeVolumeBackend::new(30));
    let mut rotary = RotaryPipeline::new(volume.as_ref());
    let mut desk = DeskRuntime::new(OsServices {
        volume: volume.clone(),
        synth: Arc::new(FakeInputSynth::new(Ok(()))),
        media: Arc::new(FakeMediaObserver::default()),
        system: Box::new(NoSystemProbe),
        clock: || None,
        foreground: Arc::new(FakeForeground::default()),
    });
    desk.set_mode(DisplayMode::Volume, &mut |_| {});
    let mut active = None;
    session.open(&mut HostEnd(&mut wire), &mut manager).unwrap();
    settle(
        &mut wire,
        &mut session,
        &mut manager,
        &mut orch,
        &mut dispatcher,
        &mut device,
        100,
    );
    synchronize_session(&mut active, &session, &manager, &mut rotary, &mut desk);
    let old_nonce = session.current_session().unwrap();
    let initial = desk.tick(Duration::ZERO, &mut |_| {}).status.unwrap();
    session
        .send_status(&mut HostEnd(&mut wire), initial)
        .unwrap();
    dispatcher
        .poll(&mut DeviceEnd(&mut wire), &mut device, 100)
        .unwrap();
    assert_eq!(
        dispatcher.take_status().unwrap().status.mode,
        DisplayMode::Volume
    );

    // Power loss discards firmware state without delivering Bye or removing the serial port.
    dispatcher = Dispatcher::new(identity);
    device = DeviceState::new();
    device.apply(DeviceEvent::BootComplete);
    for second in 1..=3 {
        session
            .send_ping(&mut HostEnd(&mut wire), second * 1000)
            .unwrap();
        settle(
            &mut wire,
            &mut session,
            &mut manager,
            &mut orch,
            &mut dispatcher,
            &mut device,
            second * 1000,
        );
        assert_eq!(session.heartbeat_timed_out(), second == 3);
    }
    manager.apply(ManagerEvent::HeartbeatTimeout);
    synchronize_session(&mut active, &session, &manager, &mut rotary, &mut desk);
    assert_eq!(active, None);
    assert!(manager.apply(ManagerEvent::BackoffElapsed));
    session.open(&mut HostEnd(&mut wire), &mut manager).unwrap();
    settle(
        &mut wire,
        &mut session,
        &mut manager,
        &mut orch,
        &mut dispatcher,
        &mut device,
        4000,
    );
    assert_eq!(manager.state(), ConnectionState::Connected);
    let new_nonce = session.current_session().unwrap();
    assert_ne!(old_nonce, new_nonce);

    // First input may already be queued before the task initializes the newly accepted session.
    assert!(dispatcher.send_input_event(
        &mut DeviceEnd(&mut wire),
        1,
        InputKind::GestureStarted,
        4000
    ));
    session
        .pump(&mut HostEnd(&mut wire), &mut manager, &mut orch)
        .unwrap();
    synchronize_session(&mut active, &session, &manager, &mut rotary, &mut desk);
    let mut accepted = Vec::new();
    rotary.accept_inputs(
        &session.take_input_events(),
        volume.as_ref(),
        &mut Vec::new(),
        |input| {
            accepted.push(input);
            false
        },
        |_| {},
    );
    assert_eq!(
        accepted,
        vec![LogicalInput::GestureStarted { gesture_id: 1 }]
    );

    let restored = desk
        .tick(Duration::from_secs(4), &mut |_| {})
        .status
        .unwrap();
    assert_eq!(restored.mode, DisplayMode::Volume);
    session
        .send_status(&mut HostEnd(&mut wire), restored)
        .unwrap();
    // The first locally accepted Status is lost; unchanged facts must still be refreshed.
    wire.desktop_to_device.clear();
    let retry = desk
        .tick(Duration::from_secs(5), &mut |_| {})
        .status
        .unwrap();
    session.send_status(&mut HostEnd(&mut wire), retry).unwrap();
    dispatcher
        .poll(&mut DeviceEnd(&mut wire), &mut device, 5000)
        .unwrap();
    let received = dispatcher.take_status().unwrap();
    assert_eq!(received.session, new_nonce);
    assert_eq!(received.status.mode, DisplayMode::Volume);
}
