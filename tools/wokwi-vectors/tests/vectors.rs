//! Vector-generator tests: determinism, and proof that every vector is a genuine frame produced by the
//! real codec (it must decode back to the intended message) or a deliberate corruption that the real
//! decoder rejects.

use kivori_model::SendableState;
use kivori_protocol::{decode_message, Message, MAX_FRAME, PROTOCOL_MAJOR};
use kivori_wokwi_vectors::{vector, vectors, yaml_bytes, HELLO_NONCE, PING_T_MS};

/// Decodes a complete wire packet (strips the trailing `0x00` delimiter) with the real decoder.
fn decode(bytes: &[u8]) -> Result<Message, kivori_protocol::ProtoError> {
    let packet: Vec<u8> = bytes
        .split(|&b| b == 0)
        .find(|p| !p.is_empty())
        .unwrap_or(&[])
        .to_vec();
    let mut scratch: heapless::Vec<u8, MAX_FRAME> = heapless::Vec::new();
    decode_message(&packet, &mut scratch, &[PROTOCOL_MAJOR]).map(|(_, msg)| msg)
}

#[test]
fn generation_is_deterministic() {
    let a = vectors();
    let b = vectors();
    assert_eq!(a, b, "vectors must be byte-identical across runs");
    // And stable across the whole catalogue, not just per item.
    let flat_a: Vec<u8> = a.iter().flat_map(|v| v.bytes.clone()).collect();
    let flat_b: Vec<u8> = b.iter().flat_map(|v| v.bytes.clone()).collect();
    assert_eq!(flat_a, flat_b);
}

#[test]
fn vector_names_are_unique_and_non_empty() {
    let all = vectors();
    assert!(!all.is_empty());
    let mut names: Vec<&str> = all.iter().map(|v| v.name).collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "vector names must be unique");
    assert!(all.iter().all(|v| !v.bytes.is_empty()), "no empty vector");
}

#[test]
fn valid_vectors_round_trip_through_the_real_decoder() {
    // These are genuine frames: the real decoder must accept them and yield the intended message.
    assert!(matches!(
        decode(&vector("hello").bytes),
        Ok(Message::Hello(h)) if h.nonce == HELLO_NONCE
    ));
    assert!(matches!(
        decode(&vector("ping").bytes),
        Ok(Message::Ping(p)) if p.t_ms == PING_T_MS
    ));
    assert!(matches!(
        decode(&vector("set_state_idle").bytes),
        Ok(Message::SetState(s)) if s.desired == SendableState::Idle
    ));
    assert!(matches!(
        decode(&vector("set_state_happy").bytes),
        Ok(Message::SetState(s)) if s.desired == SendableState::Happy
    ));
    // A higher minor at the same major is still accepted (FR-003).
    assert!(matches!(
        decode(&vector("hello_compatible_minor").bytes),
        Ok(Message::Hello(_))
    ));
}

#[test]
fn incompatible_major_is_rejected_by_the_real_decoder() {
    assert!(
        decode(&vector("hello_incompatible_major").bytes).is_err(),
        "an unsupported major must not decode under the supported-majors policy"
    );
}

#[test]
fn corrupted_vectors_are_rejected_by_the_real_decoder() {
    for name in ["ping_crc_invalid", "malformed_cobs", "truncated_frame"] {
        assert!(
            decode(&vector(name).bytes).is_err(),
            "{name} must be rejected by the real decoder"
        );
    }
}

#[test]
fn sequence_vectors_carry_the_intended_numbers() {
    // Sequence numbers live in the header, so decode the header too.
    let seq_of = |name: &str| -> u16 {
        let bytes = vector(name).bytes;
        let packet: Vec<u8> = bytes
            .split(|&b| b == 0)
            .find(|p| !p.is_empty())
            .unwrap()
            .to_vec();
        let mut scratch: heapless::Vec<u8, MAX_FRAME> = heapless::Vec::new();
        decode_message(&packet, &mut scratch, &[PROTOCOL_MAJOR])
            .map(|(h, _)| h.seq)
            .expect("valid frame")
    };
    assert_eq!(seq_of("ping"), 1);
    assert_eq!(seq_of("ping_seq_duplicate"), 1, "duplicate reuses the seq");
    assert_eq!(seq_of("ping_seq_gap"), 900);
    assert_eq!(seq_of("ping_seq_wrap_max"), u16::MAX);
    assert_eq!(seq_of("ping_seq_wrap_zero"), 0);
}

#[test]
fn yaml_byte_formatting_is_a_valid_inline_sequence() {
    assert_eq!(yaml_bytes(&[0, 1, 255]), "[0, 1, 255]");
    let v = vector("ping");
    let rendered = yaml_bytes(&v.bytes);
    assert!(rendered.starts_with('[') && rendered.ends_with(']'));
    // Every element must be a plain decimal octet — Wokwi rejects anything else.
    let inner = rendered.trim_start_matches('[').trim_end_matches(']');
    for item in inner.split(", ") {
        let n: u16 = item.parse().expect("decimal octet");
        assert!(n <= 255);
    }
}

// ── Scenario-shape invariants ────────────────────────────────────────────────────────────────────
//
// These guard the class of bug that made `state-cycle-serial` time out for 30 s: the scenario waited on
// `KIVORI-EXT SENDABLE-GUARD ok`, a marker the firmware emits once at boot, AFTER the injection steps —
// so `wait-serial`, which only scans forward, could never match it again.

