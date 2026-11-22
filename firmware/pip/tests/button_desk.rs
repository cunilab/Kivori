//! M1 on the real runtime over the wire: push-switch Press / Hold, the recovery hold, and the
//! session-scoped desk status. Host simulation only — physical switch timing is a hardware row.

use std::cell::{Cell, RefCell};

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::{Clock, InputSource};
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig, Tick};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_framebuffer::hash_rgb565;
use kivori_model::desk::{ControlLabels, DeskStatus, DisplayMode, MediaText};
use kivori_model::input::{Direction, InputLevels};
use kivori_model::picker::{HoldActions, PickerName, ProfileChoices};
use kivori_model::{Capabilities, CompanionState, ProtocolVersion, SendableState};
use kivori_protocol::{
    decode_message, encode_message, ByeReason, ControlId, ControlLabelsUpdate, FirmwareVersion,
    HeldTurnModeUpdate, Hello, HoldActionsUpdate, InputEvent, InputKind, Message, Nonce, Ping,
    ProfileChoicesUpdate, Ready, SetState, Status, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR,
    PROTOCOL_MINOR,
};

const M1: Capabilities = Capabilities::PHYSICAL_INPUT_V1
    .union(Capabilities::BUTTON_INPUT_V1)
    .union(Capabilities::DESK_STATUS_V1)
    .union(Capabilities::ACTION_FEEDBACK_V1);

/// Levels the test sets between steps.
#[derive(Default)]
struct Levels(
    Cell<(bool, bool, bool)>,
    Cell<[bool; 3]>,
    RefCell<Vec<(InputLevels, u32)>>,
    Cell<u32>,
);

impl InputSource for &Levels {
    fn sample(&mut self) -> InputLevels {
        let (a, b, sw) = self.0.get();
        InputLevels {
            a,
            b,
            sw,
            keys: self.1.get(),
        }
    }

    fn drain(&mut self, now_ms: u32, f: &mut dyn FnMut(InputLevels, u32)) {
        for (levels, at_ms) in self.2.borrow_mut().drain(..) {
            f(levels, at_ms);
        }
        f(self.sample(), now_ms);
    }

    fn dropped_edges(&self) -> u32 {
        self.3.get()
    }
}

struct Rig {
    runtime: Runtime<'static>,
    clock: VirtualClock,
    pipe: SimPipe,
    display: Box<CaptureDisplay>,
    blob: AssetBlob<'static>,
    levels: Levels,
    seq: u16,
    nonce: Nonce,
}

impl Rig {
    fn new() -> Self {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            // Folds to the default idle seed: these tests compare faces at different times, so they
            // keep the one idle schedule they were written against.
            device_id: {
                let mut id = [0; 16];
                id[..4].copy_from_slice(&kivori_model::mascot::DEFAULT_IDLE_SEED.to_le_bytes());
                id
            },
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            capabilities: M1
                .union(Capabilities::PRESENTATION_V1)
                .union(Capabilities::DOUBLE_PRESS_V1)
                .union(Capabilities::MEDIA_INFO_V1)
                .union(Capabilities::CONTROL_LABELS_V1)
                .union(Capabilities::CONTEXT_BUTTONS_V1)
                .union(Capabilities::HELD_ROTATION_V1)
                .union(Capabilities::PROFILE_PICKER_V1)
                .union(Capabilities::HOLD_RING_V1)
                .union(Capabilities::KNOB_HOLD_V1)
                .union(Capabilities::HELD_TURN_MODE_V1),
        };
        let mut rig = Self {
            runtime: Runtime::new(identity, RuntimeConfig::default()),
            clock: VirtualClock::new(),
            pipe: SimPipe::new(),
            display: Box::new(CaptureDisplay::new()),
            blob: AssetBlob::parse(bytes).expect("valid blob"),
            levels: Levels::default(),
            seq: 0,
            nonce: 0x5EED_0001,
        };
        rig.step();
        rig
    }

    fn send(&mut self, message: &Message) {
        let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
        encode_message(
            message,
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            self.seq,
            &mut wire,
        )
        .expect("encode");
        self.seq += 1;
        self.pipe.host_send(&wire).expect("pipe capacity");
    }

    fn connect(&mut self, caps: Capabilities) {
        let hello = Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: caps,
            nonce: self.nonce,
        });
        self.send(&hello);
        self.step();
        self.send(&Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: caps,
        }));
        self.step();
        let _ = self.drain();
    }

    fn step(&mut self) -> Tick {
        let mut input = &self.levels;
        self.runtime.step(
            &self.clock,
            &mut self.pipe,
            &mut input,
            self.display.as_mut(),
            &self.blob,
        )
    }

    /// Advances `ms` in 10 ms ticks, returning whether any tick asked for a reboot.
    fn run(&mut self, ms: u32) -> bool {
        let mut reboot = false;
        for _ in 0..ms / 10 {
            self.clock.advance(10);
            reboot |= self.step().reboot;
        }
        reboot
    }

    /// Like [`Self::run`], with the desktop's one-second heartbeat, so the firmware's 4 s
    /// host-silence timeout does not end the session during a long hold.
    fn run_with_heartbeat(&mut self, ms: u32) -> bool {
        let mut reboot = false;
        for i in 0..ms / 10 {
            if i % 100 == 99 {
                self.send(&Message::Ping(Ping { t_ms: i }));
            }
            self.clock.advance(10);
            reboot |= self.step().reboot;
        }
        reboot
    }

    fn switch(&mut self, down: bool) {
        let (a, b, _) = self.levels.0.get();
        self.levels.0.set((a, b, down));
    }

    fn turn(&mut self, direction: Direction) {
        let (_, _, sw) = self.levels.0.get();
        let phases = match direction {
            Direction::Cw => [(false, true), (true, true), (true, false), (false, false)],
            Direction::Ccw => [(true, false), (true, true), (false, true), (false, false)],
        };
        for (a, b) in phases {
            self.levels.0.set((a, b, sw));
            self.clock.advance(5);
            self.step();
        }
    }

    fn key(&mut self, index: usize, down: bool) {
        let mut keys = self.levels.1.get();
        keys[index] = down;
        self.levels.1.set(keys);
    }

    /// Queues captured switch edges without a runtime tick between them.
    fn queue_edges(&mut self, control: ControlId, edges: &[(bool, u32)]) {
        for &(down, at_ms) in edges {
            match control {
                ControlId::Button => self.switch(down),
                ControlId::ContextButton(index) => self.key(usize::from(index), down),
                _ => panic!("not a switch"),
            }
            self.levels
                .2
                .borrow_mut()
                .push(((&self.levels).sample(), at_ms));
        }
    }

    fn burst(&mut self, control: ControlId, edges: &[(bool, u32)], now: u32) -> Tick {
        self.queue_edges(control, edges);
        self.clock.advance(now.wrapping_sub(self.clock.now_ms()));
        self.step()
    }

    fn reconnect(&mut self, at_ms: u32) -> Tick {
        self.nonce += 1;
        self.clock.advance(at_ms.wrapping_sub(self.clock.now_ms()));
        self.send(&Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: BUTTONS,
            nonce: self.nonce,
        }));
        self.send(&Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: BUTTONS,
        }));
        self.step()
    }

    fn inputs(&mut self) -> Vec<(ControlId, InputKind)> {
        self.drain()
            .into_iter()
            .filter_map(|message| match message {
                Message::InputEvent(event) => {
                    assert_eq!(event.session, self.nonce);
                    Some((event.control, event.kind))
                }
                _ => None,
            })
            .collect()
    }

    /// Contextual-button events sent so far, as (index, kind).
    fn key_events(&mut self) -> Vec<(u8, InputKind)> {
        self.drain()
            .into_iter()
            .filter_map(|m| match m {
                Message::InputEvent(InputEvent {
                    control: ControlId::ContextButton(i),
                    kind,
                    ..
                }) => Some((i, kind)),
                _ => None,
            })
            .collect()
    }

    /// Holds contextual button `index` for `ms`, releases, and settles.
    fn key_for(&mut self, index: usize, ms: u32) -> bool {
        self.key(index, true);
        let mut reboot = self.run(ms);
        self.key(index, false);
        reboot |= self.run(100);
        reboot
    }

    fn drain(&mut self) -> Vec<Message> {
        let bytes = self.pipe.host_recv();
        let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
        bytes
            .split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .filter_map(|p| decode_message(p, &mut scratch, &[PROTOCOL_MAJOR]).ok())
            .map(|(_, m)| m)
            .collect()
    }

    fn button_kinds(&mut self) -> Vec<InputKind> {
        self.drain()
            .into_iter()
            .filter_map(|m| match m {
                Message::InputEvent(e) if e.control == ControlId::Button => {
                    assert_eq!(e.session, self.nonce);
                    Some(e.kind)
                }
                _ => None,
            })
            .collect()
    }

    /// Presses for `ms`, releases, and settles.
    fn press_for(&mut self, ms: u32) -> bool {
        self.switch(true);
        let mut reboot = self.run(ms);
        self.switch(false);
        reboot |= self.run(100);
        reboot
    }
}

