//! Host tests for the production runtime (T074).
//!
//! These drive the *same* [`Runtime`] the device binary runs, through the host sim adapters. Every
//! behaviour the firmware promises — the device-owned boot transition, handshake, heartbeat, state
//! commands, change-driven tile output, malformed-frame survival, safe diagnostics, and health cadence —
//! is asserted here, with no hardware and no simulator.
//!
//! Nothing here validates a physical panel: `CaptureDisplay` is a buffer, not glass.
#![cfg(feature = "host-sim")]

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::{Clock, InputSource};
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::render::{TILE_COLS, TILE_COUNT};
use kivori_firmware::runtime::{Runtime, RuntimeConfig, Tick};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_model::input::InputLevels;
use kivori_model::{
    Capabilities, CompanionState, MascotAction, MascotPersonality, ProtocolVersion, SendableState,
};
use kivori_protocol::{
    decode_message, encode_message, Bye, ByeReason, ErrorCategory, FirmwareVersion, Hello, Message,
    Ping, PlayMascotAction, Ready, SetState, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: [0xAB; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: Capabilities::MASCOT_INTERACTION,
    }
}

fn wire_version() -> ProtocolVersion {
    ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
}

/// Host → device: frames `msg` with sequence `seq` onto the pipe.
fn host_write(pipe: &mut SimPipe, msg: &Message, seq: u16) {
    let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
    encode_message(msg, wire_version(), seq, &mut wire).expect("encode");
    pipe.host_send(&wire).expect("pipe has capacity");
}

/// Decodes every complete device→host frame currently queued.
fn host_drain(pipe: &mut SimPipe) -> Vec<Message> {
    let bytes = pipe.host_recv();
    let mut messages = Vec::new();
    let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
    for packet in bytes.split(|&b| b == 0) {
        if packet.is_empty() {
            continue;
        }
        if let Ok((_, msg)) = decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
            messages.push(msg);
        }
    }
    messages
}

/// A stub input source: this suite exercises the protocol/lifecycle/render loop, not physical
/// input, so it always reports no motion (Task 4 wires the port; Task 13 replaces it on device).
struct NoInput;

impl InputSource for NoInput {
    fn sample(&mut self) -> InputLevels {
        InputLevels {
            a: false,
            b: false,
            sw: false,
        }
    }
}

/// The whole test rig: runtime plus its four ports and the compiled asset blob.
struct Harness {
    runtime: Runtime<'static>,
    clock: VirtualClock,
    pipe: SimPipe,
    input: NoInput,
    display: Box<CaptureDisplay>,
    blob_bytes: Vec<u8>,
}

impl Harness {
    fn new() -> Self {
        Self {
            runtime: Runtime::new(identity(), RuntimeConfig::default()),
            clock: VirtualClock::new(),
            pipe: SimPipe::new(),
            input: NoInput,
            display: Box::new(CaptureDisplay::new()),
            blob_bytes: compile_default_blob(),
        }
    }

    /// One production tick at the current virtual time.
    fn step(&mut self) -> Tick {
        let blob = AssetBlob::parse(&self.blob_bytes).expect("valid blob");
        self.runtime.step(
            &self.clock,
            &mut self.pipe,
            &mut self.input,
            self.display.as_mut(),
            &blob,
        )
    }

    /// Advances the clock past the frame interval and ticks.
    fn tick_next_frame(&mut self) -> Tick {
        self.clock
            .advance(RuntimeConfig::default().frame_interval_ms);
        self.step()
    }
}

#[test]
fn boot_transitions_to_offline_and_paints_the_first_frame() {
    let mut h = Harness::new();
    let tick = h.step();

    // The device owns `booting` at power-on and falls back to `offline` with no host (FR-014/015).
    assert_eq!(tick.state, Some(CompanionState::Offline));
    assert_eq!(h.runtime.state(), CompanionState::Offline);
    assert_eq!(
        tick.tiles_flushed as usize, TILE_COUNT,
        "first frame flushes every tile"
    );
    assert_eq!(h.display.blits as usize, TILE_COUNT);
}

#[test]
fn a_tick_before_the_next_frame_flushes_nothing() {
    let mut h = Harness::new();
    h.step();
    let quiet = h.step();
    assert_eq!(
        quiet.tiles_flushed, 0,
        "no tile changed, so nothing may reach the panel (FR-013)"
    );
}

