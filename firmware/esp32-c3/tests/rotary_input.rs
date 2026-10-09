#![cfg(feature = "host-sim")]

use kivori_firmware::input::quadrature::QuadratureDecoder;
use kivori_model::input::Direction;

/// Drive the decoder through a sequence of (a, b) levels, collecting emitted detents.
fn drive(seq: &[(bool, bool)]) -> Vec<Direction> {
    let mut d = QuadratureDecoder::new();
    let mut out = Vec::new();
    for &(a, b) in seq {
        if let Some(dir) = d.update(a, b) {
            out.push(dir);
        }
    }
    out
}

/// One full clockwise detent is the four-phase cycle 00 -> 01 -> 11 -> 10 -> 00.
const CW_CYCLE: [(bool, bool); 5] = [
    (false, false),
    (false, true),
    (true, true),
    (true, false),
    (false, false),
];

/// Counter-clockwise is the same cycle traversed in reverse.
const CCW_CYCLE: [(bool, bool); 5] = [
    (false, false),
    (true, false),
    (true, true),
    (false, true),
    (false, false),
];

#[test]
fn full_clockwise_cycle_emits_exactly_one_cw_detent() {
    assert_eq!(drive(&CW_CYCLE), vec![Direction::Cw]);
}

#[test]
fn full_counter_clockwise_cycle_emits_exactly_one_ccw_detent() {
    assert_eq!(drive(&CCW_CYCLE), vec![Direction::Ccw]);
}

#[test]
fn partial_motion_that_returns_emits_no_detent() {
    // Bounce walks to an adjacent Gray state and comes back without completing a detent.
    let seq = [
        (false, false),
        (false, true),
        (false, false),
        (false, true),
        (false, false),
    ];
    assert_eq!(drive(&seq), vec![]);
}

#[test]
fn repeated_identical_samples_emit_nothing() {
    let seq = [(false, false); 8];
    assert_eq!(drive(&seq), vec![]);
}

#[test]
fn illegal_double_bit_transition_is_counted_and_emits_no_detent() {
    let mut d = QuadratureDecoder::new();
    assert_eq!(d.update(false, false), None);
    // 00 -> 11 changes both bits at once: electrically impossible for a real detent.
    assert_eq!(d.update(true, true), None);
    assert_eq!(d.invalid_transitions(), 1);
}

#[test]
fn three_consecutive_cw_cycles_emit_three_cw_detents() {
    let mut seq = Vec::new();
    for _ in 0..3 {
        seq.extend_from_slice(&CW_CYCLE[1..]);
    }
    let mut full = vec![(false, false)];
    full.extend(seq);
    assert_eq!(
        drive(&full),
        vec![Direction::Cw, Direction::Cw, Direction::Cw]
    );
}

#[test]
fn reversal_mid_cycle_does_not_emit_a_detent() {
    // Advance two quarter-steps clockwise, then retreat to the start. No detent completed.
    let seq = [
        (false, false),
        (false, true),
        (true, true),
        (false, true),
        (false, false),
    ];
    assert_eq!(drive(&seq), vec![]);
}

#[test]
fn invalid_transition_discards_quarter_steps_banked_before_it() {
    // 00 -> 01 -> 11 banks two clockwise quarter-steps, then 11 -> 00 is an illegal
    // double-bit transition. Two more clockwise quarter-steps follow. If the banked
    // steps survived the discontinuity they would splice with the steps after it
    // (2 + 2 = 4) and wrongly complete a detent; product invariant 42 requires
    // that a detent only ever be emitted for a continuous, fully observed traversal.
    let mut d = QuadratureDecoder::new();
    let mut out = Vec::new();
    for &(a, b) in &[
        (false, false),
        (false, true),
        (true, true),
        (false, false), // illegal: 11 -> 00, both bits change
        (false, true),
        (true, true),
    ] {
        if let Some(dir) = d.update(a, b) {
            out.push(dir);
        }
    }
    assert_eq!(out, vec![]);
    assert_eq!(d.invalid_transitions(), 1);
}

#[test]
fn detent_completes_normally_immediately_after_an_invalid_transition() {
    // After the same illegal 11 -> 00 discontinuity used above, a full, continuous
    // clockwise cycle should still complete cleanly — an invalid transition must not
    // wedge the decoder against ever emitting again.
    let seq = [
        (false, false),
        (false, true),
        (true, true),
        (false, false), // illegal: 11 -> 00, both bits change
        (false, true),
        (true, true),
        (true, false),
        (false, false),
    ];
    assert_eq!(drive(&seq), vec![Direction::Cw]);
}

#[test]
fn a_count_restarted_mid_cycle_never_completes_on_a_bounce_at_rest() {
    // A missed state (10 -> 01) restarts the count off-rest; the rest of that cycle
    // (01 -> 11 -> 10 -> 00) is only three quarter-steps, so arriving at rest emits nothing,
    // and a bounce at rest afterwards must not complete a phantom detent (invariant 46).
    let seq = [
        (false, false),
        (false, true),
        (true, true),
        (true, false),
        (false, true), // illegal: 10 -> 01, both bits change
        (true, true),
        (true, false),
        (false, false), // rest
        (false, true),  // bounce
        (false, false),
    ];
    assert_eq!(drive(&seq), vec![]);
}

#[test]
fn a_reset_off_rest_never_completes_on_a_bounce_at_rest() {
    let mut d = QuadratureDecoder::new();
    let mut out = Vec::new();
    // First sample after a reset lands mid-cycle at 01, then the knob settles and bounces.
    for &(a, b) in &[
        (false, true),
        (true, true),
        (true, false),
        (false, false),
        (false, true),
        (false, false),
    ] {
        out.extend(d.update(a, b));
    }
    assert_eq!(out, vec![]);
}