#[test]
fn held_rotation_is_distinct_and_consumes_the_button_press() {
    for initial_hold in [100, 700] {
        let mut rig = Rig::new();
        rig.connect(M1.union(Capabilities::HELD_ROTATION_V1));
        rig.switch(true);
        rig.run(initial_hold);
        rig.turn(Direction::Cw);
        rig.turn(Direction::Ccw);
        rig.switch(false);
        rig.run(400);
        let inputs = rig.inputs();
        assert!(inputs.contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Cw))));
        assert!(inputs.contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Ccw))));
        assert!(inputs.contains(&(ControlId::PressedRotary, InputKind::GestureEnded)));
        assert!(!inputs
            .iter()
            .any(|(control, kind)| *control == ControlId::Rotary
                || (*control == ControlId::Button
                    && matches!(
                        kind,
                        InputKind::Press | InputKind::Hold | InputKind::DoublePress
                    ))));
        rig.turn(Direction::Cw);
        assert!(
            rig.inputs()
                .contains(&(ControlId::Rotary, InputKind::Detent(Direction::Cw))),
            "ordinary rotation works after release"
        );
    }
}

#[test]
fn held_rotation_never_turns_into_recovery_even_after_ten_seconds() {
    let mut rig = Rig::new();
    rig.connect(M1.union(Capabilities::HELD_ROTATION_V1));
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    assert!(!rig.run_with_heartbeat(11_000));
    rig.turn(Direction::Ccw);
    rig.switch(false);
    rig.run(100);
    let inputs = rig.inputs();
    assert!(inputs.contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Ccw))));
    assert!(!inputs
        .iter()
        .any(|(control, _)| *control == ControlId::Button));
}

#[test]
fn recovery_already_started_cannot_be_stolen_by_held_rotation() {
    let mut rig = Rig::new();
    rig.connect(M1.union(Capabilities::HELD_ROTATION_V1));
    rig.switch(true);
    assert!(!rig.run_with_heartbeat(2_100));
    rig.turn(Direction::Cw);
    assert!(rig.run_with_heartbeat(8_100));
    assert!(rig.inputs().is_empty());
}

#[test]
fn old_host_still_swallows_held_rotation_without_a_knob_hold_action() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.run(1_000);
    rig.switch(false);
    rig.run(100);
    assert!(rig.inputs().is_empty(), "knob Hold is never an action");
}

#[test]
fn held_rotation_without_button_capability_remains_inert() {
    let mut rig = Rig::new();
    rig.connect(Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::HELD_ROTATION_V1));
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.run(1_000);
    rig.switch(false);
    rig.run(100);
    assert!(rig.inputs().is_empty());
}

#[test]
fn an_existing_normal_rotation_cannot_become_a_held_rotation() {
    let mut rig = Rig::new();
    rig.connect(M1.union(Capabilities::HELD_ROTATION_V1));
    rig.turn(Direction::Cw);
    rig.switch(true);
    rig.run(50);
    rig.turn(Direction::Ccw);
    rig.run(300);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    let inputs = rig.inputs();
    assert!(inputs.contains(&(ControlId::Rotary, InputKind::Detent(Direction::Cw))));
    assert!(!inputs
        .iter()
        .any(|(c, _)| matches!(c, ControlId::PressedRotary | ControlId::Button)));
}

