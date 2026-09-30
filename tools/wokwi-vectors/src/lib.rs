//! Canonical Wokwi automation vectors, generated from the REAL `kivori-protocol` codec.
//!
//! Every byte a Wokwi scenario injects with `write-serial` is produced here by `encode_message`, so the
//! simulator exercises the true framing (COBS + header + CRC-32 + postcard). No wire-format constant is
//! duplicated: change the protocol and these vectors change with it (a stale-vector check in CI fails
//! the build until they are regenerated).
//!
//! Output is deterministic — fixed nonces/timestamps/sequence numbers, no clock or RNG input — so
//! regenerating produces byte-identical files.

use core::fmt::Write as _;
use kivori_model::{Capabilities, ProtocolVersion, SendableState};
use kivori_protocol::{
    encode_message, FirmwareVersion, Hello, Message, Ping, SetState, MAX_WIRE, PROTOCOL_MAJOR,
    PROTOCOL_MINOR,
};

/// A named wire vector: the exact bytes a scenario writes to the device's serial input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vector {
    /// Stable case name (used in file output and scenario comments).
    pub name: &'static str,
    /// What the case proves, for the generated documentation.
    pub purpose: &'static str,
    /// The complete wire packet (COBS-framed, `0x00`-delimited) or deliberately corrupted bytes.
    pub bytes: Vec<u8>,
}

/// Fixed desktop identity used by every generated `Hello` (determinism).
fn desktop_version() -> FirmwareVersion {
    FirmwareVersion {
        major: 1,
        minor: 0,
        patch: 0,
    }
}

/// Encodes `msg` at `version`/`seq` through the real codec.
fn frame(msg: &Message, version: ProtocolVersion, seq: u16) -> Vec<u8> {
    let mut wire: heapless::Vec<u8, MAX_WIRE> = heapless::Vec::new();
    encode_message(msg, version, seq, &mut wire).expect("canonical vector encodes");
    wire.to_vec()
}

fn current() -> ProtocolVersion {
    ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
}

/// The fixed nonce every generated `Hello` echoes, so scenarios can assert the echo.
pub const HELLO_NONCE: u32 = 0x1234_5678;
/// The fixed `Ping` timestamp scenarios assert in the `Pong` echo.
pub const PING_T_MS: u32 = 4242;

fn hello(nonce: u32, caps: Capabilities) -> Message {
    Message::Hello(Hello {
        desktop_version: desktop_version(),
        desktop_caps: caps,
        nonce,
    })
}