#[test]
fn reset_drops_both_phase_and_accumulator() {
    // Bank three of the four quarter-steps of a clockwise detent, then reset. If
    // `reset` failed to clear the accumulator, the very next quarter-step after the
    // reset would splice with the stale count and complete a detent one step early.
    let mut d = QuadratureDecoder::new();
    let mut out = Vec::new();

    for &(a, b) in &[
        (false, false), // establish phase 00
        (false, true),  // 01, acc = 1
        (true, true),   // 11, acc = 2
        (true, false),  // 10, acc = 3 (one shy of a detent)
    ] {
        if let Some(dir) = d.update(a, b) {
            out.push(dir);
        }
    }

    d.reset();

    // Re-establishes phase (must not be treated as a transition from the pre-reset
    // phase), then one more quarter-step. If the accumulator had leaked across reset
    // (stale 3 + 1), this would wrongly complete a detent here already.
    for &(a, b) in &[(false, false), (false, true)] {
        if let Some(dir) = d.update(a, b) {
            out.push(dir);
        }
    }
    assert_eq!(out, vec![], "reset must have dropped the stale accumulator");

    // The decoder must not be wedged: a genuine full cycle from here still completes.
    for &(a, b) in &[(true, true), (true, false), (false, false)] {
        if let Some(dir) = d.update(a, b) {
            out.push(dir);
        }
    }
    assert_eq!(out, vec![Direction::Cw]);
}

use kivori_firmware::input::gesture::{RotaryEvent, RotaryGesture};

const GESTURE_END_MS: u32 = 250;

#[test]
fn first_detent_opens_a_gesture_and_reports_the_detent() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    let (started, detent) = g.on_detent(Direction::Cw, 1_000);
    assert_eq!(started, Some(RotaryEvent::GestureStarted { gesture_id: 1 }));
    assert_eq!(
        detent,
        RotaryEvent::Detent {
            gesture_id: 1,
            direction: Direction::Cw
        }
    );
}

#[test]
fn detents_inside_the_window_stay_in_one_gesture() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    let (started, _) = g.on_detent(Direction::Cw, 1_000);
    assert!(started.is_some());

    // 249 ms later: still the same gesture, so no new GestureStarted.
    let (started, detent) = g.on_detent(Direction::Cw, 1_249);
    assert_eq!(started, None);
    assert_eq!(
        detent,
        RotaryEvent::Detent {
            gesture_id: 1,
            direction: Direction::Cw
        }
    );
}

#[test]
fn gesture_ends_after_the_inactivity_window() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    g.on_detent(Direction::Cw, 1_000);

    assert_eq!(
        g.poll(1_249),
        None,
        "must not end before the window elapses"
    );
    assert_eq!(
        g.poll(1_250),
        Some(RotaryEvent::GestureEnded { gesture_id: 1 })
    );
    assert_eq!(g.poll(1_500), None, "GestureEnded is emitted exactly once");
}

#[test]
fn a_detent_after_the_window_opens_a_new_gesture_id() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    g.on_detent(Direction::Cw, 1_000);
    assert_eq!(
        g.poll(1_250),
        Some(RotaryEvent::GestureEnded { gesture_id: 1 })
    );

    let (started, detent) = g.on_detent(Direction::Ccw, 2_000);
    assert_eq!(started, Some(RotaryEvent::GestureStarted { gesture_id: 2 }));
    assert_eq!(
        detent,
        RotaryEvent::Detent {
            gesture_id: 2,
            direction: Direction::Ccw
        }
    );
}

#[test]
fn reversal_within_a_gesture_does_not_split_the_gesture() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    g.on_detent(Direction::Cw, 1_000);
    let (started, detent) = g.on_detent(Direction::Ccw, 1_100);
    assert_eq!(started, None, "reversal is not a new gesture");
    assert_eq!(
        detent,
        RotaryEvent::Detent {
            gesture_id: 1,
            direction: Direction::Ccw
        }
    );
}

#[test]
fn reset_closes_the_gesture_silently_and_restarts_numbering() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    g.on_detent(Direction::Cw, 1_000);
    g.reset();
    assert_eq!(g.poll(5_000), None, "a reset gesture emits no GestureEnded");

    let (started, _) = g.on_detent(Direction::Cw, 6_000);
    assert_eq!(started, Some(RotaryEvent::GestureStarted { gesture_id: 1 }));
}

use heapless::Vec as FixedVec;
use kivori_firmware::sim::{ScriptedInput, SCRIPT_CAPACITY};

/// The sim helpers take fixed-capacity level scripts (the sim module is `no_std` so it stays
/// buildable for the device target); this keeps the scenarios below as readable as `vec![...]`.
fn script<const N: usize>(levels: [InputLevels; N]) -> FixedVec<InputLevels, SCRIPT_CAPACITY> {
    FixedVec::from_slice(&levels).expect("scenario fits SCRIPT_CAPACITY")
}
use kivori_model::input::InputLevels;

fn lv(a: bool, b: bool) -> InputLevels {
    InputLevels {
        a,
        b,
        sw: false,
        keys: [false; 3],
    }
}

#[test]
fn scripted_input_source_replays_levels_then_holds_the_last() {
    use kivori_firmware::ports::InputSource;

    let mut src = ScriptedInput::new(script([lv(false, false), lv(false, true)]));
    assert_eq!(src.sample(), lv(false, false));
    assert_eq!(src.sample(), lv(false, true));
    // Exhausted scripts hold the final level rather than wrapping or panicking.
    assert_eq!(src.sample(), lv(false, true));
}

#[test]
fn a_full_cw_cycle_through_the_port_produces_started_detent_ended() {
    use kivori_firmware::sim::{drive_rotary, SeenInput};

    // Host-sim end-to-end: scripted levels -> validated detent -> emitted event stream.
    let levels = script([
        lv(false, false),
        lv(false, true),
        lv(true, true),
        lv(true, false),
        lv(false, false),
    ]);

    assert_eq!(
        drive_rotary(levels).as_slice(),
        [
            SeenInput::GestureStarted { gesture_id: 1 },
            SeenInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw
            },
            SeenInput::GestureEnded { gesture_id: 1 },
        ]
    );
}

#[test]
fn a_reversal_stays_in_one_gesture_and_reports_both_directions() {
    use kivori_firmware::sim::{drive_rotary, SeenInput};

    let mut levels = script([
        // one CW detent
        lv(false, false),
        lv(false, true),
        lv(true, true),
        lv(true, false),
        lv(false, false),
    ]);
    // then one CCW detent, back the way it came
    levels
        .extend_from_slice(&[
            lv(true, false),
            lv(true, true),
            lv(false, true),
            lv(false, false),
        ])
        .expect("scenario fits SCRIPT_CAPACITY");

    assert_eq!(
        drive_rotary(levels).as_slice(),
        [
            SeenInput::GestureStarted { gesture_id: 1 },
            SeenInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw
            },
            SeenInput::Detent {
                gesture_id: 1,
                direction: Direction::Ccw
            },
            SeenInput::GestureEnded { gesture_id: 1 },
        ]
    );
}