#[test]
fn an_existing_context_button_press_cannot_become_a_held_rotation() {
    let mut rig = Rig::new();
    rig.connect(
        M1.union(Capabilities::CONTEXT_BUTTONS_V1)
            .union(Capabilities::HELD_ROTATION_V1),
    );
    rig.key(0, true);
    rig.run(50);
    rig.switch(true);
    rig.run(50);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(50);
    rig.key(0, false);
    rig.run(100);
    let inputs = rig.inputs();
    assert_eq!(inputs, [(ControlId::ContextButton(0), InputKind::Press)]);
}

#[test]
fn a_detent_during_button_debounce_cannot_claim_held_rotation() {
    let mut rig = Rig::new();
    rig.connect(M1.union(Capabilities::HELD_ROTATION_V1));
    rig.switch(true);
    rig.step();
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    assert_eq!(rig.inputs(), [(ControlId::Button, InputKind::Press)]);
}

#[test]
fn prior_short_press_is_preserved_but_held_turn_does_not_double_press() {
    for held_ms in [50, 700] {
        let mut rig = Rig::new();
        rig.connect(
            M1.union(Capabilities::DOUBLE_PRESS_V1)
                .union(Capabilities::HELD_ROTATION_V1),
        );
        rig.press_for(100);
        rig.switch(true);
        rig.run(held_ms);
        rig.turn(Direction::Cw);
        rig.switch(false);
        rig.run(400);
        let inputs = rig.inputs();
        assert_eq!(
            inputs
                .iter()
                .filter(|(c, k)| *c == ControlId::Button && *k == InputKind::Press)
                .count(),
            1
        );
        assert!(!inputs.iter().any(|(c, k)| *c == ControlId::Button
            && matches!(k, InputKind::Hold | InputKind::DoublePress)));
        assert!(inputs.contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Cw))));
    }
}

#[test]
fn captured_input_loss_preserves_an_ordinary_press_but_blocks_claiming_it_for_rotation() {
    let mut rig = Rig::new();
    rig.connect(M1.union(Capabilities::HELD_ROTATION_V1));
    rig.switch(true);
    rig.run(100);
    rig.levels.3.set(1);
    rig.step();
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(400);
    assert_eq!(rig.inputs(), [(ControlId::Button, InputKind::Press)]);

    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Ccw);
    rig.switch(false);
    rig.run(400);
    assert!(rig
        .inputs()
        .contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Ccw))));
}

#[test]
fn captured_input_loss_during_held_turn_requires_a_fresh_press_after_reconnect() {
    let mut rig = Rig::new();
    let caps = M1.union(Capabilities::HELD_ROTATION_V1);
    rig.connect(caps);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    assert!(rig
        .inputs()
        .contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Cw))));
    rig.levels.3.set(1);
    assert!(rig.step().link_dropped);
    rig.nonce += 1;
    rig.connect(caps);
    rig.turn(Direction::Ccw);
    rig.switch(false);
    rig.run(400);
    assert!(
        rig.inputs().is_empty(),
        "the pre-boundary press must not act or replay"
    );
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Ccw);
    rig.switch(false);
    rig.run(400);
    assert!(rig
        .inputs()
        .contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Ccw))));
}

#[test]
fn a_new_session_while_still_pressed_cannot_replay_or_reboot_the_consumed_press() {
    let mut rig = Rig::new();
    let caps = M1.union(Capabilities::HELD_ROTATION_V1);
    rig.connect(caps);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    let _ = rig.inputs();
    rig.nonce += 1;
    rig.connect(caps);
    rig.turn(Direction::Ccw);
    assert!(!rig.run_with_heartbeat(10_100));
    for (a, b) in [(false, true), (true, true), (true, false)] {
        rig.levels.0.set((a, b, true));
        rig.clock.advance(5);
        rig.step();
    }
    rig.switch(false);
    rig.run(400);
    rig.levels.0.set((false, false, false));
    rig.step();
    assert!(
        rig.inputs().is_empty(),
        "a consumed press cannot act in a new session"
    );
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Ccw);
    rig.switch(false);
    rig.run(400);
    assert!(rig
        .inputs()
        .contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Ccw))));
}

#[test]
fn a_short_press_reaches_the_desktop_once() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.press_for(200);
    assert_eq!(rig.button_kinds(), [InputKind::Press]);
}

#[test]
fn knob_hold_never_emits_an_action() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.press_for(1_000);
    assert!(
        rig.button_kinds().is_empty(),
        "knob Hold is never an action"
    );
}

#[test]
fn button_input_stays_silent_without_its_capability() {
    let mut rig = Rig::new();
    rig.connect(Capabilities::PHYSICAL_INPUT_V1);
    rig.press_for(200);
    assert!(rig.button_kinds().is_empty());
}

#[test]
fn a_press_with_no_session_is_dropped_not_replayed_later() {
    let mut rig = Rig::new();
    rig.press_for(200);
    rig.connect(M1);
    rig.run(500);
    assert!(rig.button_kinds().is_empty());
}

#[test]
fn the_recovery_hold_reboots_after_ten_seconds_and_sends_no_action() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.switch(true);
    assert!(!rig.run_with_heartbeat(9_900), "not before ten seconds");
    // Rotation during the hold changes nothing (invariant 33): spin a full detent.
    for (a, b) in [(false, true), (true, true), (true, false), (false, false)] {
        rig.levels.0.set((a, b, true));
        rig.clock.advance(5);
        assert!(!rig.step().reboot);
    }
    assert!(rig.run(200), "reboot is requested at ten seconds");
    let messages = rig.drain();
    assert!(
        !messages.iter().any(|m| matches!(m, Message::InputEvent(_))),
        "no Press, Hold or detent escapes a recovery hold"
    );
    assert!(messages
        .iter()
        .any(|m| matches!(m, Message::Bye(bye) if bye.reason == ByeReason::Shutdown)));
}

