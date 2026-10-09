//! Per-unit QA and provisioning helper (M3 S7). DEVELOPMENT ONLY: it flashes the bundled firmware
//! onto the ONE Kivori plugged in, checks the link and the controls, prints PASS or FAIL and appends
//! a row to `provisioning-log.csv` (short device hash and versions only).
//!
//! Run it with `just provision` (which builds the firmware and bundles it). The port name is shown
//! on the terminal for the operator but is never written to the log.
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kivori_desktop::device::{
    discovery::{filter_candidates, PortCandidate, DEFAULT_ALLOWLIST},
    fsm::{ConnectionManager, ManagerEvent},
    serial::{enumerate, SerialPortLink},
    session::{Session, SessionConfig},
};
use kivori_desktop::firmware::{
    bundled_image_available, flash_bundled, BUNDLED_FIRMWARE, RECONNECT_TIMEOUT,
};
use kivori_desktop::firmware_marker::scan_firmware_version;
use kivori_desktop::orchestrator::Orchestrator;
use kivori_desktop::provision::{
    csv_row, evaluate, format_utc, InputCheck, InputProgress, Observations, BANNER, CSV_HEADER,
    HEARTBEAT_HOLD_SECS,
};
use kivori_model::Capabilities;

const LOG_FILE: &str = "provisioning-log.csv";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const INPUT_TIMEOUT: Duration = Duration::from_secs(20);

fn main() {
    println!("{BANNER}");
    if let Some(arg) = std::env::args().nth(1) {
        // The app's espflash invocation is `flash` only; it has no whole-chip erase option, so a
        // first-time `--erase` is not offered. Say so instead of silently ignoring the flag.
        let note = if arg == "--erase" {
            "--erase is not supported: the bundled flashing path has no erase option."
        } else {
            "this tool takes no arguments."
        };
        eprintln!("{note}\nusage: cargo run -p kivori-desktop --example provision");
        std::process::exit(2);
    }
    let mut obs = Observations {
        flash_failure: None,
        handshake: false,
        negotiated_caps: Capabilities::NONE,
        device_version: None,
        bundled_version: scan_firmware_version(BUNDLED_FIRMWARE),
        heartbeat_ok: false,
        state_report_seen: false,
        health_seen: false,
        inputs_done: Vec::new(),
    };
    let mut hash = None;
    let outcome = run(&mut obs, &mut hash);
    if let Err(message) = outcome {
        println!("Stopped: {message}");
    }
    let verdict = evaluate(&obs);
    println!();
    println!("{}", verdict.label());
    if !verdict.passed() {
        println!("  failed checks: {}", verdict.failed_tokens());
    }
    println!("  device hash:   {}", hash.as_deref().unwrap_or("unknown"));
    println!(
        "  firmware:      unit {} / bundled {}",
        obs.device_version.as_deref().unwrap_or("unknown"),
        obs.bundled_version.as_deref().unwrap_or("unknown")
    );
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let row = csv_row(&format_utc(now), hash.as_deref(), &obs, &verdict);
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
fn single_port() -> Result<PortCandidate, String> {
    let ports = enumerate();
    let found = filter_candidates(&ports, DEFAULT_ALLOWLIST);
    match found.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err("no Kivori unit found: plug in one unit and retry.".into()),
        _ => {
            Err("more than one Kivori unit is plugged in: leave only the one to provision.".into())
        }
    }
}

fn run(obs: &mut Observations, hash: &mut Option<String>) -> Result<(), String> {
    if !bundled_image_available() {
        obs.flash_failure = Some("image_unavailable");
        return Err("no firmware is bundled (run through `just provision`).".into());
    }
    let port = single_port().inspect_err(|_| obs.flash_failure = Some("port"))?;
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
        match single_port() {
            Ok(port) => break port,
            Err(message) if Instant::now() >= deadline => return Err(message),
            Err(_) => {}
        }
    };
    let mut link = SerialPortLink::open(&port.port_name).map_err(|e| format!("open: {e}"))?;
    let mut session = Session::new(SessionConfig::default());
    let mut manager = ConnectionManager::new();
    let mut orchestrator = Orchestrator::new();
    session
        .open(&mut link, &mut manager)
        .map_err(|e| format!("open session: {e:?}"))?;

    let start = Instant::now();
    let mut retry_at = Duration::from_secs(4);
    while !manager.state().can_drive_device() {
        if start.elapsed() >= HANDSHAKE_TIMEOUT {
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
    obs.handshake = true;
    obs.negotiated_caps = session.negotiated_caps();
    if let Some(device) = manager.device() {
        let v = device.firmware_version;
        obs.device_version = Some(format!("{}.{}.{}", v.major, v.minor, v.patch));
        *hash = Some(device.device_id_hash_short.clone());
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
        "\nNow the controls. Do each one when asked ({} s each).",
        INPUT_TIMEOUT.as_secs()
    );
    let _ = session.take_input_events();
    let ping_clock = Instant::now();
    let mut next_ping = Duration::ZERO;
    for check in InputCheck::ALL {
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
