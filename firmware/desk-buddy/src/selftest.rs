//! In-firmware self-test harness (`wokwi` feature) — the pre-hardware simulation gate.
//!
//! This is an INTERNAL on-target integration self-test: the simulated host lives inside the firmware, so
//! [`crate::sim_probe::LoopbackTransport`] feeds real encoded frames to the REAL [`Dispatcher`], driving
//! the real [`DeviceState`] and the canonical renderer. Every check prints one deterministic
//! `KIVORI-SIM …` line; `sim/wokwi/scenarios/*.yaml` wait on those lines and the run ends with
//! `KIVORI-SIM ALL PASS` (or `FAIL`).
//!
//! It deliberately does NOT exercise the serial peripheral — external serial injection lives in
//! [`crate::external`] (`wokwi-serial` feature), which receives Wokwi `write-serial` bytes through the
//! real USB Serial/JTAG transport.
//!
//! What this proves: the embedded runtime boots, the protocol/lifecycle/render paths execute on the
//! real target ISA under the real esp-hal runtime, and diagnostics stay redacted.
//! What it does NOT prove: anything electrical, any real display controller's init sequence or colour
//! order, USB enumeration, or sustainable physical frame rate. See `sim/wokwi/README.md`.

use crate::health::{build_health, build_pong};
use crate::ports::Clock;
use crate::proto::{DeviceIdentity, Dispatcher};
use crate::render::{TileRenderer, TILE_COLS, TILE_COUNT, TILE_H, TILE_W};
use crate::sim_probe::{LoopbackTransport, TileProbe};
use crate::state::{DeviceEvent, DeviceState};
use esp_println::println;
use heapless::Vec;
use kivori_assets::AssetBlob;
use kivori_model::{Capabilities, CompanionState, ProtocolVersion, SendableState, PREVIEW_FPS};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, Message, Ping, SetState, MAX_FRAME,
    MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

/// The canonical asset blob, compiled at build time (see `build.rs`). Never parsed from source art.
static ASSETS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kivori.assets"));

const TAG: &str = "KIVORI-SIM";

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: [0x5A; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: Capabilities::MASCOT_INTERACTION
            .union(Capabilities::PHYSICAL_INPUT_V1)
            .union(Capabilities::PRESENTATION_V1),
    }
}

fn wire_version() -> ProtocolVersion {
    ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
}

/// Encodes `msg` at `version`/`seq` and queues it as if a host had sent it.
fn host_send(pipe: &mut LoopbackTransport, msg: &Message, version: ProtocolVersion, seq: u16) {
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    if encode_message(msg, version, seq, &mut wire).is_ok() {
        pipe.host_send(&wire);
    }
}

/// Decodes the first message the device wrote, if any.
fn device_reply(pipe: &LoopbackTransport) -> Option<Message> {
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    for packet in pipe.device_output().split(|&b| b == 0) {
        if packet.is_empty() {
            continue;
        }
        if let Ok((_, msg)) = decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
            return Some(msg);
        }
    }
    None
}

/// Prints one check result and folds it into the running pass flag.
fn check(pass: &mut bool, ok: bool, name: &str) {
    if ok {
        println!("{TAG} PASS {name}");
    } else {
        println!("{TAG} FAIL {name}");
        *pass = false;
    }
}