#[test]
fn recovery_needs_no_desktop_at_all() {
    let mut rig = Rig::new();
    assert!(rig.press_for(10_100));
}

#[test]
fn releasing_inside_the_recovery_window_cancels_it() {
    let mut rig = Rig::new();
    rig.connect(M1);
    assert!(!rig.press_for(5_000));
    assert!(!rig.run(10_000));
    assert!(
        rig.button_kinds().is_empty(),
        "a cancelled recovery fires nothing"
    );
}

#[test]
fn a_press_that_starts_inside_a_rotary_gesture_fires_nothing() {
    let mut rig = Rig::new();
    rig.connect(M1);
    for (a, b) in [(false, true), (true, true), (true, false), (false, false)] {
        rig.levels.0.set((a, b, false));
        rig.clock.advance(5);
        rig.step();
    }
    rig.press_for(200);
    assert_eq!(rig.button_kinds(), Vec::<InputKind>::new());
    // Once the rotary gesture has ended, a press works again.
    rig.run(300);
    rig.press_for(200);
    assert_eq!(rig.button_kinds(), [InputKind::Press]);
}

#[test]
fn the_recovery_takeover_owns_the_panel_and_goes_away_on_release() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.run(100);
    rig.switch(true);
    rig.run(3_000);
    let during = hash_rgb565(rig.display.frame());
    rig.switch(false);
    rig.run(200);
    assert_ne!(hash_rgb565(rig.display.frame()), during);
}

#[test]
fn a_desk_status_from_the_session_changes_the_view_and_a_session_end_forgets_it() {
    let mut rig = Rig::new();
    rig.connect(M1);
    let system = DeskStatus {
        mode: DisplayMode::System,
        ..DeskStatus::UNKNOWN
    };

    // A status stamped with another session is ignored.
    rig.send(&Message::Status(Status {
        session: rig.nonce ^ 1,
        status: system,
    }));
    rig.run(50);
    let buddy = rig.display.frame().to_vec();

    rig.send(&Message::Status(Status {
        session: rig.nonce,
        status: system,
    }));
    rig.run(50);
    assert_ne!(
        rig.display.frame(),
        buddy.as_slice(),
        "the System view replaced the buddy"
    );
    let system_frame = rig.display.frame().to_vec();

    // A new session starts from the Buddy view with nothing known.
    rig.nonce += 1;
    rig.connect(M1);
    rig.run(50);
    assert_ne!(rig.display.frame(), system_frame.as_slice());
}

/// Whether anything is drawn in the legend rows under the keycap (y 209..236).
fn legend_drawn(frame: &[kivori_model::Rgb565]) -> bool {
    let rows = &frame[209 * 240..236 * 240];
    rows.iter().any(|p| *p != rows[0])
}

const BUTTONS: Capabilities = M1.union(Capabilities::CONTEXT_BUTTONS_V1);

#[test]
fn a_contextual_button_sends_press_and_hold_with_its_index() {
    let mut rig = Rig::new();
    rig.connect(BUTTONS);
    rig.key_for(0, 200);
    rig.key_for(2, 1_000);
    assert_eq!(
        rig.key_events(),
        [(0, InputKind::Press), (2, InputKind::Hold)]
    );
}

#[test]
fn contextual_buttons_stay_silent_without_their_capability() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.key_for(1, 200);
    assert!(rig.key_events().is_empty());
}

#[test]
fn a_contextual_button_never_reboots_and_a_long_hold_fires_nothing() {
    let mut rig = Rig::new();
    rig.connect(BUTTONS);
    assert!(
        !rig.key_for(1, 12_000),
        "only the encoder switch can reboot"
    );
    assert!(rig.key_events().is_empty());
}

#[test]
fn two_buttons_at_once_fire_nothing() {
    let mut rig = Rig::new();
    rig.connect(BUTTONS);
    rig.key(0, true);
    rig.key(1, true);
    rig.run(200);
    rig.key(0, false);
    rig.key(1, false);
    rig.run(100);
    assert!(rig.key_events().is_empty());

    // A button pressed while the encoder switch is down fires nothing either; the switch press
    // that owned input still does.
    rig.switch(true);
    rig.run(50);
    rig.key_for(2, 100);
    rig.switch(false);
    rig.run(100);
    assert!(rig.key_events().is_empty());
}

#[test]
fn the_recovery_hold_still_reboots_with_a_button_held() {
    let mut rig = Rig::new();
    rig.connect(BUTTONS);
    rig.key(0, true);
    rig.run(100);
    rig.switch(true);
    assert!(rig.run(10_100), "recovery is out-of-band (invariant 24)");
    assert!(!rig
        .drain()
        .iter()
        .any(|m| matches!(m, Message::InputEvent(_))));
}

#[test]
fn control_labels_show_only_when_negotiated_and_only_for_their_session() {
    let labels = |session| {
        Message::ControlLabels(ControlLabelsUpdate {
            session,
            labels: ControlLabels {
                rotate: MediaText::from_text("Volume"),
                press: MediaText::from_text("Play/Pause"),
                hold: MediaText::from_text("Mute"),
                ..ControlLabels::default()
            },
        })
    };
    let mut rig = Rig::new();

    // Not negotiated: the message stays inert.
    rig.connect(M1);
    rig.send(&labels(rig.nonce));
    rig.run(50);
    assert!(!legend_drawn(rig.display.frame()));

    rig.nonce += 1;
    rig.connect(M1.union(Capabilities::CONTROL_LABELS_V1));
    rig.send(&labels(rig.nonce ^ 1));
    rig.run(50);
    assert!(
        !legend_drawn(rig.display.frame()),
        "another session's labels"
    );
    rig.send(&labels(rig.nonce));
    rig.run(50);
    assert!(legend_drawn(rig.display.frame()));

    // A new session knows nothing until the desktop says it again.
    rig.nonce += 1;
    rig.connect(M1.union(Capabilities::CONTROL_LABELS_V1));
    rig.run(50);
    assert!(!legend_drawn(rig.display.frame()));
}

