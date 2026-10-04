//! M1 on the real runtime over the wire: push-switch Press / Hold, the recovery hold, and the
//! session-scoped desk status. Host simulation only — physical switch timing is a hardware row.

use std::cell::Cell;

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::InputSource;
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig, Tick};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_framebuffer::hash_rgb565;
use kivori_model::desk::{ControlLabels, DeskStatus, DisplayMode, MediaText};
use kivori_model::input::InputLevels;
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, ByeReason, ControlId, ControlLabelsUpdate, FirmwareVersion,
    Hello, InputEvent, InputKind, Message, Nonce, Ready, Status, MAX_FRAME, MAX_WIRE,
    PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const M1: Capabilities = Capabilities::PHYSICAL_INPUT_V1
    .union(Capabilities::BUTTON_INPUT_V1)
    .union(Capabilities::DESK_STATUS_V1)
    .union(Capabilities::ACTION_FEEDBACK_V1);

/// Levels the test sets between steps.
#[derive(Default)]
struct Levels(Cell<(bool, bool, bool)>, Cell<[bool; 3]>);

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
            device_id: [0x42; 16],
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
                .union(Capabilities::CONTEXT_BUTTONS_V1),
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

    fn switch(&mut self, down: bool) {
        let (a, b, _) = self.levels.0.get();
        self.levels.0.set((a, b, down));
    }

    fn key(&mut self, index: usize, down: bool) {
        let mut keys = self.levels.1.get();
        keys[index] = down;
        self.levels.1.set(keys);
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
fn a_short_press_reaches_the_desktop_once() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.press_for(200);
    assert_eq!(rig.button_kinds(), [InputKind::Press]);
}

#[test]
fn a_hold_is_sent_as_hold_and_never_also_as_press() {
    let mut rig = Rig::new();
    rig.connect(M1);
    rig.press_for(1_000);
    assert_eq!(rig.button_kinds(), [InputKind::Hold]);
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
    assert!(!rig.run(9_900), "not before ten seconds");
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
