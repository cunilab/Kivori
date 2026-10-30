//! Cross-process connection regressions; every timeout here uses real elapsed time.
use super::*;
use kivori_model::ProtocolVersion;
use kivori_protocol::{
    encode_message, ControlId, InputEvent, InputKind, Message, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

fn check(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_string())
}

fn until(
    sandbox: &mut Sandbox,
    duration: Duration,
    predicate: impl Fn(&Sandbox) -> bool,
) -> Result<(), String> {
    let deadline = Instant::now() + duration;
    loop {
        sandbox.tick();
        if predicate(sandbox) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Connection scenario timed out: {:?}",
                sandbox.snapshot.error
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn connected(sandbox: &mut Sandbox) -> Result<(), String> {
    until(sandbox, Duration::from_secs(5), |s| {
        s.snapshot.connected && s.snapshot.frame.is_some()
    })?;
    check(
        sandbox.snapshot.error.is_none(),
        "unexpected connection error",
    )
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

/// Exercises the actual child runner, real desktop session and fake OS without a GUI or hardware.
pub fn connection_smoke(executable: &Path) -> Result<(), String> {
    let mut sandbox = Sandbox::open(executable).map_err(|error| error.to_string())?;
    connected(&mut sandbox)?;
    sandbox.input("clockwise")?;
    until(&mut sandbox, Duration::from_secs(5), |s| {
        s.snapshot.volume == 52 && s.snapshot.detents == 1
    })?;
    check(
        sandbox.snapshot.volume == 52,
        "initial detent did not reach fake OS",
    )?;
    let old_nonce = sandbox
        .session
        .current_session()
        .ok_or("missing initial session")?;
    let old_generation = sandbox.snapshot.generation;
    sandbox.input("tap")?;
    sandbox.connection("disconnect")?;
    check(
        !sandbox.snapshot.connected
            && sandbox.snapshot.frame.is_none()
            && sandbox.link.is_none()
            && sandbox.edges.is_empty(),
        "disconnect retained live state/queues",
    )?;
    check(
        sandbox.input("tap").is_err(),
        "input admitted during outage",
    )?;
    sandbox.connection("reconnect")?;
    connected(&mut sandbox)?;
    settle(&mut sandbox, Duration::from_millis(400))?;
    check(
        sandbox.snapshot.generation > old_generation
            && sandbox.session.current_session() != Some(old_nonce),
        "reconnect did not renew session",
    )?;
    check(
        sandbox.snapshot.volume == 52 && sandbox.snapshot.media_triggers == 0,
        "old queued action replayed or fake OS reset",
    )?;
    check(
        sandbox.input_for_generation("tap", old_generation).is_err(),
        "late old-generation webview input was admitted after reconnect",
    )?;
    println!("PASS S5.2 queued-input disconnect/reconnect: new nonce/generation, retained fake OS, no replay");

    // An otherwise valid fresh-sequence frame carries the OLD nonce. This deliberately isolates
    // session admission from duplicate-sequence rejection; reconnect below resets the injected gap.
    let mut stale = Default::default();
    encode_message(
        &Message::InputEvent(InputEvent {
            session: old_nonce,
            gesture_id: 19,
            control: ControlId::Button,
            kind: InputKind::Press,
            device_ms: sandbox.started.elapsed().as_millis() as u32,
        }),
        ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
        0x4000,
        &mut stale,
    )
    .map_err(|_| "stale frame encode")?;
    sandbox
        .link
        .as_mut()
        .ok_or("missing link")?
        .inbound
        .extend(stale);
    let rejected = sandbox.rejected_inputs;
    until(&mut sandbox, Duration::from_secs(1), |s| {
        s.rejected_inputs > rejected
    })?;
    check(
        sandbox.snapshot.media_triggers == 0,
        "stale nonce frame executed",
    )?;
    sandbox.connection("reconnect")?;
    connected(&mut sandbox)?;
    sandbox.input("tap")?;
    until(&mut sandbox, Duration::from_secs(2), |s| {
        s.snapshot.media_triggers == 1
    })?;
    println!(
        "PASS S5.2 stale-nonce frame rejected; fresh-session press still executes exactly once"
    );

    // A press has reached the old Runtime but its release never does; no completed press may
    // appear in the replacement Runtime/session.
    sandbox.input("press")?;
    settle(&mut sandbox, Duration::from_millis(100))?;
    sandbox.connection("disconnect")?;
    sandbox.connection("reconnect")?;
    connected(&mut sandbox)?;
    settle(&mut sandbox, Duration::from_millis(600))?;
    check(
        sandbox.snapshot.media_triggers == 1,
        "half press replayed after reconnect",
    )?;
    for _ in 0..10 {
        sandbox.connection("disconnect")?;
        sandbox.connection("reconnect")?;
        connected(&mut sandbox)?;
    }
    check(
        sandbox.snapshot.media_triggers == 1 && sandbox.link.is_some(),
        "rapid reconnect duplicated actions or lost owner",
    )?;
    println!("PASS S5.2 half-press interruption and ten reconnect cycles: one live owner, no phantom action");

    let detents = sandbox.snapshot.detents;
    sandbox.connection("fragmented")?;
    connected(&mut sandbox)?;
    sandbox.input("clockwise")?;
    until(&mut sandbox, Duration::from_secs(2), |s| {
        s.snapshot.volume == 54
    })?;
    check(
        sandbox.snapshot.detents == detents + 1,
        "fragmented frame duplicated/lost detent",
    )?;
    println!("PASS S5.2 seven-byte bidirectional fragmentation: handshake and exact-once detent");

    for mode in ["wrong-major", "unknown-product", "unknown-board"] {
        sandbox.connection(mode)?;
        until(&mut sandbox, Duration::from_secs(5), |s| {
            s.snapshot.error.is_some()
        })?;
        check(
            !sandbox.snapshot.connected
                && sandbox.snapshot.frame.is_none()
                && sandbox.link.is_none(),
            "rejected peer retained usable state",
        )?;
        check(
            sandbox.input("tap").is_err(),
            "rejected peer admitted input",
        )?;
        println!("PASS S5.2 {mode}: real protocol/identity admission refused peer");
        sandbox.connection("reconnect")?;
        connected(&mut sandbox)?;
    }

    sandbox.connection("silent")?;
    until(&mut sandbox, Duration::from_secs(5), |s| {
        s.snapshot.error.is_some()
    })?;
    check(
        sandbox
            .snapshot
            .error
            .as_ref()
            .is_some_and(|error| error.contains("heartbeat")),
        "silent peer did not fail real heartbeat deadline",
    )?;
    check(
        sandbox.link.is_none() && sandbox.snapshot.frame.is_none(),
        "heartbeat loss retained child/frame",
    )?;
    sandbox.connection("reconnect")?;
    connected(&mut sandbox)?;
    println!(
        "PASS S5.2 missing heartbeats: real elapsed timeout, boundary cleanup, successful retry"
    );

    sandbox.input("tap")?;
    sandbox.connection("exit")?;
    until(&mut sandbox, Duration::from_secs(3), |s| {
        s.snapshot.error.is_some()
    })?;
    check(
        sandbox
            .snapshot
            .error
            .as_ref()
            .is_some_and(|error| error.contains("closed")),
        "child exit did not surface EOF",
    )?;
    check(
        sandbox.link.is_none() && sandbox.edges.is_empty() && !sandbox.snapshot.connected,
        "child EOF retained session/queues",
    )?;
    sandbox.connection("reconnect")?;
    connected(&mut sandbox)?;
    settle(&mut sandbox, Duration::from_millis(500))?;
    check(
        sandbox.snapshot.media_triggers == 1 && sandbox.snapshot.volume == 54,
        "EOF retry replayed old input or reset fake OS",
    )?;
    sandbox.input("tap")?;
    until(&mut sandbox, Duration::from_secs(2), |s| {
        s.snapshot.media_triggers == 2
    })?;
    println!("PASS S5.2 child EOF: no stale replay on retry, fresh press executes once");
    sandbox.connection("disconnect")?;
    Ok(())
}