#[test]
fn with_double_press_negotiated_two_quick_presses_are_one_double_press() {
    let mut rig = Rig::new();
    rig.connect(M1.union(Capabilities::DOUBLE_PRESS_V1));
    rig.switch(true);
    rig.run(100);
    rig.switch(false);
    rig.run(100);
    rig.switch(true);
    rig.run(100);
    rig.switch(false);
    rig.run(500);
    assert_eq!(rig.button_kinds(), [InputKind::DoublePress]);

    // A lone press still fires once, after the double-press window.
    rig.press_for(100);
    rig.run(300);
    assert_eq!(rig.button_kinds(), [InputKind::Press]);
}

#[test]
fn without_double_press_a_press_is_sent_without_waiting() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.switch(true);
    rig.run(100);
    rig.switch(false);
    rig.run(40);
    assert_eq!(rig.button_kinds(), [InputKind::Press]);
}

#[test]
fn buffered_short_presses_reach_the_wire_for_every_switch() {
    for control in [
        ControlId::Button,
        ControlId::ContextButton(0),
        ControlId::ContextButton(1),
        ControlId::ContextButton(2),
    ] {
        let mut rig = Rig::new();
        rig.connect(BUTTONS);
        assert!(!rig.burst(control, &[(true, 100), (false, 200)], 230).reboot);
        assert_eq!(rig.inputs(), [(control, InputKind::Press)]);
        rig.run(500);
        assert!(rig.inputs().is_empty());
    }
}

#[test]
fn buffered_presses_with_a_stable_release_never_merge_into_hold() {
    for double in [false, true] {
        for control in [ControlId::Button, ControlId::ContextButton(1)] {
            let mut rig = Rig::new();
            rig.connect(if double {
                BUTTONS.union(Capabilities::DOUBLE_PRESS_V1)
            } else {
                BUTTONS
            });
            rig.burst(
                control,
                &[(true, 100), (false, 520), (true, 550), (false, 970)],
                1_000,
            );
            let expected = if double && control == ControlId::Button {
                vec![(control, InputKind::DoublePress)]
            } else {
                vec![(control, InputKind::Press), (control, InputKind::Press)]
            };
            assert_eq!(rig.inputs(), expected);
            rig.run(500);
            assert!(rig.inputs().is_empty());
        }
    }
}

#[test]
fn buffered_release_of_one_control_does_not_block_the_next_control() {
    for (first, second) in [
        (ControlId::ContextButton(0), ControlId::Button),
        (ControlId::Button, ControlId::ContextButton(0)),
        (ControlId::ContextButton(2), ControlId::ContextButton(0)),
    ] {
        let mut rig = Rig::new();
        rig.connect(BUTTONS);
        rig.burst(first, &[(true, 100), (false, 200)], 210);
        assert!(rig.inputs().is_empty());
        rig.burst(second, &[(true, 230), (false, 330)], 360);
        assert_eq!(
            rig.inputs(),
            [(first, InputKind::Press), (second, InputKind::Press)]
        );
    }
}

#[test]
fn a_buffered_release_cancels_recovery_and_never_becomes_hold() {
    let mut rig = Rig::new();
    rig.connect(BUTTONS);
    assert!(!rig.burst(ControlId::Button, &[(true, 100)], 2_100).reboot);
    // The loop catches up after the reboot deadline, but the captured release ended recovery.
    assert!(
        !rig.burst(ControlId::Button, &[(false, 2_200)], 10_200)
            .reboot
    );
    assert!(rig.inputs().is_empty());
}

#[test]
fn a_session_boundary_cancels_a_press_still_debouncing() {
    for control in [ControlId::Button, ControlId::ContextButton(1)] {
        let mut rig = Rig::new();
        rig.connect(BUTTONS);
        rig.burst(control, &[(true, 100)], 110);
        rig.nonce += 1;
        rig.connect(BUTTONS);
        rig.burst(control, &[(false, 200)], 230);
        assert!(rig.inputs().is_empty());
        rig.burst(control, &[(true, 300), (false, 400)], 430);
        assert_eq!(rig.inputs(), [(control, InputKind::Press)]);
    }
}

#[test]
fn reconnect_rejects_unconsumed_presses_from_before_or_at_the_boundary() {
    for control in [ControlId::Button, ControlId::ContextButton(1)] {
        for (down, up, boundary) in [(100, 200, 300), (100, 350, 300), (300, 400, 300)] {
            let mut rig = Rig::new();
            rig.connect(BUTTONS);
            rig.queue_edges(control, &[(true, down)]);
            if up <= boundary {
                rig.queue_edges(control, &[(false, up)]);
            }
            rig.reconnect(boundary);
            assert!(rig.inputs().is_empty());
            if up > boundary {
                rig.burst(control, &[(false, up)], up + 30);
                assert!(rig.inputs().is_empty());
            }
            rig.burst(control, &[(true, 500), (false, 600)], 630);
            assert_eq!(rig.inputs(), [(control, InputKind::Press)]);
        }
    }
}

#[test]
fn an_old_raw_down_can_still_recover_after_a_session_boundary() {
    let mut rig = Rig::new();
    rig.connect(BUTTONS);
    rig.queue_edges(ControlId::Button, &[(true, 100)]);
    assert!(!rig.reconnect(300).reboot);
    assert!(rig.run_with_heartbeat(10_100));
    assert!(rig.inputs().is_empty());
}

#[test]
fn session_boundary_and_buffered_presses_survive_uptime_wrap() {
    for control in [ControlId::Button, ControlId::ContextButton(1)] {
        for (boundary, old_down, old_up, down, up, now) in [
            (
                u32::MAX - 100,
                u32::MAX - 200,
                u32::MAX - 130,
                u32::MAX - 50,
                50,
                80,
            ),
            (120, u32::MAX - 20, 90, 200, 300, 330),
        ] {
            let mut rig = Rig::new();
            rig.connect(BUTTONS);
            rig.clock.advance(old_down - 10);
            rig.queue_edges(control, &[(true, old_down), (false, old_up)]);
            rig.reconnect(boundary);
            assert!(rig.inputs().is_empty());
            rig.burst(control, &[(true, down), (false, up)], now);
            assert_eq!(rig.inputs(), [(control, InputKind::Press)]);
        }
    }
}

