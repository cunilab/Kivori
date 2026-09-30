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