#[test]
fn a_late_frame_keeps_the_original_cadence_without_queuing_stale_frames() {
    let mut h = Harness::new();
    h.step();

    h.clock.advance(50);
    assert!(h.step().frame_rendered, "the overdue frame renders once");

    h.clock.advance(15);
    assert!(
        !h.step().frame_rendered,
        "the next anchored deadline is 66 ms"
    );

    h.clock.advance(1);
    assert!(h.step().frame_rendered, "the 66 ms deadline is preserved");
}

#[test]
fn the_handshake_is_answered_with_identity_and_nonce_echo() {
    let mut h = Harness::new();
    h.step();
    host_write(
        &mut h.pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::NONE,
            nonce: 0xDEAD_BEEF,
        }),
        0,
    );
    h.tick_next_frame();

    let replies = host_drain(&mut h.pipe);
    let ack = replies
        .iter()
        .find_map(|m| match m {
            Message::HelloAck(ack) => Some(ack),
            _ => None,
        })
        .expect("HelloAck");
    assert_eq!(ack.nonce_echo, 0xDEAD_BEEF);
    assert_eq!(ack.device_id, identity().device_id);
}

#[test]
fn ping_is_answered_with_pong_echoing_the_timestamp() {
    let mut h = Harness::new();
    h.step();
    host_write(&mut h.pipe, &Message::Ping(Ping { t_ms: 4242 }), 0);
    h.tick_next_frame();

    let pong = host_drain(&mut h.pipe)
        .into_iter()
        .find_map(|m| match m {
            Message::Pong(p) => Some(p),
            _ => None,
        })
        .expect("Pong");
    assert_eq!(pong.t_ms_echo, 4242);
}

#[test]
fn set_state_is_applied_reported_and_repainted() {
    let mut h = Harness::new();
    h.step();
    for (seq, desired, expected) in [
        (0u16, SendableState::Idle, CompanionState::Idle),
        (1, SendableState::Happy, CompanionState::Happy),
    ] {
        host_write(
            &mut h.pipe,
            &Message::SetState(SetState {
                desired,
                at_ms: None,
            }),
            seq,
        );
        let tick = h.tick_next_frame();

        assert_eq!(h.runtime.state(), expected);
        assert!(
            tick.tiles_flushed as usize <= TILE_COUNT - TILE_COLS,
            "the unchanged top background row is not retransmitted"
        );
        let report = host_drain(&mut h.pipe)
            .into_iter()
            .find_map(|m| match m {
                Message::StateReport(r) => Some(r),
                _ => None,
            })
            .expect("StateReport");
        assert_eq!(report.reported, expected);
        let initial = h.display.frame().to_vec();
        h.clock.advance(600);
        h.step();
        assert_ne!(
            h.display.frame(),
            initial.as_slice(),
            "the transition advances after the immediate state report"
        );
    }
}

#[test]
fn social_action_changes_pixels_without_changing_semantic_state_and_is_acknowledged() {
    let mut h = Harness::new();
    h.step();
    host_write(
        &mut h.pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::MASCOT_INTERACTION,
            nonce: 1,
        }),
        0,
    );
    h.tick_next_frame();
    let _ = host_drain(&mut h.pipe);
    host_write(
        &mut h.pipe,
        &Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: Capabilities::MASCOT_INTERACTION,
        }),
        1,
    );
    h.tick_next_frame();
    host_write(
        &mut h.pipe,
        &Message::SetState(SetState {
            desired: SendableState::Idle,
            at_ms: None,
        }),
        2,
    );
    h.tick_next_frame();
    let _ = host_drain(&mut h.pipe);
    h.clock.advance(600);
    h.step();
    let before = h.display.frame().to_vec();

    host_write(
        &mut h.pipe,
        &Message::PlayMascotAction(PlayMascotAction {
            action: MascotAction::Tickle,
            personality: MascotPersonality::Playful,
            seed: 23,
        }),
        3,
    );
    let first_tick = h.tick_next_frame();
    let applied_at_ms = h.clock.now_ms();
    assert_eq!(
        first_tick.tiles_flushed, 0,
        "action begins from the current rendered pose"
    );
    let tick = h.tick_next_frame();

    assert_eq!(h.runtime.state(), CompanionState::Idle);
    assert!(tick.tiles_flushed > 0);
    assert_ne!(h.display.frame(), before.as_slice());
    let applied = host_drain(&mut h.pipe)
        .into_iter()
        .find_map(|message| match message {
            Message::MascotActionApplied(applied) => Some(applied),
            _ => None,
        })
        .expect("MascotActionApplied");
    assert_eq!(applied.action, MascotAction::Tickle);
    assert_eq!(applied.personality, MascotPersonality::Playful);
    assert_eq!(applied.seed, 23);
    assert_eq!(applied.applied_at_ms, applied_at_ms);
}

