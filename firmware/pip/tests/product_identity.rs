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

/// The numeric value of `key = <n>` inside `[section]` of the products registry.
fn registry_number(section: &str, key: &str) -> u16 {
    let registry = include_str!("../../../products/registry.toml");
    let mut current = "";
    for line in registry.lines() {
        let line = line.split('#').next().unwrap().trim();
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = header;
        } else if let (true, Some((k, v))) = (current == section, line.split_once('=')) {
            if k.trim() == key {
                return v.trim().parse().expect("registry number");
            }
        }
    }
    panic!("{key} is not in [{section}] of products/registry.toml");
}

#[test]
fn this_build_is_pip_on_the_rp2040_zero_in_the_registry_and_in_its_image_record() {
    use kivori_firmware::identity::{BUILD_BOARD, BUILD_PRODUCT, IMAGE_RECORD};

    let (pip, zero) = (
        registry_number("products", "pip"),
        registry_number("boards", "rp2040-zero"),
    );
    assert_eq!((BUILD_PRODUCT, BUILD_BOARD), (pip, zero));
    let build = ProductIdentity::BUILD;
    assert_eq!(
        (build.product, build.board, build.hw_rev),
        (pip, zero, None)
    );
    assert_eq!(build.source, IdentitySource::Build);

    // `KIVORI-FW-IDENTITY:p=<product>;b=<board>;v=<x.y.z>;m=<major.minor>\0`, one NUL at the very end:
    // the updater searches the image for it, and a second NUL would cut the text short.
    let text = IMAGE_RECORD.strip_suffix('\0').expect("NUL-terminated");
    assert!(text.is_ascii() && !text.contains('\0'));
    let fields: Vec<&str> = text
        .strip_prefix("KIVORI-FW-IDENTITY:")
        .expect("marker prefix")
        .split(';')
        .collect();
    assert_eq!(
        fields,
        [
            format!("p={pip}"),
            format!("b={zero}"),
            format!("v={}", env!("CARGO_PKG_VERSION")),
            format!("m={}", env!("KIVORI_MIN_PROTOCOL")),
        ]
    );
}

#[test]
fn the_rp2040_chip_is_never_provisioned_so_the_unit_runs_and_reports_the_build() {
    use kivori_firmware::identity::check_boot;
    use kivori_protocol::identity::ChipRecordState;

    // What `physical_st7789::run_mode` passes: the RP2040 has no one-time memory for a record.
    assert_eq!(
        check_boot(ProductIdentity::BUILD, ChipRecordState::NotProvisioned),
        BootCheck::Run(ProductIdentity::BUILD)
    );
    // A chip record naming the other registered board is the wrong-board path, reporting the
    // chip's own numbers (not the build's), so the app can offer the right firmware.
    let other = registry_number("boards", "c3-supermini");
    assert_ne!(other, ProductIdentity::BUILD.board);
    let record = kivori_protocol::identity::ChipRecord {
        product: ProductIdentity::BUILD.product,
        board: other,
        hw_rev: Some(1),
    };
    let BootCheck::WrongBoard(reported) =
        check_boot(ProductIdentity::BUILD, ChipRecordState::Valid(record))
    else {
        panic!("another board's record must stop the product");
    };
    assert_eq!(
        (reported.board, reported.source),
        (other, IdentitySource::Chip)
    );
}

/// The text inside the first `"..."` after `needle` in `source`.
fn quoted_after<'a>(source: &'a str, needle: &str) -> &'a str {
    let rest = &source[source
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` not found"))..];
    let start = rest.find('"').expect("opening quote") + 1;
    &rest[start..start + rest[start..].find('"').expect("closing quote")]
}

#[test]
fn the_usb_ids_and_names_the_transport_enumerates_with_are_the_catalogs() {
    // `transport.rs` only builds for the device, so read it as text, like the pin-map test reads
    // `profile.rs`. The catalog's `usb` hint is what the app nominates serial ports with.
    let transport = include_str!("../src/transport.rs");
    let ids = transport
        .split("UsbVidPid(")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .expect("UsbVidPid in transport.rs");
    let (vid, pid) = ids.split_once(',').expect("vid, pid");
    let from_source = format!(
        "{}:{}",
        vid.trim().trim_start_matches("0x").to_uppercase(),
        pid.trim().trim_start_matches("0x").to_uppercase()
    );
    let catalog = include_str!("../../../products/pip/product.toml");
    let board = &catalog[catalog.find("[boards.rp2040-zero]").expect("board table")..];
    assert_eq!(quoted_after(board, "usb = ["), from_source);
    // The product string a person sees in the OS serial-port list.
    assert_eq!(
        quoted_after(transport, ".product("),
        quoted_after(catalog, "display_name = ")
    );
}

#[test]
fn the_firmware_advertises_every_capability_the_catalog_requires_of_a_pip() {
    use kivori_firmware::profile::physical_st7789::CAPABILITIES;

    let catalog = include_str!("../../../products/pip/product.toml");
    let list = &catalog[catalog.find("required_capabilities = [").expect("list")..];
    let list = &list[..list.find(']').expect("end of list")];
    let mut count = 0;
    for name in list.split('"').skip(1).step_by(2) {
        let flag = match name {
            "MASCOT_INTERACTION" => Capabilities::MASCOT_INTERACTION,
            "PHYSICAL_INPUT_V1" => Capabilities::PHYSICAL_INPUT_V1,
            "PRESENTATION_V1" => Capabilities::PRESENTATION_V1,
            "BUTTON_INPUT_V1" => Capabilities::BUTTON_INPUT_V1,
            "DESK_STATUS_V1" => Capabilities::DESK_STATUS_V1,
            "ACTION_FEEDBACK_V1" => Capabilities::ACTION_FEEDBACK_V1,
            "DOUBLE_PRESS_V1" => Capabilities::DOUBLE_PRESS_V1,
            "MEDIA_INFO_V1" => Capabilities::MEDIA_INFO_V1,
            "CONTROL_LABELS_V1" => Capabilities::CONTROL_LABELS_V1,
            "CONTEXT_BUTTONS_V1" => Capabilities::CONTEXT_BUTTONS_V1,
            "HOST_TAKEOVERS_V1" => Capabilities::HOST_TAKEOVERS_V1,
            other => panic!("catalog requires {other}: add it to this test"),
        };
        assert!(CAPABILITIES.contains(flag), "the firmware lacks {name}");
        count += 1;
    }
    assert_eq!(count, 11);
}