/// The buddy wears the muted face only while the desktop says it is muted (issue #13).
#[test]
fn the_buddy_wears_the_muted_face_only_while_the_desktop_reports_mute() {
    let mut rig = Rig::new();
    rig.connect(M1);
    let send = |rig: &mut Rig, muted| {
        let session = rig.nonce;
        rig.send(&Message::Status(Status {
            session,
            status: DeskStatus {
                muted,
                ..DeskStatus::UNKNOWN
            },
        }));
        rig.run(300);
        // The mouth region (crop 108,139 24x13): the status row draws elsewhere.
        let frame = rig.display.frame();
        (139..152)
            .flat_map(|y| (108..132).map(move |x| frame[y * 240 + x]))
            .collect::<Vec<_>>()
    };
    // Connected but not yet told a state: the device shows Offline, which is never overridden.
    assert_eq!(rig.runtime.state(), CompanionState::Offline);
    let offline = send(&mut rig, None);
    assert_eq!(
        send(&mut rig, Some(true)),
        offline,
        "Offline keeps its face"
    );

    rig.send(&Message::SetState(SetState {
        desired: SendableState::Idle,
        at_ms: None,
    }));
    rig.run(1_000);
    assert_eq!(rig.runtime.state(), CompanionState::Idle);
    let unknown = send(&mut rig, None);
    let muted = send(&mut rig, Some(true));
    assert_ne!(muted, unknown, "muted zips the mouth");
    assert_eq!(send(&mut rig, Some(false)), unknown, "unmuting restores it");
}

// ---- the profile picker (knob switch held and turned) and the hold ring ----

const PICKER: Capabilities = BUTTONS
    .union(Capabilities::CONTROL_LABELS_V1)
    .union(Capabilities::PROFILE_PICKER_V1)
    .union(Capabilities::HOLD_RING_V1);

fn choices(rig: &Rig, generation: u8, selected: u8) -> Message {
    let mut c = ProfileChoices {
        generation,
        count: 2,
        selected,
        ..ProfileChoices::default()
    };
    c.names[0] = PickerName::from_text("Claude Code");
    c.names[1] = PickerName::from_text("Photoshop");
    Message::ProfileChoices(ProfileChoicesUpdate {
        session: rig.nonce,
        choices: c,
    })
}

fn picks(rig: &mut Rig) -> Vec<(u8, u8)> {
    rig.drain()
        .into_iter()
        .filter_map(|m| match m {
            Message::ProfilePick(p) => {
                assert_eq!(p.session, rig.nonce);
                Some((p.pick.generation, p.pick.index))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn one_held_detent_never_opens_the_picker_or_fires_a_hold_and_release_resets_it() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&choices(&rig, 1, 0));
    rig.run(50);
    for _ in 0..2 {
        rig.switch(true);
        rig.run(100);
        rig.turn(Direction::Cw);
        rig.run(1_000);
        rig.switch(false);
        rig.run(100);
        let sent = rig.drain();
        assert!(!sent
            .iter()
            .any(|m| matches!(m, Message::ProfilePick(_) | Message::InputEvent(_))));
    }
}

#[test]
fn turning_with_the_switch_held_picks_a_profile_and_sends_one_pick_on_release() {
    let mut rig = Rig::new();
    rig.connect(PICKER.union(Capabilities::HELD_ROTATION_V1));
    rig.send(&choices(&rig, 7, 0));
    rig.run(50);
    let before = rig.display.frame().to_vec();
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.run(100);
    assert_ne!(
        rig.display.frame(),
        before.as_slice(),
        "the picker owns the screen"
    );
    assert!(rig
        .drain()
        .iter()
        .all(|m| !matches!(m, Message::InputEvent(_))));
    rig.switch(false);
    rig.run(100);
    let sent = rig.drain();
    assert!(
        !sent.iter().any(|m| matches!(m, Message::InputEvent(_))),
        "the picker is the device's own: no PressedRotary, no Press, no Hold: {sent:?}"
    );
    let picked: Vec<_> = sent
        .iter()
        .filter_map(|m| match m {
            Message::ProfilePick(p) => Some((p.pick.generation, p.pick.index)),
            _ => None,
        })
        .collect();
    assert_eq!(
        picked,
        [(7, 2)],
        "the second pinned profile, from the list it was picked from"
    );
    // Ordinary rotation works again afterwards.
    rig.turn(Direction::Cw);
    assert!(rig
        .inputs()
        .contains(&(ControlId::Rotary, InputKind::Detent(Direction::Cw))));
}

#[test]
fn the_picker_stays_on_screen_until_the_desktop_confirms_and_then_goes() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&choices(&rig, 1, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(300);
    assert_eq!(picks(&mut rig), [(1, 1)]);
    let waiting = rig.display.frame().to_vec();
    // The desktop now runs the pick: the confirmation shows, then the buddy returns.
    rig.send(&choices(&rig, 1, 1));
    rig.run(100);
    assert_ne!(
        rig.display.frame(),
        waiting.as_slice(),
        "confirmed looks different"
    );
    rig.run(1_200);
    let home = rig.display.frame().to_vec();
    // Picker gone: the legend row is back (the picker has none).
    rig.send(&Message::ControlLabels(ControlLabelsUpdate {
        session: rig.nonce,
        labels: ControlLabels {
            rotate: MediaText::from_text("Volume"),
            press: MediaText::from_text("Play/Pause"),
            ..ControlLabels::default()
        },
    }));
    rig.run(100);
    assert_ne!(rig.display.frame(), home.as_slice());
    assert!(legend_drawn(rig.display.frame()));
}

#[test]
fn an_unanswered_pick_never_claims_success_and_the_real_state_returns() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&choices(&rig, 1, 0));
    rig.send(&Message::ControlLabels(ControlLabelsUpdate {
        session: rig.nonce,
        labels: ControlLabels {
            rotate: MediaText::from_text("Volume"),
            press: MediaText::from_text("Play/Pause"),
            ..ControlLabels::default()
        },
    }));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    assert_eq!(
        rig.display.pixel(120, 190),
        RING_DOT,
        "the picker is up, waiting"
    );
    // No answer: after the wait and the "not confirmed" line, the home view is back.
    assert!(!rig.run_with_heartbeat(3_000));
    assert!(legend_drawn(rig.display.frame()), "back on the real state");
    assert_ne!(rig.display.pixel(120, 190), RING_DOT, "the picker is gone");
}

