//! Firmware flashing state-machine coverage without hardware or a real `espflash` process.

use kivori_desktop::activity::ActivityEventKind;
use kivori_desktop::firmware::{FirmwarePhase, FlashWorkflow, ResumeTarget};
use kivori_desktop::runtime::device_task::run_accepted_firmware_flash;
use kivori_desktop::runtime::state::{AppState, DeviceCommand};
use kivori_desktop::{activity::ActivityLog, ipc::dto};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

struct FakeFlasher {
    result: Result<(), &'static str>,
}

impl FakeFlasher {
    fn run(&self) -> Result<(), &'static str> {
        self.result
    }
}

#[test]
fn rejects_disconnected_and_duplicate_flash_requests() {
    let mut flash = FlashWorkflow::new(true, 512);
    assert!(flash
        .request(false, Some("COM7"), Some("deadbeef"))
        .is_err());
    assert_eq!(flash.status().phase, FirmwarePhase::Idle);

    assert_eq!(
        flash.request(true, Some("COM7"), Some("deadbeef")).unwrap(),
        "COM7"
    );
    assert_eq!(flash.status().phase, FirmwarePhase::Preparing);
    assert!(flash.request(true, Some("COM7"), Some("deadbeef")).is_err());
}

#[test]
fn failed_flash_resumes_normal_connection_work() {
    let mut flash = FlashWorkflow::new(true, 512);
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    let fake = FakeFlasher {
        result: Err("Firmware flashing timed out."),
    };

    assert_eq!(flash.finish(fake.run()), ResumeTarget::Discovery);
    assert_eq!(flash.status().phase, FirmwarePhase::Failed);
    assert_eq!(flash.status().message, "Firmware flashing timed out.");
}

#[test]
fn app_state_reserves_flash_before_queuing_a_second_request() {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let thread_cancel = Arc::clone(&cancel);
    let thread = std::thread::spawn(move || {
        while !thread_cancel.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
    });
    let mut connected = dto::initial_status();
    connected.connection = "connected".to_string();
    let state = AppState::new(
        false,
        Arc::new(Mutex::new(connected)),
        Arc::new(ActivityLog::new(8)),
        tx,
        Arc::clone(&cancel),
        thread,
    );

    state.firmware_status.lock().unwrap().available = false;
    assert!(state.queue_firmware_flash().is_err());
    assert!(rx.try_recv().is_err());
    state.firmware_status.lock().unwrap().available = true;
    assert!(state.queue_firmware_flash().is_ok());
    assert_eq!(rx.recv().unwrap(), DeviceCommand::FlashFirmware);
    assert!(state.queue_firmware_flash().is_err());
    assert_eq!(
        state.firmware_status_snapshot().phase,
        FirmwarePhase::Preparing
    );
    state.shutdown();
}

#[test]
fn successful_flash_waits_for_handshake_on_the_same_port() {
    let mut flash = FlashWorkflow::new(true, 512);
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    let fake = FakeFlasher { result: Ok(()) };

    assert_eq!(
        flash.finish(fake.run()),
        ResumeTarget::SamePort("COM7".to_string())
    );
    assert_eq!(flash.status().phase, FirmwarePhase::Reconnecting);
    assert!(!flash.handshake("COM8", "deadbeef", true));
    assert_eq!(flash.status().phase, FirmwarePhase::Reconnecting);
    assert!(!flash.handshake("COM7", "not-same", true));
    assert!(flash.handshake("COM7", "deadbeef", true));
    assert_eq!(flash.status().phase, FirmwarePhase::Succeeded);
}

#[test]
fn swapped_board_on_the_same_port_is_not_reported_as_success() {
    let mut flash = FlashWorkflow::new(true, 512);
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    flash.finish(FakeFlasher { result: Ok(()) }.run());

    // Same port, compatible firmware, but a different unit's device hash.
    assert!(!flash.handshake("COM7", "cafef00d", true));
    assert_eq!(flash.status().phase, FirmwarePhase::Reconnecting);

    // The swapped board never turns into a success; the reconnect window ends in failure.
    flash.reconnect_timed_out();
    assert_eq!(flash.status().phase, FirmwarePhase::Failed);
    assert!(!flash.handshake("COM7", "deadbeef", true));
}