#[test]
fn the_dispatcher_records_the_accepted_session_nonce() {
    // Uses the existing host-sim handshake helper pattern from
    // `firmware/esp32-c3/tests/host_sim.rs`: drive Hello -> HelloAck -> Ready, then assert
    // the dispatcher retained the nonce it accepted.
    let d = kivori_firmware::sim::handshaken_dispatcher(0x1234_5678);
    assert_eq!(d.accepted_session(), Some(0x1234_5678));
}

// --- capability gating (controller decision 1): send_input_event must stay inert unless BOTH a
// session has been accepted AND the capability was negotiated. Each half is proven independently
// so a shortcut implementation (e.g. gating on session alone) cannot pass by accident.

use heapless::Vec as HVec;
use kivori_firmware::proto::{DeviceIdentity, Dispatcher};
use kivori_firmware::sim::{handshaken_dispatcher as handshaken, SimPipe};
use kivori_firmware::state::DeviceState;
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, ControlId, FirmwareVersion, Hello, InputKind, Message, Nonce,
    Ready, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

fn gating_identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: [0xCD; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: Capabilities::PHYSICAL_INPUT_V1,
    }
}

fn gating_wire_version() -> ProtocolVersion {
    ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
}

/// Host -> device: frames `msg` with sequence `seq` onto the pipe (mirrors `tests/host_sim.rs`).
fn gating_host_write(pipe: &mut SimPipe, msg: &Message, seq: u16) {
    let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
    encode_message(msg, gating_wire_version(), seq, &mut wire).expect("encode");
    pipe.host_send(&wire).expect("pipe has capacity");
}

/// Decodes every complete device -> host frame currently queued (mirrors `tests/host_sim.rs`).
fn gating_host_drain(pipe: &mut SimPipe) -> Vec<Message> {
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

#[test]
fn send_input_event_is_inert_without_a_negotiated_capability() {
    // Session accepted (Hello answered) but Ready never negotiated PHYSICAL_INPUT_V1: the
    // capability gate — not just the session gate — must be what stops emission.
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(gating_identity());

    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::NONE,
            nonce: 0xAAAA_BBBB,
        }),
        0,
    );
    dispatcher.poll(&mut pipe, &mut device, 0).expect("poll");
    assert_eq!(dispatcher.accepted_session(), Some(0xAAAA_BBBB));
    let _ = pipe.host_recv(); // discard the HelloAck; only the gate under test matters here

    let sent = dispatcher.send_input_event(&mut pipe, 1, InputKind::GestureStarted, 0);
    assert!(!sent, "an unnegotiated capability must stay inert");
    assert!(
        gating_host_drain(&mut pipe).is_empty(),
        "nothing may reach the wire"
    );
}

#[test]
fn send_input_event_is_inert_without_an_accepted_session() {
    // Ready negotiates the capability, but Hello/HelloAck never happened: no session nonce was
    // ever accepted, so emission must still stay inert.
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(gating_identity());

    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        0,
    );
    dispatcher.poll(&mut pipe, &mut device, 0).expect("poll");
    assert_eq!(dispatcher.accepted_session(), None);

    let sent = dispatcher.send_input_event(&mut pipe, 1, InputKind::GestureStarted, 0);
    assert!(!sent, "no accepted session must stay inert");
    assert!(gating_host_drain(&mut pipe).is_empty());
}

#[test]
fn send_input_event_emits_on_the_wire_once_negotiated_and_accepted() {
    let mut pipe = SimPipe::new();
    let mut dispatcher = handshaken(0x1111_2222);

    let sent = dispatcher.send_input_event(&mut pipe, 7, InputKind::Detent(Direction::Cw), 42);
    assert!(
        sent,
        "a negotiated capability and accepted session must emit"
    );

    match gating_host_drain(&mut pipe).as_slice() {
        [Message::InputEvent(event)] => {
            assert_eq!(event.session, 0x1111_2222);
            assert_eq!(event.gesture_id, 7);
            assert_eq!(event.control, ControlId::Rotary);
            assert_eq!(event.kind, InputKind::Detent(Direction::Cw));
            assert_eq!(event.device_ms, 42);
        }
        other => panic!("expected a single InputEvent, got {other:?}"),
    }
}

#[test]
fn a_failed_input_write_invalidates_the_session_for_runtime_recovery() {
    struct ZeroWrite;

    impl Transport for ZeroWrite {
        type Error = ();

        fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
            Ok(0)
        }

        fn write(&mut self, _buf: &[u8]) -> Result<usize, Self::Error> {
            Ok(0)
        }
    }

    let mut dispatcher = handshaken(0x1111_2222);
    assert!(!dispatcher.send_input_event(&mut ZeroWrite, 7, InputKind::Detent(Direction::Cw), 42,));
    assert_eq!(dispatcher.accepted_session(), None);
    assert!(
        dispatcher.take_session_ended(),
        "the runtime must be told to reset its gesture state"
    );
}

#[test]
fn send_input_event_is_inert_when_the_desktop_over_claims_a_capability_the_device_never_advertised()
{
    // The device itself never advertises PHYSICAL_INPUT_V1 (capabilities: NONE), but a buggy or
    // hostile desktop sends `Ready` claiming it anyway. The dispatcher must intersect with what
    // the device actually advertised, not trust the desktop's claim verbatim — an unnegotiated
    // capability MUST NOT activate behavior.
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(DeviceIdentity {
        device_id: [0xEF; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: Capabilities::NONE,
    });

    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xC0FF_EE00,
        }),
        0,
    );
    dispatcher.poll(&mut pipe, &mut device, 0).expect("poll");
    let _ = pipe.host_recv();

    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            // Over-claims a capability the device never advertised.
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        1,
    );
    dispatcher.poll(&mut pipe, &mut device, 0).expect("poll");

    let sent = dispatcher.send_input_event(&mut pipe, 1, InputKind::GestureStarted, 0);
    assert!(
        !sent,
        "a capability the device never advertised must not activate, even if the desktop claims it"
    );
    assert!(gating_host_drain(&mut pipe).is_empty());
}

// --- session-boundary reset (controller decision 4): a gesture opened in one session must not
// be completable, or silently continued, in the next one. Exercised through the real `Runtime`
// (not just the decoder/gesture types directly), since the reset is wired in `Runtime::step`.