/// Builds the full catalogue of canonical vectors, in a stable order.
#[must_use]
pub fn vectors() -> Vec<Vector> {
    // Corrupted frames are derived from a real encoding so only the intended field is wrong.
    let mut crc_invalid = frame(&Message::Ping(Ping { t_ms: PING_T_MS }), current(), 4);
    // The final byte before the 0x00 delimiter belongs to the CRC; flipping it invalidates the frame.
    let crc_pos = crc_invalid.len().saturating_sub(2);
    crc_invalid[crc_pos] ^= 0xFF;

    vec![
        Vector {
            name: "hello",
            purpose: "handshake open at the current protocol version",
            bytes: frame(&hello(HELLO_NONCE, Capabilities::NONE), current(), 0),
        },
        Vector {
            name: "hello_compatible_minor",
            purpose: "same major, higher minor — must be accepted (FR-003)",
            bytes: frame(
                &hello(HELLO_NONCE, Capabilities::NONE),
                ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR + 5),
                0,
            ),
        },
        Vector {
            name: "hello_incompatible_major",
            purpose: "unsupported major — must be rejected with no valid response (FR-003)",
            bytes: frame(
                &hello(HELLO_NONCE, Capabilities::NONE),
                ProtocolVersion::new(PROTOCOL_MAJOR + 1, 0),
                0,
            ),
        },
        Vector {
            name: "hello_unsupported_capability",
            purpose:
                "host advertises capability bits the device does not implement — intersection must hold",
            bytes: frame(
                &hello(HELLO_NONCE, Capabilities::from_bits(0xFFFF_FFFF)),
                current(),
                // seq 0 would repeat `hello_compatible_minor` and be dropped as a duplicate; 2 is a
                // gap, so it is served, and the following seq-1 `ping` is a gap rather than a repeat.
                2,
            ),
        },
        Vector {
            name: "ping",
            purpose: "heartbeat — expects a Pong echoing the timestamp",
            bytes: frame(&Message::Ping(Ping { t_ms: PING_T_MS }), current(), 1),
        },
        Vector {
            name: "set_state_idle",
            purpose: "desired state idle — expects StateReport(idle)",
            bytes: frame(
                &Message::SetState(SetState {
                    desired: SendableState::Idle,
                    at_ms: None,
                }),
                current(),
                2,
            ),
        },
        Vector {
            name: "set_state_happy",
            purpose: "desired state happy — expects StateReport(happy)",
            bytes: frame(
                &Message::SetState(SetState {
                    desired: SendableState::Happy,
                    at_ms: None,
                }),
                current(),
                3,
            ),
        },
        // Sequence policy (contracts/protocol.md §6): a duplicate must not re-apply side effects; a gap
        // and a wraparound are both valid decodes the device accepts.
        Vector {
            name: "ping_seq_duplicate",
            purpose: "same sequence number as the previous frame — side effects must not repeat",
            bytes: frame(&Message::Ping(Ping { t_ms: PING_T_MS }), current(), 1),
        },
        Vector {
            name: "ping_seq_gap",
            purpose: "forward jump in sequence — accepted, classified as a gap",
            bytes: frame(&Message::Ping(Ping { t_ms: PING_T_MS }), current(), 900),
        },
        Vector {
            name: "ping_seq_wrap_max",
            purpose: "sequence at u16::MAX, before the wraparound",
            bytes: frame(
                &Message::Ping(Ping { t_ms: PING_T_MS }),
                current(),
                u16::MAX,
            ),
        },
        Vector {
            name: "ping_seq_wrap_zero",
            purpose: "sequence wrapped to 0 — must be treated as the expected next value",
            bytes: frame(&Message::Ping(Ping { t_ms: PING_T_MS }), current(), 0),
        },
        Vector {
            name: "ping_crc_invalid",
            purpose: "valid framing, corrupted CRC-32 — must be dropped, never dispatched (SC-008)",
            bytes: crc_invalid,
        },
        Vector {
            name: "malformed_cobs",
            purpose: "invalid COBS run length — must be dropped without panic (SC-008)",
            bytes: vec![0x05, 0xFF, 0x01, 0x00],
        },
        Vector {
            name: "truncated_frame",
            purpose: "declared payload longer than the frame — must be dropped (protocol §8)",
            bytes: vec![0x03, 0x56, 0x4B, 0x00],
        },
    ]
}

/// Formats `bytes` as a YAML inline sequence for a `write-serial` step.
#[must_use]
pub fn yaml_bytes(bytes: &[u8]) -> String {
    let items: Vec<String> = bytes.iter().map(|b| b.to_string()).collect();
    format!("[{}]", items.join(", "))
}

/// Looks a vector up by name.
///
/// # Panics
/// Panics if `name` is not a known vector (a generator bug, caught in tests).
#[must_use]
pub fn vector(name: &str) -> Vector {
    vectors()
        .into_iter()
        .find(|v| v.name == name)
        .unwrap_or_else(|| panic!("unknown vector: {name}"))
}

/// Header written to every generated file so nobody hand-edits them.
const BANNER: &str = "# GENERATED FILE — do not edit.\n\
                      # Produced by `cargo run -p kivori-wokwi-vectors` from the real kivori-protocol\n\
                      # codec; every write-serial byte array below is a genuine wire frame. CI regenerates\n\
                      # this file and fails if it differs (stale vectors).\n";

fn write_serial(out: &mut String, name: &str) {
    let v = vector(name);
    let _ = writeln!(out, "  # {}: {}", v.name, v.purpose);
    let _ = writeln!(out, "  - write-serial: {}", yaml_bytes(&v.bytes));
}