#[test]
fn a_pick_from_an_older_list_is_not_confirmed_by_a_newer_one() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&choices(&rig, 1, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    assert_eq!(picks(&mut rig), [(1, 1)]);
    let waiting = rig.display.frame().to_vec();
    // The list changed meanwhile (generation 2 says index 1 is pinned): that is not our pick.
    rig.send(&choices(&rig, 2, 1));
    rig.run(50);
    // Still waiting, not confirmed (the frame may animate but must not be the confirmed look).
    let _ = waiting;
    assert_eq!(
        rig.display.pixel(120, 190),
        RING_DOT,
        "still waiting, not green: that list is not the one picked from"
    );
    rig.run(1_500);
    rig.run(1_300);
    assert!(rig
        .drain()
        .iter()
        .all(|m| !matches!(m, Message::ProfilePick(_))));
}

#[test]
fn the_picker_is_inert_unless_negotiated_and_a_new_session_forgets_it() {
    let mut rig = Rig::new();
    rig.connect(BUTTONS.union(Capabilities::CONTROL_LABELS_V1));
    rig.send(&choices(&rig, 1, 0));
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    assert!(picks(&mut rig).is_empty());
    // Negotiated, but the session ends while choosing: nothing is sent into the next session.
    rig.nonce += 1;
    rig.connect(PICKER);
    rig.send(&choices(&rig, 1, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.nonce += 1;
    rig.connect(PICKER);
    rig.switch(false);
    rig.run(200);
    assert!(picks(&mut rig).is_empty(), "a pick never crosses a session");
}

#[test]
fn a_switch_bounce_while_choosing_sends_no_pick_and_keeps_the_choice() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&choices(&rig, 7, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    // The raw switch chatters low for one 10 ms tick (under the 20 ms debounce) mid-turn.
    rig.switch(false);
    rig.run(10);
    rig.switch(true);
    rig.run(30);
    assert!(picks(&mut rig).is_empty(), "a bounce is not a release");
    // The picker is still choosing: the next detent moves on from entry 1, not from a new start.
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.run(50);
    assert!(picks(&mut rig).is_empty());
    rig.switch(false);
    rig.run(100);
    assert_eq!(
        picks(&mut rig),
        [(7, 2)],
        "exactly one pick, from the one choosing session"
    );
}

#[test]
fn losing_input_while_choosing_cancels_the_picker_but_keeps_the_link() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&choices(&rig, 7, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    let choosing = rig.display.frame().to_vec();
    rig.levels.3.set(1);
    let tick = rig.step();
    assert!(
        !tick.link_dropped,
        "the picker queued nothing: keep the link"
    );
    rig.run(100);
    assert_ne!(
        rig.display.frame(),
        choosing.as_slice(),
        "the picker is gone from the screen"
    );
    // Releasing sends no pick (the choice was cancelled), and the next press works as usual.
    rig.switch(false);
    rig.run(100);
    let sent = rig.drain();
    assert!(
        !sent.iter().any(|m| matches!(m, Message::ProfilePick(_))),
        "{sent:?}"
    );
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    assert_eq!(picks(&mut rig), [(7, 1)], "the same session still picks");
}

#[test]
fn holding_after_a_turn_past_two_seconds_never_starts_recovery_and_sends_one_pick() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&choices(&rig, 7, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    assert!(
        !rig.run_with_heartbeat(2_500),
        "a consumed press never reboots"
    );
    assert!(picks(&mut rig).is_empty(), "nothing is sent before release");
    rig.switch(false);
    rig.run(100);
    assert_eq!(picks(&mut rig), [(7, 1)]);
}

#[test]
fn a_new_picker_after_not_confirmed_starts_from_the_desktops_state() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&choices(&rig, 1, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    assert_eq!(picks(&mut rig), [(1, 1)]);
    // No answer: wait out "not confirmed" until the picker has closed.
    assert!(!rig.run_with_heartbeat(3_500));
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    assert_eq!(
        picks(&mut rig),
        [(1, 1)],
        "the desktop still runs Auto, so one step on is entry 1 again"
    );
}

fn ring_pixel(rig: &Rig) -> kivori_model::Rgb565 {
    // The centre of the ring: empty until the ring is drawn, then the growing dot.
    rig.display.pixel(120, 84)
}

const RING_DOT: kivori_model::Rgb565 = kivori_model::Rgb565::from_rgb888(0x4f, 0xc3, 0xf7);

fn knob_labels(rig: &Rig, hold: &str) -> Message {
    Message::ControlLabels(ControlLabelsUpdate {
        session: rig.nonce,
        labels: ControlLabels {
            rotate: MediaText::from_text("Volume"),
            press: MediaText::from_text("Play/Pause"),
            hold: MediaText::from_text(hold),
            ..ControlLabels::default()
        },
    })
}

#[test]
fn knob_hold_never_shows_a_ring_even_with_legacy_hold_labels() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&knob_labels(&rig, "Mute"));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    assert_ne!(ring_pixel(&rig), RING_DOT, "a quick tap shows no ring");
    rig.run(500);
    assert_ne!(ring_pixel(&rig), RING_DOT, "knob Hold never shows a ring");
    rig.run(500);
    assert_ne!(ring_pixel(&rig), RING_DOT, "full: the dot turns white");
    let ready = rig.display.frame().to_vec();
    // Recovery takes over at 2 s with its own screen.
    rig.run_with_heartbeat(1_100);
    assert_ne!(rig.display.frame(), ready.as_slice());
    rig.switch(false);
    rig.run(100);

    // An unbound knob Hold is not teased.
    rig.send(&knob_labels(&rig, ""));
    rig.run(50);
    rig.switch(true);
    rig.run(600);
    assert_ne!(ring_pixel(&rig), RING_DOT, "no Hold, no ring");
    rig.switch(false);
    rig.run(100);
}