use core::cell::Cell;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::Transport;
use kivori_firmware::runtime::{Runtime, RuntimeConfig};
use kivori_firmware::sim::{CaptureDisplay, VirtualClock};
use kivori_protocol::{Bye, ByeReason};

/// Asserts every `InputEvent` in `events` carries exactly `session`, and that no `Detent` or
/// `GestureEnded` for a gesture appears before that gesture's `GestureStarted` — i.e. the stream
/// never continues a gesture silently (no stale-or-mismatched session nonce, no orphaned detent).
fn assert_clean_input_stream(events: &[Message], session: Nonce) {
    use std::collections::HashSet;
    let mut started: HashSet<u16> = HashSet::new();
    for m in events {
        let Message::InputEvent(event) = m else {
            continue;
        };
        assert_eq!(
            event.session, session,
            "an InputEvent carried the wrong/stale session nonce"
        );
        match event.kind {
            InputKind::GestureStarted => {
                started.insert(event.gesture_id);
            }
            InputKind::Detent(_) | InputKind::GestureEnded => {
                assert!(
                    started.contains(&event.gesture_id),
                    "gesture {} produced a Detent/GestureEnded with no preceding GestureStarted",
                    event.gesture_id
                );
            }
            // Discrete push-switch events have no gesture lifecycle to check.
            InputKind::Press | InputKind::Hold | InputKind::DoublePress => {}
        }
    }
}

/// Wraps a `SimPipe`, failing the next `read` exactly once when armed via `fail_next_read`. Lets a
/// host-sim test exercise a genuine transport failure — `SimPipe` itself is `Infallible` and can
/// never fail on its own.
struct FlakyTransport<'p> {
    inner: &'p mut SimPipe,
    fail_next_read: &'p Cell<bool>,
}

struct WriteFailTransport<'p> {
    inner: &'p mut SimPipe,
}

impl Transport for WriteFailTransport<'_> {
    type Error = ();

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.inner.read(buf).map_err(|_| ())
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, Self::Error> {
        Ok(0)
    }
}

impl Transport for FlakyTransport<'_> {
    type Error = ();

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if self.fail_next_read.replace(false) {
            return Err(());
        }
        self.inner.read(buf).map_err(|_| ())
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.inner.write(buf).map_err(|_| ())
    }
}

/// One full CW detent cycle as a level script: 00 -> 01 -> 11 -> 10 -> 00.
fn cw_cycle_levels() -> FixedVec<InputLevels, SCRIPT_CAPACITY> {
    script([
        lv(false, false),
        lv(false, true),
        lv(true, true),
        lv(true, false),
        lv(false, false),
    ])
}

#[test]
fn an_input_write_failure_drops_the_link_before_the_tick_returns() {
    let mut runtime = Runtime::new(gating_identity(), RuntimeConfig::default());
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let mut display = CaptureDisplay::new();
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");
    let mut idle = ScriptedInput::new(script([lv(false, false)]));

    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xA000_0007,
        }),
        0,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        1,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    gating_host_write(
        &mut pipe,
        &Message::SetState(kivori_protocol::SetState {
            desired: kivori_model::SendableState::Happy,
            at_ms: None,
        }),
        2,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    let _ = pipe.host_recv();
    assert_eq!(runtime.state(), CompanionState::Happy);

    let mut cw = ScriptedInput::new(cw_cycle_levels());
    let mut dropped = false;
    for _ in 0..5 {
        clock.advance(1);
        let tick = runtime.step(
            &clock,
            &mut WriteFailTransport { inner: &mut pipe },
            &mut cw,
            &mut display,
            &blob,
        );
        dropped |= tick.link_dropped;
    }
    assert!(dropped, "the failed GestureStarted must drop this tick");
    assert_eq!(runtime.state(), CompanionState::Offline);

    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xB000_0008,
        }),
        0,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        1,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    let _ = pipe.host_recv();

    let mut fresh_cw = ScriptedInput::new(cw_cycle_levels());
    for _ in 0..5 {
        clock.advance(1);
        runtime.step(&clock, &mut pipe, &mut fresh_cw, &mut display, &blob);
    }
    let events = gating_host_drain(&mut pipe);
    assert_clean_input_stream(&events, 0xB000_0008);
    assert!(events.iter().any(|message| matches!(
        message,
        Message::InputEvent(event) if matches!(event.kind, InputKind::GestureStarted)
    )));
}

#[test]
fn a_detent_after_a_stall_opens_a_new_gesture_instead_of_extending_the_quiet_one() {
    let mut runtime = Runtime::new(gating_identity(), RuntimeConfig::default());
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let mut display = CaptureDisplay::new();
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");
    let mut idle = ScriptedInput::new(script([lv(false, false)]));

    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xC000_0003,
        }),
        0,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        1,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    let _ = pipe.host_recv();

    // Detent 1, then three quarter-steps of detent 2 inside the window...
    let mut levels = cw_cycle_levels();
    levels
        .extend_from_slice(&[
            lv(false, true),
            lv(true, true),
            lv(true, false),
            lv(false, false),
        ])
        .expect("scenario fits SCRIPT_CAPACITY");
    let mut input = ScriptedInput::new(levels);
    for _ in 0..8 {
        clock.advance(1);
        runtime.step(&clock, &mut pipe, &mut input, &mut display, &blob);
    }
    // ...then a render stall longer than the idle window before the tick that completes it.
    clock.advance(300);
    runtime.step(&clock, &mut pipe, &mut input, &mut display, &blob);

    let kinds: Vec<(u16, InputKind)> = gating_host_drain(&mut pipe)
        .into_iter()
        .filter_map(|m| match m {
            Message::InputEvent(e) => Some((e.gesture_id, e.kind)),
            _ => None,
        })
        .collect();
    let ended_1 = kinds
        .iter()
        .position(|&(id, k)| id == 1 && matches!(k, InputKind::GestureEnded));
    let started_2 = kinds
        .iter()
        .position(|&(id, k)| id == 2 && matches!(k, InputKind::GestureStarted));
    assert!(
        matches!((ended_1, started_2), (Some(e), Some(s)) if e < s),
        "gesture 1 must end before the stalled detent opens gesture 2: {kinds:?}"
    );
}