/// Runs every simulation check, printing assertion lines. Returns `true` if all passed.
pub fn run<C: Clock>(clock: &C) -> bool {
    let mut pass = true;
    println!("{TAG} BOOT firmware=kivori-firmware target=esp32c3 protocol={PROTOCOL_MAJOR}.{PROTOCOL_MINOR}");

    // 1. Deterministic monotonic clock (ADR-0003): time advances and never goes backwards.
    let t0 = clock.now_ms();
    let mut spins = 0u32;
    let mut t1 = clock.now_ms();
    while t1 == t0 && spins < 5_000_000 {
        t1 = clock.now_ms();
        spins += 1;
    }
    check(&mut pass, t1 >= t0, "clock-monotonic");
    check(&mut pass, t1 > t0, "clock-advances");

    // 2. Lifecycle: the device owns booting → offline; no host involvement (FR-014).
    let mut device = DeviceState::new();
    check(
        &mut pass,
        device.current() == CompanionState::Booting,
        "lifecycle-boots-in-booting",
    );
    let after_boot = device.apply(DeviceEvent::BootComplete);
    check(
        &mut pass,
        after_boot == Some(CompanionState::Offline),
        "lifecycle-booting-to-offline",
    );

    let mut pipe = LoopbackTransport::new();
    let mut dispatcher = Dispatcher::new(identity());

    // 3. Handshake: Hello → HelloAck echoing the nonce, advertising identity + capabilities (FR-002).
    let nonce = 0xDEAD_BEEF_u32;
    host_send(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::NONE,
            nonce,
        }),
        wire_version(),
        0,
    );
    let polled = dispatcher
        .poll(&mut pipe, &mut device, clock.now_ms())
        .is_ok();
    check(&mut pass, polled, "protocol-poll-ok");
    match device_reply(&pipe) {
        Some(Message::HelloAck(ack)) => {
            check(&mut pass, ack.nonce_echo == nonce, "handshake-nonce-echo");
            check(
                &mut pass,
                ack.device_id == identity().device_id,
                "handshake-identity",
            );
            check(
                &mut pass,
                ack.device_caps == identity().capabilities,
                "capability-advertised",
            );
        }
        _ => check(&mut pass, false, "handshake-helloack"),
    }
    pipe.clear_output();

    // 4. Major-version rejection: a frame from an unsupported major is dropped with no reply (FR-003).
    host_send(
        &mut pipe,
        &Message::Ping(Ping { t_ms: 1 }),
        ProtocolVersion::new(PROTOCOL_MAJOR + 1, 0),
        1,
    );
    let _ = dispatcher.poll(&mut pipe, &mut device, clock.now_ms());
    check(
        &mut pass,
        device_reply(&pipe).is_none(),
        "version-reject-no-reply",
    );
    pipe.clear_output();

    // 5. Heartbeat: Ping → Pong echoing the timestamp (protocol §7).
    host_send(
        &mut pipe,
        &Message::Ping(Ping { t_ms: 4242 }),
        wire_version(),
        2,
    );
    let _ = dispatcher.poll(&mut pipe, &mut device, clock.now_ms());
    match device_reply(&pipe) {
        Some(Message::Pong(pong)) => {
            check(&mut pass, pong.t_ms_echo == 4242, "heartbeat-pong-echo");
        }
        _ => check(&mut pass, false, "heartbeat-pong"),
    }
    pipe.clear_output();

    // 6. SetState → the device adopts it and reports back (FR-012).
    host_send(
        &mut pipe,
        &Message::SetState(SetState {
            desired: SendableState::Happy,
            at_ms: None,
        }),
        wire_version(),
        3,
    );
    let _ = dispatcher.poll(&mut pipe, &mut device, clock.now_ms());
    check(
        &mut pass,
        device.current() == CompanionState::Happy,
        "setstate-applied",
    );
    match device_reply(&pipe) {
        Some(Message::StateReport(report)) => check(
            &mut pass,
            report.reported == CompanionState::Happy,
            "statereport-emitted",
        ),
        _ => check(&mut pass, false, "statereport-emitted"),
    }
    pipe.clear_output();

    // 7. Malformed-frame recovery: garbage is dropped without panic or state change, and the very next
    //    valid frame is still served (SC-008).
    pipe.host_send(&[0x02, 0xFF, 0x00, 0x13, 0x37, 0x00]);
    let survived = dispatcher
        .poll(&mut pipe, &mut device, clock.now_ms())
        .is_ok();
    check(&mut pass, survived, "malformed-no-panic");
    check(
        &mut pass,
        device.current() == CompanionState::Happy && device_reply(&pipe).is_none(),
        "malformed-no-side-effect",
    );
    pipe.clear_output();
    host_send(
        &mut pipe,
        &Message::Ping(Ping { t_ms: 7 }),
        wire_version(),
        5,
    );
    let _ = dispatcher.poll(&mut pipe, &mut device, clock.now_ms());
    check(
        &mut pass,
        matches!(device_reply(&pipe), Some(Message::Pong(_))),
        "malformed-recovery",
    );
    pipe.clear_output();

    // 8. `booting`/`offline` are device-originated and unrepresentable in a SetState payload (FR-014/015):
    //    SendableState has exactly four variants, none of them device-originated.
    let sendable_ok = SendableState::ALL.len() == 4
        && SendableState::ALL
            .iter()
            .all(|s| !s.to_companion().is_device_originated());
    check(&mut pass, sendable_ok, "sendable-excludes-device-states");
    check(
        &mut pass,
        CompanionState::Booting.is_device_originated()
            && CompanionState::Offline.is_device_originated(),
        "device-originated-states",
    );

    // 9. Renderer integration: real scenes → RGB565 tiles through the canonical renderer, and a second
    //    identical frame flushes nothing (change-driven refresh; FR-013).
    match AssetBlob::parse(ASSETS) {
        Ok(blob) => {
            check(&mut pass, true, "assets-parsed");
            let mut renderer = TileRenderer::new();
            let mut probe = TileProbe::new();
            let rendered = renderer
                .render(&blob, CompanionState::Idle, 0, &mut probe)
                .is_ok();
            check(&mut pass, rendered, "render-ok");
            let bands = probe.records();
            check(&mut pass, bands.len() == TILE_COUNT, "tile-count-36");
            let geometry_ok = bands.iter().enumerate().all(|(i, r)| {
                r.rect.w == TILE_W
                    && r.rect.h == TILE_H
                    && r.rect.x == (i % TILE_COLS) as u16 * TILE_W
                    && r.rect.y == (i / TILE_COLS) as u16 * TILE_H
                    && r.pixels == TILE_W as usize * TILE_H as usize
            });
            check(&mut pass, geometry_ok, "tile-geometry-rgb565");
            println!("{TAG} INFO tile0-hash={:016x}", bands[0].hash);
            let idle_hashes: Vec<u64, TILE_COUNT> = bands.iter().map(|r| r.hash).collect();

            probe.reset();
            let again = renderer
                .render(&blob, CompanionState::Idle, 0, &mut probe)
                .is_ok();
            check(&mut pass, again, "render-repeat-ok");
            check(&mut pass, probe.flushes == 0, "change-driven-no-reflush");

            // A different state flushes exactly the tiles whose content changed; tiles that stay
            // identical (e.g. background corners) are skipped.
            let mut reference = TileProbe::new();
            let _ = TileRenderer::new().render(&blob, CompanionState::Happy, 0, &mut reference);
            let changed = reference
                .records()
                .iter()
                .zip(idle_hashes.iter())
                .filter(|(happy, idle)| happy.hash != **idle)
                .count();
            probe.reset();
            let _ = renderer.render(&blob, CompanionState::Happy, 0, &mut probe);
            check(
                &mut pass,
                changed > 0 && probe.flushes as usize == changed,
                "change-driven-reflush-on-change",
            );
            println!("{TAG} INFO preview-fps={PREVIEW_FPS}");
        }
        Err(_) => check(&mut pass, false, "assets-parsed"),
    }

    // 10. Diagnostics carry only safe values — the wire types have no field for payload bytes, a raw
    //     identity, or a path (ADR-0005). Health reports free memory; Pong reports uptime.
    let health = build_health(4096);
    let pong = build_pong(11, clock.now_ms());
    check(
        &mut pass,
        health.free_bytes == 4096,
        "diagnostics-safe-health",
    );
    check(&mut pass, pong.t_ms_echo == 11, "diagnostics-safe-pong");

    if pass {
        println!("{TAG} ALL PASS");
    } else {
        println!("{TAG} ALL FAIL");
    }
    pass
}
