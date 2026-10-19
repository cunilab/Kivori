//! Per-unit QA and provisioning helper (M3 S7). DEVELOPMENT ONLY: it flashes the bundled firmware
//! onto the ONE Kivori plugged in, checks the link and the controls, prints PASS or FAIL and appends
//! a row to `provisioning-log.csv` (short device hash and versions only).
//!
//! Run it with `just provision` (which builds the firmware and bundles it). The port name is shown
//! on the terminal for the operator but is never written to the log.
//!
//! With `--product/--board/--rev` it previews the chip identity record (QA still flashes); with
//! `--burn-identity` it writes it into the one-time eFuses through `espefuse`, after checking the
//! block is blank and the operator typed the product id. All decisions are in `provision.rs`.
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kivori_desktop::catalog::{Board, Catalog};
use kivori_desktop::device::{
    discovery::{filter_candidates, PortCandidate},
    fsm::{ConnectionManager, ManagerEvent},
    serial::{enumerate, SerialPortLink},
    session::{Session, SessionConfig},
};
use kivori_desktop::firmware::{
    bundled_image_identity, flash_bundled, BUNDLED_FIRMWARE, RECONNECT_TIMEOUT,
};
use kivori_desktop::firmware_marker::scan_firmware_version;
use kivori_desktop::identity::DeviceIdentity;
use kivori_desktop::orchestrator::Orchestrator;
use kivori_desktop::provision::{
    block_file, check_block, check_image_matches_plan, confirmation_matches, csv_row,
    describe_record, espefuse_version_accepted, evaluate, format_utc, guard_before_flash,
    parse_args, parse_espefuse_version, require_burn_backend, resolve_plan, verify_identity,
    BlockCheck, IdentityLog, IdentityPlan, InputProgress, Observations, QaPolicy, BANNER,
    CSV_HEADER, ESPEFUSE_MAJOR, HEARTBEAT_HOLD_SECS, USAGE,
};
use kivori_model::Capabilities;

const LOG_FILE: &str = "provisioning-log.csv";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const INPUT_TIMEOUT: Duration = Duration::from_secs(20);

fn main() {
    println!("{BANNER}");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let plan = match parse_args(&args).and_then(|a| {
        let plan = resolve_plan(Catalog::embedded(), &a)?;
        let policy = QaPolicy::for_image(Catalog::embedded(), bundled_image_identity())?;
        if let Some(plan) = &plan {
            check_image_matches_plan(Some(policy.image), plan)?;
        }
        Ok((plan, a.burn, policy))
    }) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let (plan, burn, policy) = plan;
    println!(
        "QA target: {} ({}) on {} ({}).",
        policy.product.display_name, policy.product.id, policy.board.display_name, policy.board.id
    );
    if let Some(plan) = &plan {
        println!("Identity record:\n{}", describe_record(plan));
        if !burn {
            println!("IDENTITY PREVIEW: no eFuses will be written. Firmware will still be flashed and checked.");
        }
    }
    let mut obs = Observations {
        flash_failure: None,
        handshake: false,
        identity: None,
        negotiated_caps: Capabilities::NONE,
        device_version: None,
        bundled_version: scan_firmware_version(BUNDLED_FIRMWARE),
        heartbeat_ok: false,
        state_report_seen: false,
        health_seen: false,
        inputs_done: Vec::new(),
        identity_check: None,
    };
    let mut hash = None;
    let outcome = run(&mut obs, &mut hash, &policy, plan.as_ref().filter(|_| burn));
    if let Err(message) = outcome {
        println!("Stopped: {message}");
    }
    let verdict = evaluate(&obs, &policy);
    println!();
    println!("{}", verdict.label());
    if !verdict.passed() {
        println!("  failed checks: {}", verdict.failed_tokens());
    }
    println!("  device hash:   {}", hash.as_deref().unwrap_or("unknown"));
    if let Some(passed) = obs.identity_check {
        println!(
            "  identity:      {}",
            if passed {
                "PASS (reported from the chip)"
            } else {
                "FAIL"
            }
        );
    }
    println!(
        "  firmware:      unit {} / bundled {}",
        obs.device_version.as_deref().unwrap_or("unknown"),
        obs.bundled_version.as_deref().unwrap_or("unknown")
    );
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let row = csv_row(
        &format_utc(now),
        hash.as_deref(),
        &obs,
        &verdict,
        &IdentityLog::from_device(obs.identity.as_ref()),
    );
    match append_log(&row) {
        Ok(()) => println!("  logged to {LOG_FILE}"),
        Err(e) => println!("  could not write {LOG_FILE}: {e}"),
    }
    std::process::exit(i32::from(!verdict.passed()));
}