#[test]
fn a_gesture_open_before_bye_cannot_be_silently_continued_after_reconnecting() {
    let mut runtime = Runtime::new(gating_identity(), RuntimeConfig::default());
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let mut display = CaptureDisplay::new();
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");
    let mut idle = ScriptedInput::new(script([lv(false, false)]));

    // Session A: handshake, negotiating PHYSICAL_INPUT_V1.
    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xA000_0001,
        }),
        0,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        1,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    let _ = pipe.host_recv();

    // One full CW detent opens gesture 1 in session A. Well inside the 250ms window, so it stays
    // open (no `GestureEnded` yet) when the session closes.
    let mut cw = ScriptedInput::new(cw_cycle_levels());
    for _ in 0..5 {
        clock.advance(1);
        runtime.step(&clock, &mut pipe, &mut cw, &mut display, &blob);
    }
    let opened_in_a = gating_host_drain(&mut pipe).into_iter().any(|m| {
        matches!(
            m,
            Message::InputEvent(e) if e.session == 0xA000_0001
                && matches!(e.kind, InputKind::GestureStarted)
        )
    });
    assert!(
        opened_in_a,
        "the first detent must open a gesture in session A"
    );

    // `Bye` closes session A while the gesture is still open.
    gating_host_write(
        &mut pipe,
        &Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        2,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    let _ = pipe.host_recv();

    // Session B: a fresh handshake with a different nonce.
    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xB000_0002,
        }),
        3,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        4,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    let _ = pipe.host_recv();

    // A second full CW detent in session B. Without the reset, `RotaryGesture` would still think
    // gesture 1 from session A is open and would silently continue it (no `GestureStarted`) — a
    // gesture from the old session leaking into the new one.
    let mut cw2 = ScriptedInput::new(cw_cycle_levels());
    for _ in 0..5 {
        clock.advance(1);
        runtime.step(&clock, &mut pipe, &mut cw2, &mut display, &blob);
    }
    let opened_fresh_in_b = gating_host_drain(&mut pipe).into_iter().any(|m| {
        matches!(
            m,
            Message::InputEvent(e) if e.session == 0xB000_0002
                && matches!(e.kind, InputKind::GestureStarted)
        )
    });
    assert!(
        opened_fresh_in_b,
        "a fresh GestureStarted must fire in session B; the old session's open gesture must not \
         silently continue across the boundary"
    );
}

#[test]
fn a_gesture_open_on_reconnect_without_bye_cannot_be_silently_continued() {
    // Variant of the test above with NO `Bye` at all — the common real-world case (a desktop
    // crash or a force-quit never sends one). A bare new `Hello` must still reset the input state.
    let mut runtime = Runtime::new(gating_identity(), RuntimeConfig::default());
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let mut display = CaptureDisplay::new();
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");
    let mut idle = ScriptedInput::new(script([lv(false, false)]));

    // Session A: handshake, negotiating PHYSICAL_INPUT_V1.
    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xA000_0003,
        }),
        0,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        1,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    let _ = pipe.host_recv();

    // One full CW detent opens gesture 1 in session A, left open (well inside the 250ms window).
    let mut cw = ScriptedInput::new(cw_cycle_levels());
    for _ in 0..5 {
        clock.advance(1);
        runtime.step(&clock, &mut pipe, &mut cw, &mut display, &blob);
    }
    let events_a = gating_host_drain(&mut pipe);
    assert_clean_input_stream(&events_a, 0xA000_0003);
    assert!(
        events_a.iter().any(|m| matches!(
            m,
            Message::InputEvent(e) if matches!(e.kind, InputKind::GestureStarted)
        )),
        "the first detent must open a gesture in session A"
    );

    // NO `Bye`. The desktop just reconnects: a bare Hello -> Ready for session B.
    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xB000_0004,
        }),
        2,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        3,
    );
    runtime.step(&clock, &mut pipe, &mut idle, &mut display, &blob);
    let _ = pipe.host_recv();

    // A second full CW detent in session B. Without resetting on a bare `Hello`, the old open
    // gesture would silently continue (no `GestureStarted`) and its events would still be stamped
    // with session A's nonce even though the dispatcher has already moved on to session B.
    let mut cw2 = ScriptedInput::new(cw_cycle_levels());
    for _ in 0..5 {
        clock.advance(1);
        runtime.step(&clock, &mut pipe, &mut cw2, &mut display, &blob);
    }
    let events_b = gating_host_drain(&mut pipe);
    assert_clean_input_stream(&events_b, 0xB000_0004);
    assert!(
        events_b.iter().any(|m| matches!(
            m,
            Message::InputEvent(e) if matches!(e.kind, InputKind::GestureStarted)
        )),
        "a fresh GestureStarted must fire in session B even though no Bye was ever sent"
    );
}

