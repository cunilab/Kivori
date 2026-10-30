//! Host presence (lock, sleep, console switch) through the production tracker and actions, the real
//! desktop session and the actual firmware Runtime. Every deadline uses real elapsed time; nothing
//! here observes OS notification delivery, USB power or physical timing (validation Phase 7).
use super::*;

fn check(ok: bool, message: &str) -> Result<(), String> {
    ok.then_some(()).ok_or_else(|| message.to_string())
}

/// Ticks until `predicate` holds, failing on any sandbox error or after `limit`.
fn until(
    sandbox: &mut Sandbox,
    limit: Duration,
    what: &str,
    predicate: impl Fn(&Sandbox) -> bool,
) -> Result<Duration, String> {
    let started = Instant::now();
    loop {
        sandbox.tick();
        if let Some(error) = &sandbox.snapshot.error {
            return Err(format!("{what}: {error}"));
        }
        if predicate(sandbox) {
            return Ok(started.elapsed());
        }
        if started.elapsed() >= limit {
            return Err(format!(
                "{what}: timed out (device {:?}, connected {})",
                sandbox.snapshot.device_state, sandbox.snapshot.connected
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn settle(sandbox: &mut Sandbox, duration: Duration) -> Result<(), String> {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        sandbox.tick();
        if let Some(error) = &sandbox.snapshot.error {
            return Err(error.clone());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn device(sandbox: &Sandbox, state: &str) -> bool {
    sandbox.snapshot.device_state == Some(state)
}

/// Connected, idle on the panel, with a frame.
fn ready(sandbox: &mut Sandbox, what: &str) -> Result<Duration, String> {
    until(sandbox, Duration::from_secs(5), what, |s| {
        s.snapshot.connected && device(s, "idle") && s.snapshot.frame.is_some()
    })
}

fn lock_and_unlock(executable: &Path) -> Result<(), String> {
    let mut sandbox = Sandbox::open(executable).map_err(|error| error.to_string())?;
    ready(&mut sandbox, "initial connection")?;
    let generation = sandbox.snapshot.generation;
    sandbox.host_event(HostEvent::Locked)?;
    let slept = until(
        &mut sandbox,
        Duration::from_secs(1),
        "lock shows Sleeping",
        |s| device(s, "sleeping"),
    )?;
    check(
        sandbox.snapshot.connected && sandbox.snapshot.presence == "locked",
        "lock dropped the link or did not report locked",
    )?;
    // 7.1: the volume knob still works while locked.
    sandbox.input("clockwise")?;
    until(
        &mut sandbox,
        Duration::from_secs(2),
        "detent while locked",
        |s| s.snapshot.volume == 52,
    )?;
    sandbox.host_event(HostEvent::Unlocked)?;
    let woke = until(
        &mut sandbox,
        Duration::from_secs(1),
        "unlock restores idle",
        |s| device(s, "idle"),
    )?;
    check(
        sandbox.snapshot.connected
            && sandbox.snapshot.generation == generation
            && sandbox.snapshot.presence == "active",
        "unlock reconnected or kept a host override",
    )?;
    // A repeated unlock changes nothing.
    sandbox.host_event(HostEvent::Unlocked)?;
    settle(&mut sandbox, Duration::from_millis(300))?;
    check(
        device(&sandbox, "idle") && sandbox.snapshot.generation == generation,
        "duplicate unlock acted",
    )?;
    println!(
        "PASS PRESENCE lock/unlock: Sleeping in {} ms on the same link, knob still sets volume, idle in {} ms after unlock",
        slept.as_millis(),
        woke.as_millis()
    );
    Ok(())
}

fn sleep_and_wake(executable: &Path) -> Result<(), String> {
    let mut sandbox = Sandbox::open(executable).map_err(|error| error.to_string())?;
    ready(&mut sandbox, "initial connection")?;
    let generation = sandbox.snapshot.generation;
    let nonce = sandbox.session.current_session();
    sandbox.host_event(HostEvent::Suspending)?;
    check(
        sandbox.link.is_none() && sandbox.parked.is_some() && !sandbox.snapshot.connected,
        "suspend did not release the link while keeping the device powered",
    )?;
    let slept = until(
        &mut sandbox,
        Duration::from_secs(1),
        "goodbye shows Sleeping",
        |s| device(s, "sleeping"),
    )?;
    // 7.3: Sleeping, not Offline, well past the firmware's ~4 s host-silence limit.
    settle(&mut sandbox, Duration::from_secs(6))?;
    check(
        device(&sandbox, "sleeping"),
        "released device fell to Offline despite the sleep goodbye",
    )?;
    // 7.4: a turn and a press while the host sleeps must never run.
    sandbox.input("clockwise")?;
    sandbox.input("tap")?;
    settle(&mut sandbox, Duration::from_millis(400))?;
    check(
        sandbox.edges.is_empty(),
        "sleep-time input had not reached the firmware before wake",
    )?;
    sandbox.host_event(HostEvent::Resumed)?;
    let resumed = until(
        &mut sandbox,
        Duration::from_secs(5),
        "wake reconnects",
        |s| s.snapshot.connected && device(s, "idle"),
    )?;
    check(
        sandbox.snapshot.generation > generation && sandbox.session.current_session() != nonce,
        "wake did not start a fresh session",
    )?;
    settle(&mut sandbox, Duration::from_millis(1500))?;
    check(
        sandbox.snapshot.volume == 50 && sandbox.snapshot.media_triggers == 0,
        "input made during sleep ran after wake",
    )?;
    sandbox.input("tap")?;
    until(
        &mut sandbox,
        Duration::from_secs(2),
        "fresh press after wake",
        |s| s.snapshot.media_triggers == 1,
    )?;
    println!(
        "PASS PRESENCE sleep/wake: goodbye shows Sleeping in {} ms and holds past host silence; wake reconnects in {} ms with a fresh session; sleep-time input never runs; fresh press works",
        slept.as_millis(),
        resumed.as_millis()
    );
    Ok(())
}

fn console_switch(executable: &Path) -> Result<(), String> {
    let mut sandbox = Sandbox::open(executable).map_err(|error| error.to_string())?;
    ready(&mut sandbox, "initial connection")?;
    sandbox.host_event(HostEvent::ConsoleDisconnected)?;
    check(
        sandbox.parked.is_some() && !sandbox.snapshot.connected,
        "console loss kept the port",
    )?;
    // No goodbye is sent, so the device must fall back to its own Offline screen, never freeze.
    let offline = until(
        &mut sandbox,
        Duration::from_secs(8),
        "silent release shows Offline",
        |s| device(s, "offline"),
    )?;
    // The machine also sleeps and wakes while another user has the console: the second release and
    // the wake must neither drop the still-powered device nor reclaim the port.
    let pid = sandbox.parked.as_ref().map(|link| link.child.id());
    sandbox.host_event(HostEvent::Suspending)?;
    sandbox.host_event(HostEvent::Resumed)?;
    check(
        sandbox.link.is_none() && sandbox.parked.as_ref().map(|link| link.child.id()) == pid,
        "sleep during a console switch dropped the device or reclaimed the port",
    )?;
    sandbox.host_event(HostEvent::ConsoleConnected)?;
    let back = ready(&mut sandbox, "console return reconnects")?;
    check(
        sandbox.link.as_ref().map(|link| link.child.id()) == pid,
        "console return booted a new device instead of reattaching",
    )?;
    println!(
        "PASS PRESENCE console switch: no goodbye -> device Offline after {} ms of host silence; sleep meanwhile keeps the device; console return reattaches it in {} ms",
        offline.as_millis(),
        back.as_millis()
    );
    Ok(())
}

fn power_cut_during_sleep(executable: &Path) -> Result<(), String> {
    let mut sandbox = Sandbox::open(executable).map_err(|error| error.to_string())?;
    ready(&mut sandbox, "initial connection")?;
    sandbox.input("clockwise")?;
    until(
        &mut sandbox,
        Duration::from_secs(2),
        "detent before sleep",
        |s| s.snapshot.volume == 52,
    )?;
    sandbox.host_event(HostEvent::Suspending)?;
    // 7.7: the machine cuts USB power while asleep; the old device process is gone.
    sandbox.parked = None;
    sandbox.host_event(HostEvent::Resumed)?;
    let back = ready(&mut sandbox, "fresh boot reconnects")?;
    sandbox.input("clockwise")?;
    until(
        &mut sandbox,
        Duration::from_secs(2),
        "detent after reboot",
        |s| s.snapshot.volume == 54,
    )?;
    println!(
        "PASS PRESENCE USB power cut during sleep: a freshly booted device reconnects in {} ms and input works",
        back.as_millis()
    );
    Ok(())
}

fn stray_wake(executable: &Path) -> Result<(), String> {
    let mut sandbox = Sandbox::open(executable).map_err(|error| error.to_string())?;
    ready(&mut sandbox, "initial connection")?;
    let generation = sandbox.snapshot.generation;
    // Windows sends two resume notices; one with no suspend seen must not drop a live link.
    sandbox.host_event(HostEvent::Resumed)?;
    settle(&mut sandbox, Duration::from_millis(500))?;
    check(
        sandbox.snapshot.connected
            && sandbox.snapshot.generation == generation
            && device(&sandbox, "idle"),
        "a wake without a suspend disturbed the live link",
    )?;
    println!(
        "PASS PRESENCE stray wake: a resume with no suspend leaves the live session untouched"
    );
    Ok(())
}

/// Runs every presence case against fresh runner processes.
pub fn presence_smoke(executable: &Path) -> Result<(), String> {
    lock_and_unlock(executable)?;
    sleep_and_wake(executable)?;
    console_switch(executable)?;
    power_cut_during_sleep(executable)?;
    stray_wake(executable)
}