#[test]
fn social_actions_require_ready_and_the_negotiated_capability() {
    let action = Message::PlayMascotAction(PlayMascotAction {
        action: MascotAction::Pet,
        personality: MascotPersonality::Cozy,
        seed: 9,
    });
    let mut h = Harness::new();
    h.step();
    host_write(
        &mut h.pipe,
        &Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: Capabilities::MASCOT_INTERACTION,
        }),
        0,
    );
    h.tick_next_frame();
    host_write(&mut h.pipe, &action, 1);
    h.tick_next_frame();
    assert!(!host_drain(&mut h.pipe)
        .iter()
        .any(|m| matches!(m, Message::MascotActionApplied(_))));

    host_write(&mut h.pipe, &action, 0);
    h.tick_next_frame();
    assert!(!host_drain(&mut h.pipe)
        .iter()
        .any(|m| matches!(m, Message::MascotActionApplied(_))));

    host_write(
        &mut h.pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::NONE,
            nonce: 2,
        }),
        2,
    );
    h.tick_next_frame();
    let _ = host_drain(&mut h.pipe);
    host_write(
        &mut h.pipe,
        &Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: Capabilities::MASCOT_INTERACTION,
        }),
        3,
    );
    h.tick_next_frame();
    host_write(&mut h.pipe, &action, 4);
    h.tick_next_frame();
    assert!(!host_drain(&mut h.pipe)
        .iter()
        .any(|m| matches!(m, Message::MascotActionApplied(_))));
}

#[test]
fn a_malformed_frame_is_dropped_reported_safely_and_survived() {
    let mut h = Harness::new();
    h.step();

    // Garbage that cannot decode, terminated so the framer sees a complete packet.
    h.pipe.host_send(&[0x02, 0xFF, 0xFF, 0x00]).expect("send");
    let tick = h.tick_next_frame();

    assert_eq!(h.runtime.rejected_frames(), 1, "the frame was rejected");
    assert_eq!(tick.rejected_frames, 1);
    let diagnostic = host_drain(&mut h.pipe)
        .into_iter()
        .find_map(|m| match m {
            Message::Diagnostic(d) => Some(d),
            _ => None,
        })
        .expect("a safe Diagnostic was emitted");
    // Category + code only — the wire type has no field that could carry the offending bytes.
    assert!(matches!(
        diagnostic.category,
        ErrorCategory::Framing | ErrorCategory::Checksum | ErrorCategory::BadPayload
    ));
    assert!(diagnostic.code > 0);

    // And the very next valid frame still works: no resync deadlock (SC-008).
    host_write(&mut h.pipe, &Message::Ping(Ping { t_ms: 7 }), 0);
    h.tick_next_frame();
    assert!(
        host_drain(&mut h.pipe)
            .iter()
            .any(|m| matches!(m, Message::Pong(_))),
        "the loop recovered and answered the next valid frame"
    );
}

#[test]
fn a_duplicate_sequence_does_not_re_apply_side_effects() {
    let mut h = Harness::new();
    h.step();
    host_write(
        &mut h.pipe,
        &Message::SetState(SetState {
            desired: SendableState::Busy,
            at_ms: None,
        }),
        5,
    );
    h.tick_next_frame();
    let _ = host_drain(&mut h.pipe);

    // Same sequence number again: the state is already Busy, so no second StateReport may appear.
    host_write(
        &mut h.pipe,
        &Message::SetState(SetState {
            desired: SendableState::Idle,
            at_ms: None,
        }),
        5,
    );
    h.tick_next_frame();
    assert_eq!(
        h.runtime.state(),
        CompanionState::Busy,
        "a duplicate must not change state"
    );
    assert!(
        !host_drain(&mut h.pipe)
            .iter()
            .any(|m| matches!(m, Message::StateReport(_))),
        "a duplicate must not re-report"
    );
}

#[test]
fn bye_drops_the_link_to_offline_and_reports_it() {
    let mut h = Harness::new();
    h.step();
    host_write(
        &mut h.pipe,
        &Message::SetState(SetState {
            desired: SendableState::Happy,
            at_ms: None,
        }),
        0,
    );
    h.tick_next_frame();
    let _ = host_drain(&mut h.pipe);

    host_write(
        &mut h.pipe,
        &Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        1,
    );
    h.tick_next_frame();

    assert_eq!(h.runtime.state(), CompanionState::Offline);
    let diagnostics: Vec<_> = host_drain(&mut h.pipe)
        .into_iter()
        .filter_map(|m| match m {
            Message::Diagnostic(d) => Some(d),
            _ => None,
        })
        .collect();
    assert!(
        diagnostics.iter().any(|d| d.category == ErrorCategory::Io),
        "the dropped link is reported as a safe Io diagnostic"
    );
}

