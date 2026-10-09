//! Firmware flashing state-machine coverage without hardware or a real `espflash` process.

use kivori_desktop::activity::ActivityEventKind;
use kivori_desktop::device::discovery::PortCandidate;
use kivori_desktop::firmware::{
    classify_espflash, find_espflash_in, update_advice, EspflashExit, FirmwarePhase, FlashFailure,
    FlashWorkflow, ResumeTarget, UpdateAdvice,
};
use kivori_desktop::runtime::device_task::run_accepted_firmware_flash;
use kivori_desktop::runtime::state::{AppState, DeviceCommand};
use kivori_desktop::{activity::ActivityLog, ipc::dto};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

struct FakeFlasher {
    result: Result<(), FlashFailure>,
}

impl FakeFlasher {
    fn run(&self) -> Result<(), FlashFailure> {
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
        result: Err(FlashFailure::Timeout),
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
        flash.finish(Ok(())),
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
        flash.finish(Ok(())),
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
            Ok(())
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

fn kivori_port(name: &str) -> PortCandidate {
    PortCandidate::new(name, Some(0x303A), Some(0x1001))
}

#[test]
fn recovery_is_refused_without_exactly_one_allowlisted_port() {
    let mut flash = FlashWorkflow::new(true, 512);
    assert!(flash.request_recovery(&[]).is_err());
    // A non-allowlisted port does not count.
    let other = PortCandidate::new("COM1", Some(0x0403), Some(0x6001));
    assert!(flash
        .request_recovery(std::slice::from_ref(&other))
        .is_err());
    assert!(flash
        .request_recovery(&[kivori_port("COM7"), kivori_port("COM8")])
        .is_err());
    assert_eq!(flash.status().phase, FirmwarePhase::Idle);

    assert_eq!(
        flash
            .request_recovery(&[other, kivori_port("COM7")])
            .unwrap(),
        "COM7"
    );
    assert_eq!(flash.status().phase, FirmwarePhase::Preparing);
    assert!(flash.request_recovery(&[kivori_port("COM7")]).is_err());
}

#[test]
fn recovery_accepts_a_new_device_hash_and_records_it() {
    let mut flash = FlashWorkflow::new(true, 512);
    flash.request_recovery(&[kivori_port("COM7")]).unwrap();
    assert_eq!(
        flash.finish(Ok(())),
        ResumeTarget::SamePort("COM7".to_string())
    );
    assert!(!flash.handshake("COM8", "0badf00d", true), "wrong port");
    assert!(!flash.handshake("COM7", "0badf00d", false), "incompatible");
    assert!(flash.handshake("COM7", "0badf00d", true));
    assert_eq!(flash.status().phase, FirmwarePhase::Succeeded);
    assert_eq!(flash.recovered_hash(), Some("0badf00d"));
}

#[test]
fn verified_flash_still_rejects_a_different_hash() {
    let mut flash = FlashWorkflow::new(true, 512);
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    flash.finish(Ok(()));
    assert!(!flash.handshake("COM7", "0badf00d", true));
    assert_eq!(flash.status().phase, FirmwarePhase::Reconnecting);
    assert_eq!(flash.recovered_hash(), None);
}

fn app_state_with_connection(connection: &str) -> (AppState, mpsc::Receiver<DeviceCommand>) {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let thread_cancel = Arc::clone(&cancel);
    let thread = std::thread::spawn(move || {
        while !thread_cancel.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
    });
    let mut status = dto::initial_status();
    status.connection = connection.to_string();
    let state = AppState::new(
        false,
        Arc::new(Mutex::new(status)),
        Arc::new(ActivityLog::new(8)),
        tx,
        cancel,
        thread,
    );
    state.firmware_status.lock().unwrap().available = true;
    (state, rx)
}

#[test]
fn flashing_is_allowed_from_incompatible_but_not_from_disconnected() {
    let (state, rx) = app_state_with_connection("incompatible");
    assert!(state.queue_firmware_flash().is_ok());
    assert_eq!(rx.recv().unwrap(), DeviceCommand::FlashFirmware);
    state.shutdown();

    let (state, rx) = app_state_with_connection("disconnected");
    assert!(state.queue_firmware_flash().is_err());
    assert!(rx.try_recv().is_err());
    assert_eq!(state.firmware_status_snapshot().phase, FirmwarePhase::Idle);
    state.shutdown();
}

#[test]
fn restore_queues_from_any_connection_but_not_twice() {
    let (state, rx) = app_state_with_connection("disconnected");
    assert!(state.queue_firmware_restore().is_ok());
    assert_eq!(rx.recv().unwrap(), DeviceCommand::RestoreFirmware);
    assert!(state.queue_firmware_restore().is_err());
    state.shutdown();
}

#[test]
fn failure_tokens_ride_the_status_and_clear_on_the_next_attempt() {
    let mut flash = FlashWorkflow::new(true, 512);
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    flash.finish(Err(FlashFailure::PortBusy));
    assert_eq!(flash.status().failure, Some(FlashFailure::PortBusy));
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    assert_eq!(flash.status().failure, None);
    flash.finish(Ok(()));
    flash.reconnect_timed_out();
    assert_eq!(
        flash.status().failure,
        Some(FlashFailure::ReconnectTimedOut)
    );
}

#[test]
fn classifier_fixtures_cover_every_class() {
    use EspflashExit::{Cancelled, Code, ImageUnavailable, TimedOut, ToolMissing};
    let fixtures: &[(EspflashExit, &str, FlashFailure)] = &[
        (ToolMissing, "", FlashFailure::ToolMissing),
        (ImageUnavailable, "", FlashFailure::ImageUnavailable),
        (TimedOut, "", FlashFailure::Timeout),
        (Cancelled, "", FlashFailure::Cancelled),
        (
            Code(Some(1)),
            "Error:   x Error while connecting to device\n  `-> Failed to open serial port COM7\n  `-> Access is denied. (os error 5)",
            FlashFailure::PortBusy,
        ),
        (
            Code(Some(1)),
            "Error: serialport::Error { kind: Io(PermissionDenied), description: \"Permission denied\" }",
            FlashFailure::PortBusy,
        ),
        (
            Code(Some(1)),
            "Error: Device or resource busy",
            FlashFailure::PortBusy,
        ),
        (
            Code(Some(1)),
            "Error:   x Error while connecting to device\n  `-> Failed to connect to Espressif device: No serial data received.",
            FlashFailure::NoDownloadMode,
        ),
        (
            Code(Some(1)),
            "Error: Wrong boot mode detected (0x13)! The chip needs to be in download mode.",
            FlashFailure::NoDownloadMode,
        ),
        (
            Code(Some(1)),
            "Error: Failed to connect to Espressif device: Invalid response",
            FlashFailure::NoDownloadMode,
        ),
        (
            Code(Some(1)),
            "Error:   x Timeout while running command\n",
            FlashFailure::Timeout,
        ),
        (
            Code(Some(1)),
            "Error: Failed to parse ELF file",
            FlashFailure::ImageUnavailable,
        ),
        (Code(Some(1)), "something nobody has seen", FlashFailure::Unknown),
        (Code(Some(2)), "", FlashFailure::Unknown),
        (Code(None), "", FlashFailure::Unknown),
    ];
    for (exit, stderr, expected) in fixtures {
        assert_eq!(classify_espflash(*exit, stderr), *expected, "{stderr}");
    }
    // The fixtures reach every class except the workflow-assigned one.
    for class in FlashFailure::ALL {
        assert!(
            class == FlashFailure::ReconnectTimedOut
                || fixtures.iter().any(|(_, _, expected)| *expected == class),
            "{class:?} has no fixture"
        );
    }
}

#[test]
fn classifier_is_case_insensitive_and_conservative_about_exit_kind() {
    assert_eq!(
        classify_espflash(EspflashExit::Code(Some(1)), "ACCESS IS DENIED"),
        FlashFailure::PortBusy
    );
    // A kill reason beats whatever the dying process printed.
    assert_eq!(
        classify_espflash(EspflashExit::Cancelled, "Access is denied"),
        FlashFailure::Cancelled
    );
}

#[test]
fn sidecar_lookup_prefers_the_file_beside_the_executable() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(find_espflash_in(dir.path()), None);
    std::fs::create_dir(
        dir.path()
            .join(kivori_desktop::firmware::espflash_file_name()),
    )
    .unwrap();
    assert_eq!(
        find_espflash_in(dir.path()),
        None,
        "a directory is not a tool"
    );
    std::fs::remove_dir(
        dir.path()
            .join(kivori_desktop::firmware::espflash_file_name()),
    )
    .unwrap();
    let tool = dir
        .path()
        .join(kivori_desktop::firmware::espflash_file_name());
    std::fs::write(&tool, b"x").unwrap();
    assert_eq!(find_espflash_in(dir.path()), Some(tool));
}

#[test]
fn update_advice_table() {
    use UpdateAdvice::{DeviceNewer, Unknown, UpToDate, UpdateAvailable};
    let table = [
        (Some("1.2.0"), Some("1.3.0"), UpdateAvailable),
        (Some("1.3.0"), Some("1.3.0"), UpToDate),
        (Some("1.3.1"), Some("1.3.0"), DeviceNewer),
        (Some("1.10.0"), Some("1.9.0"), DeviceNewer),
        (Some("0.9.9"), Some("1.0.0"), UpdateAvailable),
        (None, Some("1.3.0"), Unknown),
        (Some("1.3.0"), None, Unknown),
        (Some("1.3"), Some("1.3.0"), Unknown),
        (Some("1.3.0"), Some("garbage"), Unknown),
        (Some("1.3.0.1"), Some("1.3.0"), Unknown),
    ];
    for (device, bundled, expected) in table {
        assert_eq!(
            update_advice(device, bundled),
            expected,
            "{device:?} {bundled:?}"
        );
    }
}

#[test]
fn firmware_status_exposes_no_port_or_path() {
    let mut flash = FlashWorkflow::new(true, 512);
    flash.request(true, Some("COM7"), Some("deadbeef")).unwrap();
    for _ in 0..2 {
        let json = serde_json::to_value(flash.status()).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "advice",
                "available",
                "bundledVersion",
                "failure",
                "imageSize",
                "message",
                "phase"
            ]
        );
        let text = json.to_string();
        for forbidden in ["COM7", "deadbeef", "/dev/", "\\\\", ".elf", "espflash"] {
            assert!(!text.contains(forbidden), "{forbidden} leaked into {text}");
        }
        flash.finish(Err(FlashFailure::Unknown));
        flash.request_recovery(&[kivori_port("COM7")]).unwrap();
    }
}