#[test]
fn a_gesture_open_before_link_loss_cannot_be_silently_continued_after_reconnecting() {
    // A transport failure with no `Bye` at all (a yanked cable, not a clean shutdown) must reset
    // input state exactly like `Bye` does.
    let mut runtime = Runtime::new(gating_identity(), RuntimeConfig::default());
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let mut display = CaptureDisplay::new();
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");
    let mut idle = ScriptedInput::new(script([lv(false, false)]));
    let fail_next_read = Cell::new(false);

    // Session A: handshake, negotiating PHYSICAL_INPUT_V1.
    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xA000_0005,
        }),
        0,
    );
    runtime.step(
        &clock,
        &mut FlakyTransport {
            inner: &mut pipe,
            fail_next_read: &fail_next_read,
        },
        &mut idle,
        &mut display,
        &blob,
    );
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        1,
    );
    runtime.step(
        &clock,
        &mut FlakyTransport {
            inner: &mut pipe,
            fail_next_read: &fail_next_read,
        },
        &mut idle,
        &mut display,
        &blob,
    );
    let _ = pipe.host_recv();

    // One full CW detent opens gesture 1 in session A, left open.
    let mut cw = ScriptedInput::new(cw_cycle_levels());
    for _ in 0..5 {
        clock.advance(1);
        runtime.step(
            &clock,
            &mut FlakyTransport {
                inner: &mut pipe,
                fail_next_read: &fail_next_read,
            },
            &mut cw,
            &mut display,
            &blob,
        );
    }
    let events_a = gating_host_drain(&mut pipe);
    assert_clean_input_stream(&events_a, 0xA000_0005);
    assert!(
        events_a.iter().any(|m| matches!(
            m,
            Message::InputEvent(e) if matches!(e.kind, InputKind::GestureStarted)
        )),
        "the first detent must open a gesture in session A"
    );

    // Force a single transport read failure: a genuine link loss, with no `Bye` ever sent.
    fail_next_read.set(true);
    let tick = runtime.step(
        &clock,
        &mut FlakyTransport {
            inner: &mut pipe,
            fail_next_read: &fail_next_read,
        },
        &mut idle,
        &mut display,
        &blob,
    );
    assert!(
        tick.link_dropped,
        "the forced read failure must be observed as a dropped link"
    );
    let _ = pipe.host_recv();

    // Quiet period, deliberately with NO new `Hello` yet: advance well past the 250ms gesture
    // window on idle input alone. This isolates `link_lost()`'s own effect from the (separate)
    // reset a fresh `Hello` performs — without `link_lost()` clearing `accepted_session` and
    // resetting the gesture, the still-open gesture from session A would time out here and its
    // `GestureEnded` would reach the wire under session A's nonce, with no reconnect in sight.
    for _ in 0..300 {
        clock.advance(1);
        runtime.step(
            &clock,
            &mut FlakyTransport {
                inner: &mut pipe,
                fail_next_read: &fail_next_read,
            },
            &mut idle,
            &mut display,
            &blob,
        );
    }
    let events_quiet = gating_host_drain(&mut pipe);
    assert!(
        !events_quiet
            .iter()
            .any(|m| matches!(m, Message::InputEvent(_))),
        "no InputEvent may reach the wire after a link loss until a new session is established; \
         got {events_quiet:?}"
    );

    // Session B: reconnect once the link recovers.
    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0xB000_0006,
        }),
        2,
    );
    runtime.step(
        &clock,
        &mut FlakyTransport {
            inner: &mut pipe,
            fail_next_read: &fail_next_read,
        },
        &mut idle,
        &mut display,
        &blob,
    );
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        3,
    );
    runtime.step(
        &clock,
        &mut FlakyTransport {
            inner: &mut pipe,
            fail_next_read: &fail_next_read,
        },
        &mut idle,
        &mut display,
        &blob,
    );
    let _ = pipe.host_recv();

    // A second full CW detent in session B. Without resetting on link loss, the old open gesture
    // would silently continue, stamped with the wrong (session A) nonce.
    let mut cw2 = ScriptedInput::new(cw_cycle_levels());
    for _ in 0..5 {
        clock.advance(1);
        runtime.step(
            &clock,
            &mut FlakyTransport {
                inner: &mut pipe,
                fail_next_read: &fail_next_read,
            },
            &mut cw2,
            &mut display,
            &blob,
        );
    }
    let events_b = gating_host_drain(&mut pipe);
    assert_clean_input_stream(&events_b, 0xB000_0006);
    assert!(
        events_b.iter().any(|m| matches!(
            m,
            Message::InputEvent(e) if matches!(e.kind, InputKind::GestureStarted)
        )),
        "a fresh GestureStarted must fire in session B; a link loss with no Bye must still reset \
         input state"
    );
}

use kivori_model::presentation::{PrimaryState, ValueConfidence, ValueDisplay, ValueKind};
use kivori_protocol::message::Presentation;

fn pres(session: u32, revision: u32, percent: u8, transient_ms: u16) -> Presentation {
    Presentation {
        session,
        revision,
        primary: PrimaryState::Idle,
        value: Some(ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: percent,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
        }),
        transient_ms,
    }
}

#[test]
fn a_newer_revision_is_accepted_and_an_older_one_is_dropped() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);

    assert!(state.apply(&pres(0xAAAA, 5, 50, 800), 1_000));
    assert!(
        !state.apply(&pres(0xAAAA, 4, 10, 800), 1_010),
        "older revision"
    );
    assert!(
        !state.apply(&pres(0xAAAA, 5, 10, 800), 1_020),
        "same revision"
    );
    assert!(state.apply(&pres(0xAAAA, 6, 60, 800), 1_030));
}

#[test]
fn a_presentation_from_another_session_is_dropped() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    assert!(state.apply(&pres(0xAAAA, 5, 50, 800), 1_000));
    assert!(
        !state.apply(&pres(0xBBBB, 900, 10, 800), 1_010),
        "a stale high-revision presentation from a previous session must not render"
    );
}

#[test]
fn a_new_session_accepts_revision_one_again() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    assert!(state.apply(&pres(0xAAAA, 743, 50, 800), 1_000));

    state.begin_session(0xBBBB);
    assert!(
        state.apply(&pres(0xBBBB, 1, 20, 800), 2_000),
        "a restarted desktop must not be rejected as stale"
    );
}

#[test]
fn the_transient_overlay_expires_locally_back_to_primary() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    state.apply(&pres(0xAAAA, 1, 50, 800), 1_000);

    assert!(state.value_at(1_799).is_some(), "still inside the window");
    assert!(
        state.value_at(1_800).is_none(),
        "expired overlays restore the underlying primary state"
    );
    assert_eq!(state.primary(), PrimaryState::Idle);
}

#[test]
fn a_persistent_presentation_never_expires() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    state.apply(&pres(0xAAAA, 1, 50, 0), 1_000);
    assert!(state.value_at(600_000).is_some());
}

/// A precomputed `applied_at_ms + transient_ms` deadline can itself wrap past `u32::MAX` (~49.7
/// days of device uptime) before `now_ms` does, which would make a naive `now_ms >= deadline`
/// comparison see a small deadline and a huge `now_ms` and report the overlay expired instantly —
/// even though almost no real time has passed. `value_at` must compare elapsed time
/// (`now_ms.wrapping_sub(applied_at_ms)`), not a precomputed absolute deadline.
#[test]
fn transient_expiry_is_wrap_safe_across_a_device_uptime_rollover() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    let applied_at = u32::MAX - 100;
    assert!(state.apply(&pres(0xAAAA, 1, 50, 800), applied_at));

    // Only 50ms have really elapsed since `applied_at` (no wrap has happened yet): must still be
    // inside the 800ms window, even though `applied_at + 800` itself wraps past `u32::MAX`.
    assert!(
        state.value_at(applied_at + 50).is_some(),
        "only 50ms elapsed; must not expire instantly due to the deadline wrapping"
    );

    // 800ms after `applied_at`, the clock has now genuinely wrapped: the overlay must still
    // expire correctly once that much real time has passed.
    assert!(
        state.value_at(applied_at.wrapping_add(800)).is_none(),
        "800ms genuinely elapsed (clock wrapped) must still expire"
    );
}