#[test]
fn reconnect_deadline_fails_even_without_a_handshake() {
    let mut flash = FlashWorkflow::new(true, 512);
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    assert_eq!(
        flash.finish(Ok::<(), &str>(())),
        ResumeTarget::SamePort("COM7".to_string())
    );

    // This is the state reached when the port opens but remains silent until the bounded deadline.
    flash.reconnect_timed_out();
    assert_eq!(flash.status().phase, FirmwarePhase::Failed);
    assert!(flash.status().message.contains("could not be verified"));
}

#[test]
fn accepted_firmware_workflow_queues_closed_ordered_phases_without_native_details() {
    let mut flash = FlashWorkflow::new(true, 512);
    assert_eq!(
        flash
            .drain_activity()
            .into_iter()
            .map(|item| item.kind)
            .collect::<Vec<_>>(),
        [ActivityEventKind::FirmwareAvailable]
    );
    assert!(flash
        .request(false, Some("COM7"), Some("deadbeef"))
        .is_err());
    assert!(
        flash.drain_activity().is_empty(),
        "rejected work is not accepted work"
    );
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    flash.mark_serial_released();
    flash.mark_flashing();
    assert_eq!(
        flash.finish(Ok::<(), &str>(())),
        ResumeTarget::SamePort("COM7".to_string())
    );
    assert!(flash.handshake("COM7", "deadbeef", true));
    let observations = flash.drain_activity();
    assert!(
        observations.iter().all(|item| item.metadata.is_none()),
        "firmware phases carry no native details"
    );
    assert_eq!(
        observations
            .into_iter()
            .map(|item| item.kind)
            .collect::<Vec<_>>(),
        [
            ActivityEventKind::FirmwareFlashRequested,
            ActivityEventKind::FirmwarePreparing,
            ActivityEventKind::FirmwareSerialReleased,
            ActivityEventKind::FirmwareFlasherStarted,
            ActivityEventKind::FirmwareFlashSucceeded,
            ActivityEventKind::FirmwareReconnectWaiting,
            ActivityEventKind::FirmwarePostFlashVerified,
        ]
    );
}

#[test]
fn accepted_flash_drains_each_phase_before_blocking_work() {
    use std::cell::RefCell;

    let mut flash = FlashWorkflow::new(true, 512);
    flash.drain_activity();
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    let seen = RefCell::new(Vec::new());

    let resume = run_accepted_firmware_flash(
        &mut flash,
        |status| {
            assert_eq!(status.phase, FirmwarePhase::Flashing);
            assert_eq!(
                *seen.borrow(),
                [
                    ActivityEventKind::FirmwareFlashRequested,
                    ActivityEventKind::FirmwarePreparing,
                    ActivityEventKind::FirmwareSerialReleased,
                    ActivityEventKind::FirmwareFlasherStarted,
                ],
                "accepted, preparation, serial release, and flasher start must be visible before the blocking flasher runs"
            );
            Ok::<(), &str>(())
        },
        |observation| seen.borrow_mut().push(observation.kind),
    );

    assert_eq!(resume, ResumeTarget::SamePort("COM7".to_string()));
    assert_eq!(
        seen.into_inner(),
        [
            ActivityEventKind::FirmwareFlashRequested,
            ActivityEventKind::FirmwarePreparing,
            ActivityEventKind::FirmwareSerialReleased,
            ActivityEventKind::FirmwareFlasherStarted,
            ActivityEventKind::FirmwareFlashSucceeded,
            ActivityEventKind::FirmwareReconnectWaiting,
        ]
    );
}

#[test]
fn workflow_owns_initial_availability_and_preparation_rejection() {
    let mut flash = FlashWorkflow::new(false, 0);
    assert_eq!(
        flash
            .drain_activity()
            .into_iter()
            .map(|item| item.kind)
            .collect::<Vec<_>>(),
        [ActivityEventKind::FirmwareUnavailable]
    );

    let mut flash = FlashWorkflow::new(true, 512);
    flash.drain_activity();
    flash.fail_preparation();
    assert_eq!(
        flash
            .drain_activity()
            .into_iter()
            .map(|item| item.kind)
            .collect::<Vec<_>>(),
        [ActivityEventKind::FirmwarePreparationRejected]
    );
}
