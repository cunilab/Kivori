//! One long virtual session that carries both protocol sequence counters past their `u16` wrap,
//! with input flowing the whole time. Real elapsed time; it finds defects before a physical soak
//! but earns no soak hours (validation sustained-use rows stay open).
use super::*;

/// Keeps the edge queue fed without hitting its capacity check.
const BATCH: usize = 24;

fn check(ok: bool, message: &str) -> Result<(), String> {
    ok.then_some(()).ok_or_else(|| message.to_string())
}

/// Counts how often each direction's sequence number went backwards (wrapped) while sampling.
#[derive(Default)]
struct Wraps {
    last: Option<(u16, Option<u16>)>,
    outbound: u32,
    inbound: u32,
}

impl Wraps {
    fn sample(&mut self, (tx, rx): (u16, Option<u16>)) {
        if let Some((last_tx, last_rx)) = self.last {
            self.outbound += u32::from(tx < last_tx);
            self.inbound += u32::from(matches!((last_rx, rx), (Some(a), Some(b)) if b < a));
        }
        self.last = Some((tx, rx));
    }
}

impl Wraps {
    const fn both(&self) -> bool {
        self.outbound >= 1 && self.inbound >= 1
    }
}

/// Turns the knob right then left in alternation until both sequence numbers have wrapped, then
/// drains until every queued detent has reached the desktop. Fails on any error, reconnect or
/// protocol anomaly. Returns the elapsed time, wrap counts and detents sent (always an even number,
/// so the paired turns cancel out).
fn run(sandbox: &mut Sandbox, limit: Duration) -> Result<(Duration, Wraps, usize), String> {
    let started = Instant::now();
    let mut wraps = Wraps::default();
    let generation = sandbox.snapshot.generation;
    let mut queued = 0usize;
    let mut last_report = Instant::now();
    while !wraps.both() || sandbox.snapshot.detents < queued {
        if let Some(error) = &sandbox.snapshot.error {
            return Err(format!(
                "endurance: {error} after {} detents",
                sandbox.snapshot.detents
            ));
        }
        check(
            sandbox.snapshot.generation == generation && sandbox.snapshot.connected,
            "endurance: the session dropped or reconnected",
        )?;
        // Keep turning until both sides wrapped, finishing on a left turn to restore the volume.
        while (!wraps.both() || !queued.is_multiple_of(2))
            && queued < sandbox.snapshot.detents + BATCH
        {
            let control = if queued.is_multiple_of(2) {
                "clockwise"
            } else {
                "counterclockwise"
            };
            if sandbox.input(control).is_err() {
                break;
            }
            queued += 1;
        }
        sandbox.tick();
        wraps.sample(sandbox.session.sequence_positions());
        if last_report.elapsed() >= Duration::from_secs(30) {
            last_report = Instant::now();
            eprintln!(
                "endurance: {} detents in {:?}",
                sandbox.snapshot.detents,
                started.elapsed()
            );
        }
        if started.elapsed() >= limit {
            return Err(format!(
                "endurance: {} detents and wraps host {} / device {} within {limit:?}",
                sandbox.snapshot.detents, wraps.outbound, wraps.inbound
            ));
        }
        std::thread::sleep(Duration::from_micros(500));
    }
    Ok((started.elapsed(), wraps, queued))
}

/// Opt-in long session: `virtual_device <runner> --endurance-smoke`.
pub fn endurance_smoke(executable: &Path) -> Result<(), String> {
    // The flood (about 190 detents a second, each with presentation traffic back) is far beyond what
    // the chip's 512 inbound bytes per tick at its frame rate could take, so it runs on zero-time
    // frames; modelled timing would measure that limit, not wrap handling.
    let mut sandbox = Sandbox::open_ideal(executable).map_err(|error| error.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !(sandbox.snapshot.connected && sandbox.snapshot.frame.is_some()) {
        sandbox.tick();
        check(
            Instant::now() < deadline,
            "endurance: initial connection timed out",
        )?;
        std::thread::sleep(Duration::from_millis(5));
    }
    let generation = sandbox.snapshot.generation;
    let (elapsed, wraps, sent) = run(&mut sandbox, Duration::from_secs(1200))?;
    // Let the last replies settle, then require an exact, anomaly-free ledger.
    let settle = Instant::now() + Duration::from_millis(500);
    while Instant::now() < settle {
        sandbox.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
    let diagnostics = sandbox.session.diagnostics();
    check(
        sandbox.snapshot.error.is_none()
            && sandbox.snapshot.connected
            && sandbox.snapshot.generation == generation,
        "endurance: session did not survive",
    )?;
    check(
        sandbox.snapshot.detents == sent,
        &format!(
            "endurance: {} detents arrived for {sent} sent",
            sandbox.snapshot.detents
        ),
    )?;
    check(
        diagnostics.sequence_gaps == 0
            && diagnostics.malformed_frames == 0
            && diagnostics.device_errors == 0,
        &format!("endurance: protocol anomalies {diagnostics:?}"),
    )?;
    check(
        sandbox.rejected_inputs == 0,
        "endurance: inputs were rejected as stale",
    )?;
    check(
        sandbox.snapshot.volume == 50,
        &format!(
            "endurance: paired detents left volume at {}",
            sandbox.snapshot.volume
        ),
    )?;
    // Input still works on the far side of the wrap.
    sandbox.input("tap")?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while sandbox.snapshot.media_triggers == 0 {
        sandbox.tick();
        check(
            Instant::now() < deadline,
            "endurance: press after the wrap did not run",
        )?;
        std::thread::sleep(Duration::from_millis(5));
    }
    println!(
        "PASS ENDURANCE one session, {sent} detents in {} s; u16 sequence wraps host {} / device {}: no gap, malformed frame, device error, stale rejection or reconnect; volume back to 50; press works after the wrap",
        elapsed.as_secs(),
        wraps.outbound,
        wraps.inbound
    );
    Ok(())
}