// --- Presentation capability gate reaches the panel as a real tile flush (task-11 review round 1,
// finding 3): a unit test on `PresentationResolver`/`PresentationState` alone would still pass if
// the capability gate, the session-boundary wiring, or the render-gate wiring in `Runtime::step`
// were deleted. These drive the real `Runtime` over the wire and assert on `Tick::tiles_flushed`.

use kivori_firmware::render::TileRenderer;
use kivori_framebuffer::hash_rgb565;
use kivori_model::CompanionState;

fn presentation_gating_identity(capabilities: Capabilities) -> DeviceIdentity {
    DeviceIdentity {
        device_id: [0xAB; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities,
    }
}

fn presentation_message(session: Nonce, revision: u32) -> Message {
    Message::Presentation(Presentation {
        session,
        revision,
        primary: PrimaryState::Idle,
        value: Some(ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: 70,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
        }),
        transient_ms: 800,
    })
}

/// Runs Hello -> Ready -> (optionally) Presentation -> one frame, with the desktop claiming
/// `PRESENTATION_V1` throughout, and returns the captured panel. The mascot animates on its own
/// clock, so a bare "tiles flushed" count cannot tell overlay pixels from animation pixels; the
/// tests below compare against a deterministic twin run that never sends the `Presentation`.
fn presentation_scenario(
    device_caps: Capabilities,
    send_presentation: bool,
) -> Box<CaptureDisplay> {
    let mut runtime = Runtime::new(
        presentation_gating_identity(device_caps),
        RuntimeConfig::default(),
    );
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let mut display = Box::new(CaptureDisplay::new());
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");
    let mut idle = ScriptedInput::new(script([lv(false, false)]));
    let nonce: Nonce = 0xC0DE_0001;

    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PRESENTATION_V1,
            nonce,
        }),
        0,
    );
    runtime.step(&clock, &mut pipe, &mut idle, display.as_mut(), &blob);
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: Capabilities::PRESENTATION_V1,
        }),
        1,
    );
    runtime.step(&clock, &mut pipe, &mut idle, display.as_mut(), &blob);

    if send_presentation {
        gating_host_write(&mut pipe, &presentation_message(nonce, 1), 2);
    }
    clock.advance(33); // cross the frame-interval boundary so the render gate actually fires
    let tick = runtime.step(&clock, &mut pipe, &mut idle, display.as_mut(), &blob);
    assert!(
        tick.frame_rendered,
        "the scenario must end on a rendered frame"
    );
    display
}

/// Rows of the volume bar itself.
const OVERLAY_BAR_ROWS: core::ops::Range<usize> = 180..196;

#[test]
fn a_negotiated_presentation_reaches_the_panel_as_a_real_tile_flush() {
    let with = presentation_scenario(Capabilities::PRESENTATION_V1, true);
    let without = presentation_scenario(Capabilities::PRESENTATION_V1, false);

    assert_ne!(
        hash_rgb565(with.frame()),
        hash_rgb565(without.frame()),
        "a negotiated Presentation must actually reach the panel pixels, not just update \
         in-memory state"
    );
    // The mascot makes room for the bar: below the lifted keycap and outside the bar only the
    // background remains, where the twin run still shows the full-size base.
    let background = with.pixel(0, 239);
    let clear = |x: usize, y: usize| {
        (176..240).contains(&y) && !((24..216).contains(&x) && OVERLAY_BAR_ROWS.contains(&y))
    };
    let mut twin_had_mascot_there = false;
    for y in 0..240 {
        for x in 0..240 {
            if clear(x, y) {
                assert_eq!(
                    with.pixel(x, y),
                    background,
                    "mascot overlaps the bar at ({x}, {y})"
                );
                twin_had_mascot_there |= without.pixel(x, y) != background;
            }
        }
    }
    assert!(
        twin_had_mascot_there,
        "the full-size base must reach below y176"
    );
}

#[test]
fn an_unnegotiated_presentation_never_reaches_the_panel() {
    // The desktop over-claims PRESENTATION_V1 in both `Hello` and `Ready`; the DEVICE never
    // advertised it, so the dispatcher's intersection must still gate it out — mirrors
    // `send_input_event_is_inert_when_the_desktop_over_claims_a_capability_the_device_never_advertised`.
    let with = presentation_scenario(Capabilities::NONE, true);
    let without = presentation_scenario(Capabilities::NONE, false);

    assert_eq!(
        hash_rgb565(with.frame()),
        hash_rgb565(without.frame()),
        "an unnegotiated capability must leave the panel exactly as it would be without it"
    );
}

#[test]
fn render_with_overlay_composites_the_bar_into_the_flushed_pixels() {
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");

    let mut plain = TileRenderer::new();
    let mut plain_display = CaptureDisplay::new();
    plain
        .render_with_overlay(&blob, CompanionState::Idle, 0, None, &mut plain_display)
        .expect("plain render");

    let mut overlaid = TileRenderer::new();
    let mut overlaid_display = CaptureDisplay::new();
    overlaid
        .render_with_overlay(
            &blob,
            CompanionState::Idle,
            0,
            Some(ValueDisplay {
                kind: ValueKind::Volume,
                current_percent: 60,
                confidence: ValueConfidence::Confirmed,
                at_boundary: false,
            }),
            &mut overlaid_display,
        )
        .expect("overlaid render");

    assert_ne!(
        hash_rgb565(plain_display.frame()),
        hash_rgb565(overlaid_display.frame()),
        "a Some overlay must actually change the rendered pixels, not just the in-memory value"
    );
}

#[test]
fn the_rotary_profile_does_not_collide_with_the_display_profile() {
    use kivori_firmware::profile::physical_st7789::{BL, DC, MOSI, ROTARY, RST, SCK};

    let display_pins = [SCK, MOSI, DC, RST, BL];
    for pin in [ROTARY.clk, ROTARY.dt, ROTARY.sw] {
        assert!(
            !display_pins.contains(&pin),
            "rotary pin {pin} collides with the verified display profile"
        );
        // GPIO18/19 are the native USB Serial/JTAG pair.
        assert!(
            pin != 18 && pin != 19,
            "rotary pin {pin} collides with native USB"
        );
    }

    assert_ne!(ROTARY.clk, ROTARY.dt);
    assert_ne!(ROTARY.clk, ROTARY.sw);
    assert_ne!(ROTARY.dt, ROTARY.sw);
}