#[test]
fn health_is_emitted_on_its_own_cadence_not_every_tick() {
    let mut h = Harness::new();
    let first = h.step();
    assert!(first.health_sent, "a report goes out at boot");

    // Well inside the interval: no second report.
    let soon = h.tick_next_frame();
    assert!(!soon.health_sent, "health must not fire every frame");

    h.clock.advance(RuntimeConfig::default().health_interval_ms);
    let later = h.step();
    assert!(later.health_sent, "the next interval fires one report");

    assert!(
        host_drain(&mut h.pipe)
            .iter()
            .any(|m| matches!(m, Message::Health(_))),
        "Health reached the host"
    );
}

#[test]
fn the_loop_keeps_running_across_many_ticks() {
    // A long run must not wedge, wrap the clock into the past, or stop flushing on state changes.
    let mut h = Harness::new();
    h.step();
    for round in 0..40u16 {
        if round % 10 == 0 {
            let desired = if round % 20 == 0 {
                SendableState::Idle
            } else {
                SendableState::Happy
            };
            host_write(
                &mut h.pipe,
                &Message::SetState(SetState {
                    desired,
                    at_ms: None,
                }),
                round,
            );
        }
        h.tick_next_frame();
        let _ = host_drain(&mut h.pipe);
    }
    assert!(matches!(
        h.runtime.state(),
        CompanionState::Idle | CompanionState::Happy
    ));
    assert_eq!(
        h.runtime.rejected_frames(),
        0,
        "no valid frame was rejected"
    );
}

#[test]
fn emission_order_is_stable() {
    // The Wokwi production-runtime scenario awaits markers with `wait-serial`, which scans FORWARD ONLY —
    // so the scenario's order must match the order the runtime genuinely emits its post-conditions. This
    // test pins that order, because getting it wrong is exactly the T135 defect (a marker that already
    // scrolled past can never match again, and the scenario hangs until timeout).
    let mut h = Harness::new();

    let first = h.step();
    assert_eq!(
        first.state,
        Some(CompanionState::Offline),
        "tick 1: booting -> offline"
    );
    assert_eq!(
        first.tiles_flushed as usize, TILE_COUNT,
        "tick 1: the first full frame"
    );
    assert!(
        first.health_sent,
        "tick 1: health goes out on the first tick, BEFORE any quiet frame can be observed"
    );

    let second = h.step();
    assert_eq!(second.tiles_flushed, 0, "tick 2: no new frame is due yet");
    assert!(
        !second.health_sent,
        "tick 2: health must not repeat inside its interval"
    );

    // Therefore the scenario order is: lifecycle -> first-frame -> health-report -> unchanged-frame.
    // If a future change moves health off the first tick, this test fails before the simulator does.
}

/// PRD §9.5: a reaction never masks Busy, and a refused reaction is never acknowledged.
#[test]
fn social_action_over_busy_is_refused_unacknowledged_and_draws_nothing() {
    let mut h = Harness::new();
    h.step();
    host_write(
        &mut h.pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::MASCOT_INTERACTION,
            nonce: 1,
        }),
        0,
    );
    h.tick_next_frame();
    let _ = host_drain(&mut h.pipe);
    host_write(
        &mut h.pipe,
        &Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: Capabilities::MASCOT_INTERACTION,
        }),
        1,
    );
    h.tick_next_frame();
    host_write(
        &mut h.pipe,
        &Message::SetState(SetState {
            desired: SendableState::Busy,
            at_ms: None,
        }),
        2,
    );
    h.tick_next_frame();
    let _ = host_drain(&mut h.pipe);
    // Let the Busy transition settle so any later pixel change could only come from the reaction.
    h.clock.advance(1_000);
    h.tick_next_frame();

    host_write(
        &mut h.pipe,
        &Message::PlayMascotAction(PlayMascotAction {
            action: MascotAction::Greet,
            personality: MascotPersonality::Playful,
            seed: 5,
        }),
        3,
    );
    h.tick_next_frame();
    h.tick_next_frame();

    assert_eq!(h.runtime.state(), CompanionState::Busy);
    assert!(!host_drain(&mut h.pipe)
        .iter()
        .any(|m| matches!(m, Message::MascotActionApplied(_))));
}