fn wait(out: &mut String, text: &str) {
    let _ = writeln!(out, "  - wait-serial: '{text}'");
}

/// Emits a fixed delay step. Prefer awaiting an observable marker; use only for a documented
/// Wokwi timing workaround.
#[allow(dead_code)]
fn delay(out: &mut String, ms: u32) {
    let _ = writeln!(out, "  - delay: {ms}ms");
}

fn header(out: &mut String, name: &str, doc: &str) {
    out.push_str(BANNER);
    let _ = writeln!(out, "#");
    // Every line of the doc block needs its own `#`, or the generated file is not valid YAML.
    for line in doc.lines() {
        let _ = writeln!(out, "# {}", line.trim());
    }
    let _ = writeln!(out, "name: {name}\nversion: 1\nauthor: Kivori\nsteps:");
}

/// External-serial protocol scenario: real frames in, semantic markers out.
pub fn protocol_serial() -> String {
    let mut s = String::new();
    header(
        &mut s,
        "kivori-protocol-serial",
        "External serial test: Wokwi injects real wire frames over USB Serial/JTAG; the firmware's real\n\
         COBS/CRC/postcard decoder, sequence policy, and Dispatcher handle them and emit redacted markers.",
    );
    wait(&mut s, "KIVORI-EXT READY");

    // Handshake.
    write_serial(&mut s, "hello");
    wait(&mut s, "KIVORI-EXT RX kind=Hello");
    wait(&mut s, "KIVORI-EXT TX kind=HelloAck nonce=ok");

    // Heartbeat.
    write_serial(&mut s, "ping");
    wait(&mut s, "KIVORI-EXT RX kind=Ping");
    let _ = writeln!(
        s,
        "  - wait-serial: 'KIVORI-EXT TX kind=Pong echo={PING_T_MS}'"
    );

    // Compatible minor difference is accepted.
    write_serial(&mut s, "hello_compatible_minor");
    wait(&mut s, "KIVORI-EXT TX kind=HelloAck nonce=ok");

    // Unsupported capability bits: the device still answers, advertising only what it implements.
    write_serial(&mut s, "hello_unsupported_capability");
    wait(
        &mut s,
        &format!(
            "KIVORI-EXT CAPS advertised={:#010x}",
            Capabilities::MASCOT_INTERACTION
                .union(Capabilities::PHYSICAL_INPUT_V1)
                .union(Capabilities::PRESENTATION_V1)
                .bits()
        ),
    );

    // Unsupported major: dropped, and NO response frame is produced.
    write_serial(&mut s, "hello_incompatible_major");
    wait(&mut s, "KIVORI-EXT DROP reason=decode");

    // Corrupted CRC and malformed COBS: dropped without panic.
    write_serial(&mut s, "ping_crc_invalid");
    wait(&mut s, "KIVORI-EXT DROP reason=decode");
    write_serial(&mut s, "malformed_cobs");
    wait(&mut s, "KIVORI-EXT DROP reason=decode");
    write_serial(&mut s, "truncated_frame");
    wait(&mut s, "KIVORI-EXT DROP reason=decode");

    // Recovery: the very next valid frame is served normally (SC-008).
    write_serial(&mut s, "ping");
    let _ = writeln!(
        s,
        "  - wait-serial: 'KIVORI-EXT TX kind=Pong echo={PING_T_MS}'"
    );

    // Sequence policy: duplicate must not repeat side effects; gap and wraparound are accepted.
    write_serial(&mut s, "ping_seq_duplicate");
    wait(&mut s, "KIVORI-EXT SEQ seq=1 class=duplicate");
    write_serial(&mut s, "ping_seq_gap");
    wait(&mut s, "KIVORI-EXT SEQ seq=900 class=gap");
    write_serial(&mut s, "ping_seq_wrap_max");
    write_serial(&mut s, "ping_seq_wrap_zero");
    wait(&mut s, "KIVORI-EXT SEQ seq=0 class=ok");
    s
}