fn append_log(row: &str) -> std::io::Result<()> {
    let is_new = std::fs::metadata(LOG_FILE).map_or(true, |m| m.len() == 0);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)?;
    if is_new {
        writeln!(file, "{CSV_HEADER}")?;
    }
    writeln!(file, "{row}")
}

/// Exactly one allowlisted port, or a message for the operator.
fn single_port(board: &Board) -> Result<PortCandidate, String> {
    let ports = enumerate();
    let found = filter_candidates(&ports, &board.usb);
    match found.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err("no Kivori unit found: plug in one unit and retry.".into()),
        _ => {
            Err("more than one Kivori unit is plugged in: leave only the one to provision.".into())
        }
    }
}

/// A connected session on one port.
struct Conn {
    link: SerialPortLink,
    session: Session,
    manager: ConnectionManager,
    orchestrator: Orchestrator,
}

/// Opens the port and completes the handshake within `timeout`.
fn connect(port: &str, timeout: Duration) -> Result<Conn, String> {
    let mut link = SerialPortLink::open(port).map_err(|e| format!("open: {e}"))?;
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    session
        .open(&mut link, &mut manager)
        .map_err(|e| format!("open session: {e:?}"))?;

    let start = Instant::now();
    let mut retry_at = Duration::from_secs(4);
    while !manager.state().can_drive_device() {
        if start.elapsed() >= timeout {
            return Err("the unit did not complete the handshake.".into());
        }
        session
            .pump(&mut link, &mut manager, &mut orchestrator)
            .map_err(|e| format!("pump: {e:?}"))?;
        if start.elapsed() >= retry_at && !manager.state().can_drive_device() {
            manager.apply(ManagerEvent::HandshakeTimeout);
            manager.apply(ManagerEvent::BackoffElapsed);
            session
                .open(&mut link, &mut manager)
                .map_err(|e| format!("retry: {e:?}"))?;
            retry_at = start.elapsed() + Duration::from_secs(4);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(Conn {
        link,
        session,
        manager,
        orchestrator,
    })
}

/// Pumps until the device's identity is known (a device that negotiated `PRODUCT_IDENTITY_V1`
/// sends it right after the handshake), or `wait` passes.
fn await_identity(conn: &mut Conn, wait: Duration) -> Result<Option<DeviceIdentity>, String> {
    let start = Instant::now();
    while conn.session.identity().is_none() && start.elapsed() < wait {
        conn.session
            .pump(&mut conn.link, &mut conn.manager, &mut conn.orchestrator)
            .map_err(|e| format!("pump: {e:?}"))?;
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(conn.session.identity())
}

/// Before flashing: what the unit currently running is, so the image can be checked against it.
/// `None` when it does not handshake (blank or recovering). The port is released on return.
fn probe_identity(port: &str) -> Result<Option<DeviceIdentity>, String> {
    let Ok(mut conn) = connect(port, Duration::from_secs(8)) else {
        println!(
            "The unit did not answer a handshake (blank or recovering): no identity to check."
        );
        return Ok(None);
    };
    match await_identity(&mut conn, Duration::from_secs(3))? {
        Some(identity) => Ok(Some(identity)),
        None => Err("the unit connected but did not report its identity.".into()),
    }
}

fn run(
    obs: &mut Observations,
    hash: &mut Option<String>,
    policy: &QaPolicy<'_>,
    burn: Option<&IdentityPlan>,
) -> Result<(), String> {
    let port = single_port(policy.board).inspect_err(|_| obs.flash_failure = Some("port"))?;
    // The same image-vs-device guard as the app's flashing path, before anything is written.
    let current =
        probe_identity(&port.port_name).inspect_err(|_| obs.flash_failure = Some("port"))?;
    if let Err(wrong) = guard_before_flash(policy.image, current.as_ref()) {
        obs.flash_failure = Some("wrongProduct");
        return Err(format!(
            "this unit is {} but the bundled firmware is for {}: refusing to flash.",
            wrong.device, wrong.image
        ));
    }
    if let Some(plan) = burn {
        burn_identity(&port.port_name, plan)
            .inspect_err(|_| obs.flash_failure = Some("identity_burn"))?;
    }
    println!(
        "Found one unit on {}. Flashing the bundled firmware...",
        port.port_name
    );
    let cancel = AtomicBool::new(false);
    if let Err(failure) = flash_bundled(&port.port_name, &cancel) {
        obs.flash_failure = Some(failure.token());
        return Err(failure.message().into());
    }
    println!("Flashed. Waiting for the unit to restart...");
    // The unit re-enumerates after the reset; it may come back under another name.
    let deadline = Instant::now() + RECONNECT_TIMEOUT;
    let port = loop {
        std::thread::sleep(Duration::from_millis(500));
        match single_port(policy.board) {
            Ok(port) => break port,
            Err(message) if Instant::now() >= deadline => return Err(message),
            Err(_) => {}
        }
    };
    let mut conn = connect(&port.port_name, HANDSHAKE_TIMEOUT)?;
    obs.handshake = true;
    obs.identity = await_identity(&mut conn, Duration::from_secs(5))?;
    let Conn {
        mut link,
        mut session,
        mut manager,
        mut orchestrator,
    } = conn;
    if let Some(plan) = burn {
        let ok = verify_identity(plan, obs.identity.as_ref());
        obs.identity_check = Some(ok);
        println!(
            "Identity after the burn: {}.",
            if ok {
                "PASS"
            } else {
                "FAIL (the unit does not report the burned record from the chip)"
            }
        );
    }
    obs.negotiated_caps = session.negotiated_caps();
    if let Some(device) = manager.device() {
        let v = device.firmware_version;
        obs.device_version = Some(format!("{}.{}.{}", v.major, v.minor, v.patch));
        *hash = Some(device.device_id_hash_short.clone());
    }
    if !policy.matches_identity(obs.identity.as_ref()) {
        return Err("the post-flash identity is absent, legacy or differs from the selected image; refusing QA.".into());
    }
    println!(
        "Connected (device {}).",
        hash.as_deref().unwrap_or("unknown")
    );

    println!("Holding the heartbeat for {HEARTBEAT_HOLD_SECS} s...");
    let hold = Instant::now();
    let mut next_ping = Duration::ZERO;
    while hold.elapsed() < Duration::from_secs(HEARTBEAT_HOLD_SECS) {
        tick(
            &mut session,
            &mut link,
            &mut manager,
            &mut orchestrator,
            &hold,
            &mut next_ping,
        )?;
    }
    obs.heartbeat_ok = !session.heartbeat_timed_out() && manager.state().can_drive_device();
    obs.state_report_seen = session.reported().is_some();
    obs.health_seen = session.diagnostics().free_bytes.is_some();

    println!(
        "\nCatalog requires {} input checks ({} s each).",
        policy.inputs.len(),
        INPUT_TIMEOUT.as_secs()
    );
    let _ = session.take_input_events();
    let ping_clock = Instant::now();
    let mut next_ping = Duration::ZERO;
    for &check in &policy.inputs {
        if !check.supported_by(obs.negotiated_caps) {
            println!("  -> {}: required capability missing; FAIL.", check.token());
            continue;
        }
        println!("  -> {}", check.prompt());
        let mut progress = InputProgress::new(check);
        let step = Instant::now();
        let mut done = false;
        while !done && step.elapsed() < INPUT_TIMEOUT {
            tick(
                &mut session,
                &mut link,
                &mut manager,
                &mut orchestrator,
                &ping_clock,
                &mut next_ping,
            )?;
            for event in session.take_input_events() {
                done |= progress.observe(event.control, event.kind);
            }
        }
        if done {
            println!("     ok");
            obs.inputs_done.push(check);
        } else {
            println!("     not seen");
        }
    }
    Ok(())
}

/// The espefuse executable on `PATH` (4.x installs it as `espefuse.py`, newer ones as `espefuse`).
fn find_espefuse() -> Option<&'static str> {
    ["espefuse", "espefuse.py"].into_iter().find(|name| {
        Command::new(name).arg("--help").output().is_ok_and(|out| {
            parse_espefuse_version(&String::from_utf8_lossy(&out.stdout)).is_some()
        })
    })
}

/// A fresh scratch directory for the dump and the block file.
fn scratch_dir() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("kivori-provision-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&dir).map_err(|e| format!("scratch directory: {e}"))?;
    Ok(dir)
}

/// Reads `BLOCK_USR_DATA` (read only: `espefuse dump`).
fn read_user_block(tool: &str, port: &str, dir: &Path) -> Result<Vec<u8>, String> {
    let out = Command::new(tool)
        .args(["--chip", "esp32c3", "--port", port, "dump", "--file_name"])
        .arg(dir.join("blk.bin"))
        .output()
        .map_err(|e| format!("could not run {tool}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{tool} dump failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // espefuse names the files blk0.bin, blk1.bin, ...; block 3 is BLOCK_USR_DATA.
    std::fs::read(dir.join("blk3.bin")).map_err(|e| format!("could not read the eFuse dump: {e}"))
}

/// Writes the record: the ONLY place anything is burned, reached only after the typed confirmation.
fn burn_block(tool: &str, port: &str, file: &Path) -> Result<(), String> {
    let status = Command::new(tool)
        .args(["--chip", "esp32c3", "--port", port, "--do-not-confirm"])
        .args(["burn_block_data", "BLOCK_USR_DATA"])
        .arg(file)
        .status()
        .map_err(|e| format!("could not run {tool}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{tool} burn_block_data failed ({status}); check the chip before retrying."
        ))
    }
}

/// The checks, in order, then the burn: image matches the record, espefuse version, block blank,
/// typed confirmation. The port is free here (no session is open).
fn burn_identity(port: &str, plan: &IdentityPlan) -> Result<(), String> {
    require_burn_backend(&plan.board_id)?;
    check_image_matches_plan(bundled_image_identity(), plan)?;
    let tool = find_espefuse().ok_or(
        "espefuse not found: install it with `pip install esptool==4.7.0` (docs/provisioning.md).",
    )?;
    let output = Command::new(tool)
        .arg("--help")
        .output()
        .map_err(|e| e.to_string())?;
    let version = parse_espefuse_version(&String::from_utf8_lossy(&output.stdout))
        .ok_or("could not read the espefuse version.")?;
    if !espefuse_version_accepted(version) {
        return Err(format!(
            "espefuse {}.{}.{} is not supported: this tool needs major {ESPEFUSE_MAJOR} (esptool==4.7.0).",
            version.0, version.1, version.2
        ));
    }
    let dir = scratch_dir()?;
    let result = burn_checked(tool, port, plan, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn burn_checked(tool: &str, port: &str, plan: &IdentityPlan, dir: &Path) -> Result<(), String> {
    println!("Reading the chip's user data block (read only)...");
    let block = read_user_block(tool, port, dir)?;
    match check_block(&block, plan) {
        BlockCheck::AlreadyProvisioned => {
            println!("Already provisioned with this identity: nothing to burn.");
            return Ok(());
        }
        BlockCheck::Refuse(reason) => return Err(reason),
        BlockCheck::Blank => {}
    }
    println!("\nABOUT TO BURN, PERMANENTLY, into the ESP32-C3 on {port}:");
    println!("{}", describe_record(plan));
    println!("eFuses are one-time programmable: this can never be undone or changed.");
    print!(
        "Type the product id `{}` to burn, anything else aborts: ",
        plan.product_id
    );
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("could not read the confirmation: {e}"))?;
    if !confirmation_matches(&line, plan) {
        return Err("not confirmed: nothing was burned.".into());
    }
    let file = dir.join("identity.bin");
    std::fs::write(&file, block_file(plan)).map_err(|e| format!("block file: {e}"))?;
    burn_block(tool, port, &file)?;
    println!("Burned. The unit restarts into the new firmware next; reconnecting to verify...");
    Ok(())
}

/// One pump plus a ping once a second.
fn tick(
    session: &mut Session,
    link: &mut SerialPortLink,
    manager: &mut ConnectionManager,
    orchestrator: &mut Orchestrator,
    clock: &Instant,
    next_ping: &mut Duration,
) -> Result<(), String> {
    session
        .pump(link, manager, orchestrator)
        .map_err(|e| format!("pump: {e:?}"))?;
    let now = clock.elapsed();
    if manager.state().can_drive_device() && now >= *next_ping {
        session
            .send_ping(link, now.as_millis() as u32)
            .map_err(|e| format!("ping: {e:?}"))?;
        *next_ping = now + Duration::from_secs(1);
    }
    std::thread::sleep(Duration::from_millis(20));
    Ok(())
}
