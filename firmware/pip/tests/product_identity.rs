//! Product identity on the real runtime (ADR-0014, ADR-0016 tier A): the `Identity` message right
//! after a handshake that negotiated it, and the wrong-board boot check (no product, a static
//! screen, a link that only offers identity, recovery hold still working). Host simulation only.

use std::cell::Cell;

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::identity::{BootCheck, BuildId, ProductIdentity};
use kivori_firmware::ports::InputSource;
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig, Tick};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_model::input::InputLevels;
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, IdentitySource, Message, Nonce, Ready,
    MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const CAPS: Capabilities = Capabilities::PHYSICAL_INPUT_V1
    .union(Capabilities::BUTTON_INPUT_V1)
    .union(Capabilities::CONTEXT_BUTTONS_V1)
    .union(Capabilities::PRODUCT_IDENTITY_V1)
    .union(Capabilities::BUILD_INFO_V1);

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
    fn new(boot: Option<BootCheck>) -> Self {
        let bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
        let identity = DeviceIdentity {
            device_id: [0x42; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            capabilities: CAPS,
        };
        let mut runtime = Runtime::new(identity, RuntimeConfig::default());
        if let Some(boot) = boot {
            runtime.set_boot_check(boot);
        }
        let mut rig = Self {
            runtime,
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

    /// Handshakes and returns what the device sent back (`HelloAck`, `Identity`, ...).
    fn connect(&mut self, caps: Capabilities) -> Vec<Message> {
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
        self.drain()
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
}

fn identities(messages: &[Message]) -> Vec<kivori_protocol::Identity> {
    messages
        .iter()
        .filter_map(|m| match m {
            Message::Identity(identity) => Some(*identity),
            _ => None,
        })
        .collect()
}

const OTHER: ProductIdentity = ProductIdentity {
    product: 7,
    board: 3,
    hw_rev: Some(2),
    source: IdentitySource::Chip,
};

#[test]
fn identity_follows_the_handshake_when_negotiated() {
    let mut rig = Rig::new(None);
    let sent = identities(&rig.connect(CAPS));
    assert_eq!(
        sent,
        vec![kivori_protocol::Identity {
            session: rig.nonce,
            product: ProductIdentity::BUILD.product,
            board: ProductIdentity::BUILD.board,
            hw_rev: None,
            source: IdentitySource::Build,
        }]
    );
}

#[test]
fn a_provisioned_chip_is_reported_with_its_revision_and_source() {
    let chip = ProductIdentity {
        hw_rev: Some(2),
        source: IdentitySource::Chip,
        ..ProductIdentity::BUILD
    };
    let mut rig = Rig::new(Some(BootCheck::Run(chip)));
    let sent = identities(&rig.connect(CAPS));
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0].hw_rev, sent[0].source),
        (Some(2), IdentitySource::Chip)
    );
}

#[test]
fn no_identity_without_the_negotiated_capability() {
    let mut rig = Rig::new(None);
    let without = CAPS.intersection(Capabilities::PHYSICAL_INPUT_V1);
    assert!(identities(&rig.connect(without)).is_empty());
}

#[test]
fn a_wrong_board_offers_only_identity_and_reports_the_chip() {
    let mut rig = Rig::new(Some(BootCheck::WrongBoard(OTHER)));
    let sent = rig.connect(CAPS);
    let ack = sent
        .iter()
        .find_map(|m| match m {
            Message::HelloAck(ack) => Some(*ack),
            _ => None,
        })
        .expect("a HelloAck: the link stays up");
    assert_eq!(ack.device_caps, Capabilities::PRODUCT_IDENTITY_V1);
    let ids = identities(&sent);
    assert_eq!(ids.len(), 1);
    assert_eq!(
        (ids[0].product, ids[0].board, ids[0].hw_rev, ids[0].source),
        (7, 3, Some(2), IdentitySource::Chip)
    );
}

#[test]
fn a_wrong_board_shows_a_static_screen_that_differs_from_the_product() {
    let mut normal = Rig::new(None);
    normal.run(500);
    let product = normal.display.frame().to_vec();
    let mut wrong = Rig::new(Some(BootCheck::WrongBoard(OTHER)));
    wrong.run(500);
    let screen = wrong.display.frame().to_vec();
    assert_ne!(screen, product);
    wrong.run(2_000);
    assert_eq!(
        wrong.display.frame(),
        &screen[..],
        "static: nothing animates"
    );
}

#[test]
fn the_recovery_hold_still_works_on_a_wrong_board() {
    let mut rig = Rig::new(Some(BootCheck::WrongBoard(OTHER)));
    rig.run(500);
    let screen = rig.display.frame().to_vec();
    rig.levels.0.set((false, false, true));
    assert!(!rig.run(5_000), "not yet");
    assert_ne!(rig.display.frame(), &screen[..], "recovery owns the screen");
    assert!(rig.run(6_000), "the hold completes and asks for a reboot");
}

fn builds(messages: &[Message]) -> Vec<kivori_protocol::BuildInfo> {
    messages
        .iter()
        .filter_map(|m| match m {
            Message::BuildInfo(build) => Some(*build),
            _ => None,
        })
        .collect()
}

#[test]
fn the_build_source_id_and_dirty_flag_follow_the_identity_when_negotiated() {
    let mut rig = Rig::new(None);
    let messages = rig.connect(CAPS);
    assert_eq!(
        builds(&messages),
        vec![kivori_protocol::BuildInfo {
            session: rig.nonce,
            source_id: BuildId::BUILD.source_id,
            dirty: BuildId::BUILD.dirty,
        }]
    );
    let order: Vec<_> = messages
        .iter()
        .filter(|m| matches!(m, Message::Identity(_) | Message::BuildInfo(_)))
        .collect();
    assert!(matches!(
        order[..],
        [Message::Identity(_), Message::BuildInfo(_)]
    ));
}

#[test]
fn an_older_desktop_never_receives_the_build_info() {
    // A desktop from before the capability: it offers everything but bit 16, so negotiation drops it.
    let mut rig = Rig::new(None);
    let messages = rig.connect(CAPS.intersection(Capabilities::PRODUCT_IDENTITY_V1));
    assert!(builds(&messages).is_empty());
    assert_eq!(identities(&messages).len(), 1);
}

#[test]
fn a_hostile_ready_cannot_switch_the_build_info_on() {
    // `Ready` claims the capability, but the `Hello` never offered it.
    let mut rig = Rig::new(None);
    rig.send(&Message::Hello(Hello {
        desktop_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        desktop_caps: Capabilities::PRODUCT_IDENTITY_V1,
        nonce: rig.nonce,
    }));
    rig.step();
    rig.send(&Message::Ready(Ready {
        negotiated_minor: PROTOCOL_MINOR,
        negotiated_caps: CAPS,
    }));
    rig.step();
    assert!(builds(&rig.drain()).is_empty());
}

#[test]
fn a_wrong_board_unit_offers_only_identity_so_no_build_info() {
    let mut rig = Rig::new(Some(BootCheck::WrongBoard(OTHER)));
    let messages = rig.connect(CAPS);
    assert_eq!(identities(&messages).len(), 1);
    assert!(builds(&messages).is_empty());
}