/// External-serial state cycle: SetState in, StateReport out, invalid input, recovery, completion.
pub fn state_cycle_serial() -> String {
    let mut s = String::new();
    header(
        &mut s,
        "kivori-state-cycle-serial",
        "External serial test: desired-state commands arrive as real SetState frames and the device\n\
         reports back. `booting`/`offline` are unrepresentable in the payload (FR-014/FR-015). An invalid\n\
         frame must not corrupt the current state, and the next valid frame must still be served.\n\
         The run ends on ALL PASS state-cycle, which the firmware emits ONLY after every post-condition\n\
         it observed genuinely held — never merely because a step executed.",
    );
    wait(&mut s, "KIVORI-EXT READY iface=usb-serial-jtag");

    // Handshake first, so the device has an active session.
    write_serial(&mut s, "hello");
    wait(&mut s, "KIVORI-EXT RX kind=Hello");
    wait(&mut s, "KIVORI-EXT TX kind=HelloAck nonce=ok");

    // Desired state idle: received, reported, and reflected in the device's own state.
    write_serial(&mut s, "set_state_idle");
    wait(&mut s, "KIVORI-EXT RX kind=SetState");
    wait(&mut s, "KIVORI-EXT TX kind=StateReport reported=idle");
    wait(&mut s, "KIVORI-EXT STATE current=idle");

    // Desired state happy: same three observations.
    write_serial(&mut s, "set_state_happy");
    wait(&mut s, "KIVORI-EXT TX kind=StateReport reported=happy");
    wait(&mut s, "KIVORI-EXT STATE current=happy");

    // An invalid frame must be dropped without disturbing the current state (SC-008)…
    write_serial(&mut s, "malformed_cobs");
    wait(&mut s, "KIVORI-EXT DROP reason=decode");
    wait(&mut s, "KIVORI-EXT STATE current=happy");

    // …and the very next valid frame must still be served, proving the loop is alive.
    write_serial(&mut s, "ping");
    wait(&mut s, "KIVORI-EXT TX kind=Pong");

    // Completion is asserted last and is emitted by the firmware only when the sendable guard, both
    // state reports, the drop, the state-intact check, and the post-drop recovery have all happened.
    wait(&mut s, "KIVORI-EXT ALL PASS state-cycle");
    s
}

/// Minimal external receive/transmit smoke test.
pub fn serial_smoke() -> String {
    let mut s = String::new();
    header(
        &mut s,
        "kivori-serial-smoke",
        "Smallest external check: the firmware announces readiness, receives one known real frame over\n\
         USB Serial/JTAG, and the response path produces a marker.",
    );
    wait(&mut s, "KIVORI-EXT READY iface=usb-serial-jtag");
    write_serial(&mut s, "ping");
    wait(&mut s, "KIVORI-EXT RX kind=Ping");
    let _ = writeln!(
        s,
        "  - wait-serial: 'KIVORI-EXT TX kind=Pong echo={PING_T_MS}'"
    );
    // No trailing delay: every step awaits an observable marker, so there is no timing guesswork.
    s
}

/// The vector catalogue, for review and debugging (not consumed by the simulator).
pub fn catalogue() -> String {
    let mut s = String::new();
    s.push_str(BANNER);
    let _ = writeln!(
        s,
        "#\n# Canonical wire vectors. `bytes` are decimal octets of the complete framed packet.\n\
         # hello_nonce: {HELLO_NONCE:#010x}   ping_t_ms: {PING_T_MS}"
    );
    let _ = writeln!(s, "vectors:");
    for v in vectors() {
        let _ = writeln!(s, "  - name: {}", v.name);
        let _ = writeln!(s, "    purpose: '{}'", v.purpose);
        let _ = writeln!(s, "    len: {}", v.bytes.len());
        let _ = writeln!(s, "    bytes: {}", yaml_bytes(&v.bytes));
    }
    s
}

