//! S5.5: fake executors exercise production policies; no flash/updater process is constructed.
use super::*;
use crate::activity::{ActivityEventKind, ActivityLog};
use crate::catalog::Catalog;
use crate::config::UpdateSettings;
use crate::device::discovery::{default_allowlist, PortCandidate};
use crate::firmware::{FirmwarePhase, FlashFailure, FlashWorkflow, ResumeTarget};
use crate::firmware_bundle::FirmwareBundle;
use crate::firmware_marker::ImageIdentity;
use crate::firmware_offer::{FirmwareOffer, OfferDevice, OfferInputs, OfferTracker};
use crate::identity::{
    check_image_for_device, decide_recovery, DeviceIdentity, IdentitySource, RecoveryDecision,
};
use crate::runtime::device_task::run_accepted_firmware_flash;
use crate::runtime::state::{AppState, DeviceCommand};
use crate::updater::policy::{
    Availability, Busy, CheckOutcome, CheckReport, Machine, Release, UpdateState,
};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};

fn check(ok: bool, message: &str) -> Result<(), String> {
    ok.then_some(()).ok_or_else(|| message.into())
}

fn until(
    sandbox: &mut Sandbox,
    name: &str,
    ready: impl Fn(&Sandbox) -> bool,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        sandbox.tick();
        if ready(sandbox) {
            return Ok(());
        }
        if let Some(error) = &sandbox.snapshot.error {
            return Err(format!("{name}: {error}"));
        }
        if Instant::now() >= deadline {
            return Err(format!("{name}: bounded wait expired"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn connected(sandbox: &mut Sandbox) -> Result<(), String> {
    until(sandbox, "real virtual handshake/capture", |s| {
        s.snapshot.connected && s.snapshot.frame.is_some()
    })?;
    sandbox.next_capture = Duration::MAX;
    Ok(())
}

fn run_fake(
    flash: &mut FlashWorkflow,
    result: Result<(), FlashFailure>,
    calls: &mut usize,
) -> (ResumeTarget, Vec<ActivityEventKind>) {
    let mut events = Vec::new();
    let resume = run_accepted_firmware_flash(
        flash,
        |status| {
            assert_eq!(status.phase, FirmwarePhase::Flashing);
            *calls += 1;
            result
        },
        |event| events.push(event.kind),
    );
    (resume, events)
}

fn flash_reconnect(executable: &Path) -> Result<(), String> {
    let mut sandbox = Sandbox::open(executable).map_err(|e| e.to_string())?;
    connected(&mut sandbox)?;
    let mut calls = 0;
    let mut flash = FlashWorkflow::new(true, 512);
    for failure in [
        FlashFailure::Cancelled,
        FlashFailure::Timeout,
        FlashFailure::PortBusy,
        FlashFailure::NoDownloadMode,
    ] {
        let hash = sandbox
            .manager
            .device()
            .ok_or("missing virtual identity")?
            .device_id_hash_short
            .clone();
        flash.request(true, Some("virtual-pipe"), Some(&hash))?;
        check(
            flash
                .request(true, Some("virtual-pipe"), Some(&hash))
                .is_err(),
            "duplicate flash admitted",
        )?;
        sandbox.connection("disconnect")?;
        let before = calls;
        let (resume, events) = run_fake(&mut flash, Err(failure), &mut calls);
        check(
            calls == before + 1
                && resume == ResumeTarget::Discovery
                && flash.status().phase == FirmwarePhase::Failed
                && flash.status().failure == Some(failure)
                && !flash.is_busy(),
            "failed flash did not release discovery ownership",
        )?;
        check(
            events.contains(&ActivityEventKind::FirmwareUpdateFailed)
                && !events.contains(&ActivityEventKind::FirmwarePostFlashVerified)
                && !flash.handshake("virtual-pipe", &hash, true),
            "failed flash fabricated verification",
        )?;
        sandbox.connection("reconnect")?;
        connected(&mut sandbox)?;
        println!("PASS S5.5 fake-flash + real virtual session: {failure:?} releases ownership and reconnects without claiming success");
    }
    let hash = sandbox
        .manager
        .device()
        .ok_or("missing virtual identity")?
        .device_id_hash_short
        .clone();
    let generation = sandbox.snapshot.generation;
    flash.request(true, Some("virtual-pipe"), Some(&hash))?;
    sandbox.connection("disconnect")?;
    let (resume, events) = run_fake(&mut flash, Ok(()), &mut calls);
    check(
        resume == ResumeTarget::SamePort("virtual-pipe".into())
            && flash.status().phase == FirmwarePhase::Reconnecting
            && events.contains(&ActivityEventKind::FirmwareFlashSucceeded)
            && !events.contains(&ActivityEventKind::FirmwarePostFlashVerified),
        "fake process success was promoted to verified recovery",
    )?;
    check(
        !flash.handshake("other-virtual-pipe", &hash, true)
            && !flash.handshake("virtual-pipe", "stale-unit", true),
        "wrong port or device verified a flash",
    )?;
    sandbox.connection("wrong-major")?;
    until(&mut sandbox, "unsupported virtual protocol", |s| {
        s.snapshot.error.is_some()
    })?;
    check(
        !flash.handshake(
            "virtual-pipe",
            &hash,
            sandbox.snapshot.connected && sandbox.session.can_sync_state(),
        ),
        "incompatible reconnect verified a flash",
    )?;
    sandbox.connection("reconnect")?;
    connected(&mut sandbox)?;
    let actual = sandbox
        .manager
        .device()
        .ok_or("missing reconnected identity")?
        .device_id_hash_short
        .clone();
    check(
        sandbox.snapshot.generation > generation
            && flash.handshake(
                "virtual-pipe",
                &actual,
                sandbox.snapshot.connected && sandbox.session.can_sync_state(),
            )
            && flash.status().phase == FirmwarePhase::Succeeded,
        "real compatible virtual handshake did not verify fake workflow",
    )?;
    check(
        flash
            .drain_activity()
            .iter()
            .filter(|e| e.kind == ActivityEventKind::FirmwarePostFlashVerified)
            .count()
            == 1
            && !flash.handshake("virtual-pipe", &actual, true),
        "post-flash verification repeated",
    )?;
    sandbox.input("tap")?;
    until(&mut sandbox, "usable input after fake flash", |s| {
        s.snapshot.media_triggers == 1
    })?;
    println!("PASS S5.5 fake-flash + real virtual session: process success waits for matching compatible handshake; fresh input works");

    let (vid, pid) = default_allowlist()
        .first()
        .copied()
        .ok_or("empty recovery allowlist")?;
    let port = PortCandidate::new("virtual-recovery-pipe", Some(vid), Some(pid));
    flash.request_recovery(std::slice::from_ref(&port))?;
    run_fake(&mut flash, Ok(()), &mut calls);
    flash.reconnect_timed_out();
    check(
        flash.status().failure == Some(FlashFailure::ReconnectTimedOut)
            && !flash.handshake("virtual-recovery-pipe", &actual, true)
            && !flash.is_busy(),
        "expired recovery accepted a late handshake",
    )?;
    flash.request_recovery(&[port])?;
    run_fake(&mut flash, Ok(()), &mut calls);
    check(
        flash.handshake("virtual-recovery-pipe", &actual, true)
            && flash.recovered_hash() == Some(actual.as_str()),
        "fresh recovery did not record verified virtual identity",
    )?;
    println!("PASS S5.5 workflow-policy: reconnect timeout rejects late success; explicitly retried recovery records identity");
    Ok(())
}

fn fixture_board(id: &str) -> String {
    format!("[boards.{id}]\ndisplay_name = '{id}'\nflash = 'espflash'\nusb = ['1234:0001']\n[boards.{id}.firmware]\ncrate = 'firmware/fixture'\nartifact = 'image'\nmin_protocol = '0.1'\n")
}

fn fixture_image(product: u16, board: u16) -> Vec<u8> {
    let mut bytes = vec![0; 52];
    bytes[..6].copy_from_slice(b"\x7fELF\x01\x01");
    bytes[18] = 243;
    bytes.extend_from_slice(
        format!(
            "KIVORI-FW-VERSION:0.1.0\0KIVORI-FW-IDENTITY:p={product};b={board};v=0.1.0;m=0.1\0"
        )
        .as_bytes(),
    );
    bytes
}

fn fake_write(
    image: &ImageIdentity,
    device: &DeviceIdentity,
    calls: &mut usize,
) -> Result<(), crate::identity::WrongProduct> {
    check_image_for_device(image, device)?;
    *calls += 1;
    Ok(())
}

fn target_selection() -> Result<(), String> {
    let product = |name: &str, boards: String| {
        format!("display_name = '{name}'\nshort_name = '{name}'\nmodel = 'KV-T1'\nrequired_capabilities = []\n{boards}")
    };
    let first = product(
        "First",
        fixture_board("board-a") + &fixture_board("board-b"),
    );
    let second = product("Second", fixture_board("board-a"));
    let catalog = Catalog::parse(
        "[products]\nfirst = 41\nsecond = 42\n[boards]\nboard-a = 9\nboard-b = 10\n",
        &[("first", &first), ("second", &second)],
    )
    .map_err(|e| format!("fixture catalog: {e:?}"))?;
    let bytes = fixture_image(41, 9);
    let bundle = FirmwareBundle::parse(&[&bytes]).map_err(|e| format!("fixture bundle: {e:?}"))?;
    let mut writes = 0;
    let mut attempt = |usb,
                       chosen,
                       board,
                       reported: Option<&DeviceIdentity>|
     -> Result<RecoveryDecision, String> {
        let decision = decide_recovery(&catalog, usb, chosen, board, reported);
        if let RecoveryDecision::Proceed { product, board } = decision {
            if let Some(image) = bundle.get(product, board) {
                let device = DeviceIdentity {
                    product,
                    board,
                    hw_rev: None,
                    source: IdentitySource::Build,
                };
                fake_write(image.identity(), &device, &mut writes)
                    .map_err(|_| "fixture image identity mismatch")?;
            }
        }
        Ok(decision)
    };
    check(
        matches!(attempt(Some((0x1234, 1)), None, None, None)?, RecoveryDecision::Choose(options) if options.len()==3),
        "shared USB lost product/board ambiguity",
    )?;
    check(
        matches!(attempt(Some((0x1234, 1)), Some("first"), None, None)?, RecoveryDecision::Choose(options) if options.len()==2),
        "product choice silently selected a board",
    )?;
    check(
        matches!(
            attempt(Some((0x1234, 1)), Some("first"), Some("board-b"), None)?,
            RecoveryDecision::Proceed {
                product: 41,
                board: 10
            }
        ),
        "exact absent image target changed",
    )?;
    check(
        matches!(
            attempt(Some((0x9999, 1)), Some("first"), Some("board-a"), None)?,
            RecoveryDecision::Refuse(_)
        ),
        "stale choice bypassed fresh USB candidates",
    )?;
    let unknown = DeviceIdentity {
        product: 99,
        board: 9,
        hw_rev: None,
        source: IdentitySource::Chip,
    };
    check(
        matches!(
            attempt(Some((0x1234, 1)), None, None, Some(&unknown))?,
            RecoveryDecision::Refuse(_)
        ),
        "unknown reported identity fell back to hints",
    )?;
    check(
        matches!(
            attempt(Some((0x1234, 1)), Some("first"), Some("board-a"), None)?,
            RecoveryDecision::Proceed {
                product: 41,
                board: 9
            }
        ),
        "fresh explicit pair rejected",
    )?;
    check(
        writes == 1,
        "ambiguous/refused/missing-image selection executed a fake write",
    )?;
    let device = DeviceIdentity {
        product: 41,
        board: 10,
        hw_rev: None,
        source: IdentitySource::Chip,
    };
    check(
        fake_write(
            bundle.get(41, 9).ok_or("fixture image missing")?.identity(),
            &device,
            &mut writes,
        )
        .is_err(),
        "wrong board image accepted",
    )?;
    let device = DeviceIdentity {
        product: 42,
        ..device
    };
    check(
        fake_write(
            bundle.get(41, 9).ok_or("fixture image missing")?.identity(),
            &device,
            &mut writes,
        )
        .is_err(),
        "wrong product image accepted",
    )?;
    check(writes == 1, "wrong product/board guard invoked fake writer")?;
    println!("PASS S5.5 selector-policy + counting fake writer: ambiguity, fresh pair confirmation, absent image and wrong identity fail closed");
    Ok(())
}

fn offer_at(tracker: &mut OfferTracker, at: u64, inputs: &OfferInputs<'_>) -> FirmwareOffer {
    tracker.evaluate(Duration::from_secs(at), inputs);
    tracker.current().offer.clone()
}

fn firmware_offers() -> Result<(), String> {
    let identity = DeviceIdentity::legacy();
    let image = ImageIdentity {
        product: identity.product,
        board: identity.board,
        version: "0.2.0".into(),
        min_protocol: (0, 1),
    };
    let connected = OfferDevice::Connected {
        hash: "virtual-unit".into(),
        firmware: "0.1.0".into(),
        identity: Some(identity),
        missing_capabilities: false,
    };
    let mut tracker = OfferTracker::new();
    let mut inputs = OfferInputs {
        device: &connected,
        generation: 1,
        image: Some(&image),
        restorable: true,
        host_major: kivori_protocol::PROTOCOL_MAJOR,
        app_update_due: false,
        flashing: false,
        gesture: false,
    };
    check(
        matches!(offer_at(&mut tracker, 0, &inputs), FirmwareOffer::None),
        "firmware offered before idle",
    )?;
    check(
        matches!(
            offer_at(&mut tracker, 5, &inputs),
            FirmwareOffer::OfferOptional { .. }
        ),
        "new exact firmware not offered",
    )?;
    tracker.later().map_err(|e| format!("Later: {e:?}"))?;
    check(
        matches!(offer_at(&mut tracker, 6, &inputs), FirmwareOffer::None),
        "Later did not suppress current connection",
    )?;
    inputs.generation += 1;
    tracker.evaluate(Duration::from_secs(7), &inputs);
    check(
        matches!(
            offer_at(&mut tracker, 12, &inputs),
            FirmwareOffer::OfferOptional { .. }
        ),
        "Later leaked across reconnect",
    )?;
    inputs.app_update_due = true;
    check(
        matches!(
            offer_at(&mut tracker, 13, &inputs),
            FirmwareOffer::DeferredForAppUpdate
        ),
        "app update did not take priority",
    )?;
    let newer = OfferDevice::Connected {
        hash: "virtual-unit".into(),
        firmware: "0.3.0".into(),
        identity: Some(identity),
        missing_capabilities: false,
    };
    inputs.device = &newer;
    inputs.app_update_due = false;
    tracker.evaluate(Duration::from_secs(14), &inputs);
    check(
        matches!(
            offer_at(&mut tracker, 19, &inputs),
            FirmwareOffer::UpdateApp
        ),
        "newer device offered downgrade",
    )?;
    println!("PASS S5.5 offer-policy: idle, Later/reconnect, app-first priority and newer-device refusal use injected policy time");
    Ok(())
}

fn app_state() -> (AppState, mpsc::Receiver<DeviceCommand>) {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stopped = cancel.clone();
    let thread = std::thread::spawn(move || {
        while !stopped.load(Ordering::SeqCst) {
            std::thread::park_timeout(Duration::from_millis(1));
        }
    });
    let mut status = crate::ipc::dto::initial_status();
    status.connection = "connected".into();
    let state = AppState::new(
        false,
        Arc::new(Mutex::new(status)),
        Arc::new(ActivityLog::new(8)),
        tx,
        cancel,
        thread,
    );
    state
        .firmware_status
        .lock()
        .expect("firmware status")
        .available = true;
    (state, rx)
}

// Fixture glue gates fake callbacks through production policy/admission, never updater I/O.
fn fake_download(machine: &mut Machine, calls: &mut usize, result: Result<(), ()>) -> bool {
    if machine.begin_install().is_err() {
        return false;
    }
    *calls += 1;
    machine.progress(32, Some(64));
    if result.is_ok() {
        machine.downloaded();
    } else {
        machine.install_failed();
    }
    true
}

fn fake_install(
    state: &AppState,
    machine: &mut Machine,
    run: impl FnOnce() -> Result<(), ()>,
) -> bool {
    if !matches!(machine.state(), UpdateState::ReadyToRestart(_)) {
        return false;
    }
    let Some(_reservation) = state.try_reserve_app_install() else {
        return false;
    };
    if run().is_err() {
        machine.install_failed();
    }
    true
}

struct AdmissionFixture(AppState);
impl Drop for AdmissionFixture {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

fn app_updates_and_exclusion() -> Result<(), String> {
    let idle = Busy {
        flash: false,
        gesture: false,
    };
    let version = |s: &str| semver::Version::parse(s).expect("fixed scenario version");
    let release = Release::new(version("0.5.0"), None, None);
    let mut machine = Machine::new(Availability::Enabled);
    let checks = Cell::new(0);
    let fake_check = |outcome| {
        checks.set(checks.get() + 1);
        outcome
    };
    let mut downloads = 0;
    let installs = Cell::new(0);
    let (state, rx) = app_state();
    let fixture = AdmissionFixture(state);
    let state = &fixture.0;
    check(
        !fake_install(state, &mut machine, || {
            installs.set(installs.get() + 1);
            Ok(())
        }) && installs.get() == 0,
        "installer callback ran without an offer/download",
    )?;
    check(machine.begin_check(), "fake update check refused")?;
    check(
        machine.check_finished(
            fake_check(CheckOutcome::Failed),
            &version("0.4.0"),
            &UpdateSettings::default(),
            100,
            false,
            idle,
        ) == CheckReport::Failed
            && machine.last_checked().is_none(),
        "failed check advanced successful check time",
    )?;
    check(machine.begin_check(), "failed fake check did not retry")?;
    check(
        machine.check_finished(
            fake_check(CheckOutcome::Found(release.clone())),
            &version("0.4.0"),
            &UpdateSettings::default(),
            101,
            false,
            Busy {
                flash: true,
                gesture: false,
            },
        ) == CheckReport::Held
            && machine.holding(),
        "busy device did not hold updater offer",
    )?;
    check(
        !fake_download(&mut machine, &mut downloads, Ok(())) && downloads == 0,
        "held offer invoked fake download",
    )?;
    check(
        machine.settle(idle)
            && fake_download(&mut machine, &mut downloads, Err(()))
            && downloads == 1
            && matches!(machine.state(), UpdateState::Failed(_)),
        "fake download failure did not preserve retry",
    )?;
    check(
        fake_download(&mut machine, &mut downloads, Ok(()))
            && downloads == 2
            && matches!(machine.state(), UpdateState::ReadyToRestart(_))
            && checks.get() == 2,
        "fake download retry did not become ready",
    )?;
    let mut excluded = false;
    check(
        fake_install(state, &mut machine, || {
            installs.set(installs.get() + 1);
            excluded = state.queue_firmware_flash().is_err()
                && state.queue_firmware_restore().is_err()
                && state
                    .queue_firmware_restore_for(
                        "fixture-product".into(),
                        Some("fixture-board".into()),
                    )
                    .is_err()
                && rx.try_recv().is_err();
            Err(())
        }) && excluded
            && installs.get() == 1
            && matches!(machine.state(), UpdateState::Failed(_)),
        "fake installer failed to exclude firmware or report failure",
    )?;
    check(
        fake_download(&mut machine, &mut downloads, Ok(())),
        "installer failure did not allow download retry",
    )?;
    state.queue_firmware_restore().map_err(|e| e.to_string())?;
    check(
        matches!(rx.try_recv(), Ok(DeviceCommand::RestoreFirmware))
            && state.firmware_status_snapshot().phase == FirmwarePhase::Preparing,
        "failed installer retained firmware reservation",
    )?;
    for phase in [
        FirmwarePhase::Preparing,
        FirmwarePhase::Flashing,
        FirmwarePhase::Reconnecting,
    ] {
        state.firmware_status.lock().expect("firmware status").phase = phase;
        check(
            !fake_install(state, &mut machine, || {
                installs.set(installs.get() + 1);
                Ok(())
            }) && installs.get() == 1,
            "busy firmware invoked fake installer",
        )?;
    }
    state.firmware_status.lock().expect("firmware status").phase = FirmwarePhase::Failed;
    check(
        fake_install(state, &mut machine, || {
            installs.set(installs.get() + 1);
            Err(())
        }) && installs.get() == 2,
        "failed firmware retained installer ownership",
    )?;
    drop(fixture);
    let (state, rx) = app_state();
    let fixture = AdmissionFixture(state);
    let state = &fixture.0;
    drop(rx);
    check(
        state.queue_firmware_restore().is_err()
            && state.firmware_status_snapshot().phase == FirmwarePhase::Idle
            && state.try_reserve_app_install().is_some(),
        "failed command enqueue did not roll back firmware reservation",
    )?;
    println!("PASS S5.5 updater-policy + counting fake callbacks: failed check/download/install retry, held offer and mutual flash/install admission");
    Ok(())
}

/// Headless rehearsal only; never registers the updater or calls a physical flash/install backend.
pub fn recovery_smoke(executable: &Path) -> Result<(), String> {
    target_selection()?;
    firmware_offers()?;
    app_updates_and_exclusion()?;
    flash_reconnect(executable)
}