#[test]
fn a_contextual_buttons_ring_needs_the_desktop_to_say_it_has_a_hold() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&knob_labels(&rig, "Mute"));
    rig.run(50);
    rig.key(1, true);
    rig.run(600);
    assert_ne!(ring_pixel(&rig), RING_DOT, "nothing known: not teased");
    rig.key(1, false);
    rig.run(100);
    rig.send(&Message::HoldActions(HoldActionsUpdate {
        session: rig.nonce,
        actions: HoldActions {
            buttons: [
                MediaText::default(),
                MediaText::from_text("Mic mute"),
                MediaText::default(),
            ],
        },
    }));
    rig.run(50);
    rig.key(1, true);
    rig.run(600);
    assert_eq!(ring_pixel(&rig), RING_DOT);
    rig.key(1, false);
    rig.run(100);
    // A button the desktop left empty has none.
    rig.key(0, true);
    rig.run(600);
    assert_ne!(ring_pixel(&rig), RING_DOT);
    rig.key(0, false);
    rig.run(100);
}

#[test]
fn two_held_detents_open_the_picker_without_a_knob_hold_ring() {
    let mut rig = Rig::new();
    rig.connect(PICKER);
    rig.send(&knob_labels(&rig, "Mute"));
    rig.send(&choices(&rig, 1, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(500);
    assert_ne!(ring_pixel(&rig), RING_DOT, "knob has no Hold ring");
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.run(50);
    assert_ne!(
        ring_pixel(&rig),
        RING_DOT,
        "the ring is gone: the press is the picker's"
    );
    rig.switch(false);
    rig.run(100);
    assert_eq!(picks(&mut rig), [(1, 1)]);
}

fn held_turn(rig: &Rig, picker: bool) -> Message {
    Message::HeldTurnMode(HeldTurnModeUpdate {
        session: rig.nonce,
        picker,
    })
}

#[test]
fn knob_hold_is_sent_and_rings_only_with_the_capability_and_a_label() {
    let caps = M1
        .union(Capabilities::KNOB_HOLD_V1)
        .union(Capabilities::CONTROL_LABELS_V1);
    let mut rig = Rig::new();
    rig.connect(caps);
    rig.send(&knob_labels(&rig, ""));
    rig.run(50);
    rig.switch(true);
    rig.run(600);
    assert_ne!(
        ring_pixel(&rig),
        RING_DOT,
        "an empty Hold label draws no ring"
    );
    rig.switch(false);
    rig.run(100);
    assert_eq!(rig.button_kinds(), [InputKind::Press]);

    rig.send(&knob_labels(&rig, "Mute"));
    rig.run(50);
    rig.switch(true);
    rig.run(600);
    assert_eq!(ring_pixel(&rig), RING_DOT, "a labelled Hold shows the ring");
    rig.switch(false);
    rig.run(100);
    assert_eq!(
        rig.button_kinds(),
        [InputKind::Press],
        "released inside the first second"
    );

    rig.press_for(1_200);
    assert_eq!(rig.button_kinds(), [InputKind::Hold]);
}

#[test]
fn a_held_turn_or_recovery_never_also_sends_knob_hold() {
    let caps = PICKER
        .union(Capabilities::KNOB_HOLD_V1)
        .union(Capabilities::HELD_TURN_MODE_V1);
    let mut rig = Rig::new();
    rig.connect(caps);
    rig.send(&knob_labels(&rig, "Mute"));
    rig.send(&choices(&rig, 1, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(400);
    assert_eq!(ring_pixel(&rig), RING_DOT);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.run(800);
    assert_ne!(ring_pixel(&rig), RING_DOT, "the picker owns the press");
    rig.switch(false);
    rig.run(100);
    let sent = rig.drain();
    assert!(sent.iter().any(|m| matches!(m, Message::ProfilePick(_))));
    assert!(
        !sent.iter().any(|m| matches!(
            m,
            Message::InputEvent(e) if e.kind == InputKind::Hold
        )),
        "{sent:?}"
    );

    rig.switch(true);
    assert!(
        !rig.run_with_heartbeat(2_100),
        "recovery at 2 s does not reboot"
    );
    rig.switch(false);
    rig.run(100);
    assert!(rig.button_kinds().is_empty(), "recovery cancels the Hold");
}

#[test]
fn held_turn_mode_defaults_to_the_picker_and_can_switch_both_ways() {
    let caps = PICKER
        .union(Capabilities::HELD_TURN_MODE_V1)
        .union(Capabilities::KNOB_HOLD_V1);
    let mut rig = Rig::new();
    rig.connect(caps);
    rig.send(&choices(&rig, 1, 0));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    let sent = rig.drain();
    assert!(sent.iter().any(|m| matches!(m, Message::ProfilePick(_))));
    assert!(
        !sent.iter().any(|m| matches!(m, Message::InputEvent(_))),
        "default mode is the picker: {sent:?}"
    );

    rig.send(&held_turn(&rig, false));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Ccw);
    rig.switch(false);
    rig.run(400);
    let sent = rig.drain();
    let inputs: Vec<_> = sent
        .iter()
        .filter_map(|m| match m {
            Message::InputEvent(e) => Some((e.control, e.kind)),
            _ => None,
        })
        .collect();
    assert!(inputs.contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Cw))));
    assert!(inputs.contains(&(ControlId::PressedRotary, InputKind::Detent(Direction::Ccw))));
    assert!(!inputs.iter().any(|(control, kind)| {
        *control == ControlId::Button
            && matches!(
                kind,
                InputKind::Press | InputKind::Hold | InputKind::DoublePress
            )
    }));
    assert!(
        !sent.iter().any(|m| matches!(m, Message::ProfilePick(_))),
        "non-picker mode opens no picker"
    );

    rig.send(&held_turn(&rig, true));
    rig.run(50);
    rig.switch(true);
    rig.run(100);
    rig.turn(Direction::Cw);
    rig.turn(Direction::Cw);
    rig.switch(false);
    rig.run(100);
    let sent = rig.drain();
    assert!(sent.iter().any(|m| matches!(m, Message::ProfilePick(_))));
    assert!(!sent.iter().any(|m| matches!(m, Message::InputEvent(_))));
}