/// Splits a scenario into its ordered `(kind, payload)` steps, where kind is `wait` or `write`.
fn steps(yaml: &str) -> Vec<(&'static str, String)> {
    yaml.lines()
        .filter_map(|line| {
            let t = line.trim();
            t.strip_prefix("- wait-serial: ")
                .map(|rest| ("wait", rest.trim_matches('\'').to_string()))
                .or_else(|| {
                    t.strip_prefix("- write-serial: ")
                        .map(|_| ("write", String::new()))
                })
        })
        .collect()
}

#[test]
fn boot_only_markers_are_not_awaited_late() {
    for (name, yaml) in kivori_wokwi_vectors::generated_scenarios() {
        let mut injected = false;
        for (kind, payload) in steps(&yaml) {
            if kind == "write" {
                injected = true;
                continue;
            }
            if injected {
                for boot_marker in kivori_wokwi_vectors::BOOT_ONLY_MARKERS {
                    assert!(
                        !payload.contains(boot_marker),
                        "scenario `{name}` waits on boot-only marker `{boot_marker}` after injection \
                         has started; wait-serial scans forward only, so it can never match. \
                         Offending step: `{payload}`"
                    );
                }
            }
        }
    }
}

#[test]
fn multi_step_scenarios_end_on_an_explicit_completion_marker() {
    // A scenario that injects frames must finish on a marker the firmware emits only after verifying
    // its post-conditions — otherwise simulator exit alone could look like success.
    for (name, yaml) in kivori_wokwi_vectors::generated_scenarios() {
        let all = steps(&yaml);
        let last_wait = all
            .iter()
            .rev()
            .find(|(k, _)| *k == "wait")
            .map(|(_, p)| p.clone())
            .unwrap_or_default();
        assert!(
            last_wait.contains("ALL PASS")
                || last_wait.contains("SEQ seq=")
                || last_wait.contains("TX kind=Pong echo="),
            "scenario `{name}` must end on an explicit verified-outcome marker, got `{last_wait}`"
        );
    }
}

#[test]
fn every_scenario_starts_by_awaiting_readiness() {
    // Injecting before the firmware is listening would race the boot sequence.
    for (name, yaml) in kivori_wokwi_vectors::generated_scenarios() {
        let first = steps(&yaml).into_iter().next().expect("at least one step");
        assert_eq!(
            first.0, "wait",
            "scenario `{name}` must wait before injecting"
        );
        assert!(
            first.1.contains("READY"),
            "scenario `{name}` must wait for the readiness marker first, got `{}`",
            first.1
        );
    }
}

#[test]
fn state_cycle_scenario_covers_every_required_post_condition() {
    // The scenario must genuinely assert each contract point, not just the happy path.
    let yaml = kivori_wokwi_vectors::state_cycle_serial();
    for required in [
        "KIVORI-EXT RX kind=Hello",
        "KIVORI-EXT TX kind=HelloAck nonce=ok",
        "KIVORI-EXT RX kind=SetState",
        "KIVORI-EXT TX kind=StateReport reported=idle",
        "KIVORI-EXT STATE current=idle",
        "KIVORI-EXT TX kind=StateReport reported=happy",
        "KIVORI-EXT STATE current=happy",
        "KIVORI-EXT DROP reason=decode",
        "KIVORI-EXT TX kind=Pong",
        "KIVORI-EXT ALL PASS state-cycle",
    ] {
        assert!(
            yaml.contains(required),
            "state-cycle-serial must assert `{required}`"
        );
    }
    // The completion marker must be the final assertion.
    let last = steps(&yaml)
        .into_iter()
        .rfind(|(k, _)| *k == "wait")
        .map(|(_, p)| p)
        .unwrap_or_default();
    assert_eq!(last, "KIVORI-EXT ALL PASS state-cycle");
}

#[test]
fn the_late_boot_marker_detector_is_not_vacuous() {
    // Negative control: this is the ORIGINAL buggy shape that timed out — a boot-only marker awaited
    // after injection. The same rule the real scenarios are checked against must reject it.
    let buggy = "\
name: buggy
version: 1
author: t
steps:
  - wait-serial: 'KIVORI-EXT READY iface=usb-serial-jtag'
  - write-serial: [1, 2, 3]
  - wait-serial: 'KIVORI-EXT TX kind=StateReport reported=happy'
  - wait-serial: 'KIVORI-EXT SENDABLE-GUARD ok'
";
    let mut injected = false;
    let mut caught = false;
    for (kind, payload) in steps(buggy) {
        if kind == "write" {
            injected = true;
            continue;
        }
        if injected
            && kivori_wokwi_vectors::BOOT_ONLY_MARKERS
                .iter()
                .any(|m| payload.contains(m))
        {
            caught = true;
        }
    }
    assert!(
        caught,
        "the detector must reject a boot-only marker awaited after injection"
    );
}

#[test]
fn serial_smoke_establishes_a_session_before_requesting_a_pong() {
    let yaml = kivori_wokwi_vectors::serial_smoke();
    let hello = format!("- write-serial: {}", yaml_bytes(&vector("hello").bytes));
    let ping = format!("- write-serial: {}", yaml_bytes(&vector("ping").bytes));
    let hello_at = yaml.find(&hello).expect("Hello opens the session");
    let ack_at = yaml
        .find("KIVORI-EXT TX kind=HelloAck nonce=ok")
        .expect("await accepted session");
    let ping_at = yaml.find(&ping).expect("Ping probes the accepted session");
    assert!(hello_at < ack_at && ack_at < ping_at);
}