#[test]
fn a_reconnect_hello_clears_the_previous_sessions_capabilities() {
    // A reconnect that never sent `Bye` arrives as a bare `Hello`. Between it and the `Ready`
    // that follows, the device already holds the NEW session's nonce; if it also still held the
    // OLD session's capabilities, a detent landing in that window would emit under a capability
    // the new peer has not negotiated. `link_lost` and the `Bye` arm both clear it, and the
    // field's own doc comment says it belongs to "the current session" — so `Hello` must too.
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = handshaken(0x1111_2222);
    assert!(
        dispatcher.send_input_event(&mut pipe, 1, InputKind::GestureStarted, 0),
        "precondition: the first session emits"
    );
    let _ = pipe.host_recv();

    gating_host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
            nonce: 0x3333_4444,
        }),
        2,
    );
    dispatcher
        .poll(&mut pipe, &mut device, 0)
        .expect("poll reconnect Hello");
    assert_eq!(dispatcher.accepted_session(), Some(0x3333_4444));
    let _ = pipe.host_recv(); // discard the HelloAck; only the capability gate matters here

    let sent = dispatcher.send_input_event(&mut pipe, 2, InputKind::Detent(Direction::Cw), 1);
    assert!(
        !sent,
        "the previous session's capabilities must not survive a reconnect Hello"
    );
    assert!(
        gating_host_drain(&mut pipe).is_empty(),
        "nothing may reach the wire between Hello and Ready"
    );

    // ...and the new session's own `Ready` re-establishes them.
    gating_host_write(
        &mut pipe,
        &Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
        }),
        3,
    );
    dispatcher
        .poll(&mut pipe, &mut device, 0)
        .expect("poll Ready");
    assert!(
        dispatcher.send_input_event(&mut pipe, 2, InputKind::Detent(Direction::Cw), 2),
        "the new session negotiated the capability, so it emits again"
    );
}

/// Row 3.14: fresh feedback must not wait out the frame interval. A `Presentation` arriving
/// mid-interval renders on the very tick it is applied; with no new presentation the next tick
/// still waits for the frame deadline.
#[test]
fn an_applied_presentation_renders_at_once_without_waiting_for_the_frame_deadline() {
    let mut runtime = Runtime::new(
        presentation_gating_identity(Capabilities::PRESENTATION_V1),
        RuntimeConfig::default(),
    );
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let mut display = Box::new(CaptureDisplay::new());
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");
    let mut idle = ScriptedInput::new(script([lv(false, false)]));
    let nonce: Nonce = 0xC0DE_0002;
    let hello = Message::Hello(Hello {
        desktop_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        desktop_caps: Capabilities::PRESENTATION_V1,
        nonce,
    });
    gating_host_write(&mut pipe, &hello, 0);
    runtime.step(&clock, &mut pipe, &mut idle, display.as_mut(), &blob);
    let ready = Message::Ready(Ready {
        negotiated_minor: 0,
        negotiated_caps: Capabilities::PRESENTATION_V1,
    });
    gating_host_write(&mut pipe, &ready, 1);
    clock.advance(1);
    assert!(
        !runtime
            .step(&clock, &mut pipe, &mut idle, display.as_mut(), &blob)
            .frame_rendered,
        "mid-interval with nothing new: the frame cadence holds"
    );

    gating_host_write(&mut pipe, &presentation_message(nonce, 1), 2);
    clock.advance(1);
    assert!(
        runtime
            .step(&clock, &mut pipe, &mut idle, display.as_mut(), &blob)
            .frame_rendered,
        "an applied presentation renders on the tick it arrives"
    );
}

/// An input source that delivers a whole detent's worth of edge snapshots in one drain, the way
/// the interrupt-driven physical adapter does after a long render pass.
struct BurstInput(Vec<(InputLevels, u32)>);

impl kivori_firmware::ports::InputSource for BurstInput {
    fn sample(&mut self) -> InputLevels {
        lv(false, false)
    }

    fn drain(&mut self, now_ms: u32, f: &mut dyn FnMut(InputLevels, u32)) {
        for (levels, at_ms) in self.0.drain(..) {
            f(levels, at_ms);
        }
        f(self.sample(), now_ms);
    }
}

/// Edges captured while the loop was busy rendering are all decoded on the next tick, and each
/// event carries the time its edge was captured, not the time the loop got around to it.
#[test]
fn every_drained_edge_snapshot_is_decoded_in_one_tick_with_its_capture_time() {
    let mut runtime = Runtime::new(gating_identity(), RuntimeConfig::default());
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let mut display = Box::new(CaptureDisplay::new());
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).expect("valid blob");
    let nonce: Nonce = 0xBEEF_0001;
    let mut input = BurstInput(Vec::new());
    let hello = Message::Hello(Hello {
        desktop_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        desktop_caps: Capabilities::PHYSICAL_INPUT_V1,
        nonce,
    });
    gating_host_write(&mut pipe, &hello, 0);
    runtime.step(&clock, &mut pipe, &mut input, display.as_mut(), &blob);
    let ready = Message::Ready(Ready {
        negotiated_minor: 0,
        negotiated_caps: Capabilities::PHYSICAL_INPUT_V1,
    });
    gating_host_write(&mut pipe, &ready, 1);
    runtime.step(&clock, &mut pipe, &mut input, display.as_mut(), &blob);
    let _ = gating_host_drain(&mut pipe);

    clock.advance(60);
    input.0 = vec![
        (lv(false, true), 10),
        (lv(true, true), 12),
        (lv(true, false), 14),
        (lv(false, false), 16),
    ];
    runtime.step(&clock, &mut pipe, &mut input, display.as_mut(), &blob);

    let events: Vec<_> = gating_host_drain(&mut pipe)
        .into_iter()
        .filter_map(|message| match message {
            Message::InputEvent(event) => Some((event.kind, event.device_ms)),
            _ => None,
        })
        .collect();
    assert_eq!(
        events,
        [
            (InputKind::GestureStarted, 16),
            (InputKind::Detent(Direction::Cw), 16)
        ]
    );
}