/// Markers the firmware emits exactly ONCE, at boot, before any injected frame is processed.
///
/// A scenario may only wait on one of these BEFORE its first `write-serial` step. Waiting on one
/// afterwards can never succeed — `wait-serial` scans forward from the current stream position, so a
/// one-shot boot marker has already scrolled past. That mistake caused the original
/// `state-cycle-serial` 30-second timeout, and [`boot_only_markers_are_not_awaited_late`] guards it.
pub const BOOT_ONLY_MARKERS: &[&str] = &[
    "READY",
    "SENDABLE-GUARD",
    "HEALTH free_bytes",
    // Production-runtime markers the firmware emits exactly once, before any injection:
    "KIVORI-RUN BOOT",
    "lifecycle-booting-to-offline",
    "first-frame-36-tiles",
    "unchanged-frame-no-reflush",
    "health-report",
];

/// PRODUCTION-RUNTIME scenario: real frames into the genuine `runtime::run` loop (T074).
///
/// Unlike the external-serial scenarios, the firmware under test here is not a harness — it is the same
/// run loop the physical binary calls, wired to real peripherals through the simulation-only board profile.
/// Markers are emitted from observed `Tick` values, so each one reports work that actually happened.
pub fn production_runtime() -> String {
    let mut s = String::new();
    header(
        &mut s,
        "kivori-production-runtime",
        "PRODUCTION runtime under simulation: `runtime::run` (T074) driving the real USB Serial/JTAG\n\
         transport and the real T072 SPI DisplaySink, with the simulation-only board profile.\n\
         Proves the device-owned booting->offline transition, the first full frame, change-driven refresh,\n\
         health cadence, handshake, state commands, malformed-frame survival with a SAFE diagnostic\n\
         (category+code only), and recovery on the next valid frame.\n\
         SCOPE: nothing here validates the physical panel controller, its init sequence, offsets,\n\
         orientation, colour order, or backlight — those remain unconfirmed hardware facts.\n\
         The run ends on ALL PASS production-runtime, which the firmware emits ONLY once every\n\
         post-condition it observed genuinely held.",
    );
    // Pre-injection: everything the loop does on its own, before a host exists.
    wait(
        &mut s,
        "KIVORI-RUN READY iface=usb-serial-jtag display=generic-spi",
    );
    // ORDER MATTERS: `wait-serial` scans forward only, so these must appear in the order the runtime
    // genuinely emits them. Health goes out on the FIRST tick (next_health_ms starts at 0), before the
    // second tick can observe a quiet frame — pinned by `emission_order_is_stable` in
    // firmware/esp32-c3/tests/production_runtime.rs.
    wait(&mut s, "KIVORI-RUN PASS lifecycle-booting-to-offline");
    wait(&mut s, "KIVORI-RUN PASS first-frame-36-tiles");
    wait(&mut s, "KIVORI-RUN PASS health-report");
    wait(&mut s, "KIVORI-RUN PASS unchanged-frame-no-reflush");
    // Handshake.
    write_serial(&mut s, "hello");
    wait(&mut s, "KIVORI-RUN PASS hello-ack");
    // Desired-state commands, applied and repainted through the shared renderer.
    write_serial(&mut s, "set_state_idle");
    wait(&mut s, "KIVORI-RUN PASS state-idle");
    write_serial(&mut s, "set_state_happy");
    wait(&mut s, "KIVORI-RUN PASS state-happy");
    // A frame the decoder must reject, reported as a safe category+code and nothing more.
    write_serial(&mut s, "malformed_cobs");
    wait(
        &mut s,
        "KIVORI-RUN PASS diagnostic label=frame-rejected-framing",
    );
    // Recovery: the next valid frame is still served.
    write_serial(&mut s, "ping");
    wait(&mut s, "KIVORI-RUN PASS pong");
    wait(&mut s, "KIVORI-RUN PASS recovery-after-reject");
    wait(&mut s, "KIVORI-RUN ALL PASS production-runtime");
    s
}

/// Every generated scenario, as `(name, yaml)`.
#[must_use]
pub fn generated_scenarios() -> Vec<(&'static str, String)> {
    vec![
        ("protocol-serial", protocol_serial()),
        ("state-cycle-serial", state_cycle_serial()),
        ("serial-smoke", serial_smoke()),
        ("production-runtime", production_runtime()),
    ]
}
