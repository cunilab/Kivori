//! Recovery target fixtures never enter shipping discovery or perform device I/O.
use kivori_desktop::activity::ActivityEventKind;
use kivori_desktop::catalog::Catalog;
use kivori_desktop::device::discovery::PortCandidate;
use kivori_desktop::firmware::{FirmwarePhase, FlashFailure, FlashWorkflow};
use kivori_desktop::firmware_bundle::FirmwareBundle;
use kivori_desktop::identity::{
    decide_recovery, decide_recovery_for_port, DeviceIdentity, IdentitySource, RecoveryDecision,
    RecoveryRefusal, RestoreOption, WrongProduct,
};

const USB_A: (u16, u16) = (0x1234, 1);
const USB_B: (u16, u16) = (0x1234, 2);

fn board(id: &str, usb: &str) -> String {
    format!("[boards.{id}]\ndisplay_name = '{id}'\nflash = 'espflash'\nusb = ['{usb}']\n[boards.{id}.firmware]\ncrate = 'firmware/fixture'\nartifact = 'image'\nmin_protocol = '0.1'\n")
}

fn product(name: &str, boards: &str) -> String {
    format!("display_name = '{name}'\nshort_name = '{name}'\nmodel = 'KV-T1'\nrequired_capabilities = []\n{boards}")
}

fn catalog(first_boards: &str, second_usb: &str) -> Catalog {
    Catalog::parse(
        "[products]\nfirst = 41\nsecond = 42\n[boards]\nboard-a = 9\nboard-b = 10\n",
        &[
            ("first", &product("First", first_boards)),
            ("second", &product("Second", &board("board-a", second_usb))),
        ],
    )
    .unwrap()
}

#[test]
fn policy_refusals_clear_old_choices_and_never_reserve_or_start_a_flash() {
    for (reason, expected) in [(RecoveryRefusal::NoCandidate, FlashFailure::Unknown)] {
        let mut flash = FlashWorkflow::new(true, 512);
        flash.fail_wrong_product(WrongProduct {
            device: "Old device".into(),
            image: "Old image".into(),
        });
        flash.ask_restore_choice(vec![RestoreOption {
            id: "old".into(),
            name: "Old choice".into(),
            board: "board-a".into(),
            board_name: "board-a".into(),
        }]);
        flash.drain_activity();
        flash.refuse_recovery(reason);
        let status = flash.status();
        assert_eq!(status.phase, FirmwarePhase::Failed);
        assert_eq!(status.failure, Some(expected));
        assert_eq!(status.wrong_product, None);
        assert!(status.restore_choices.is_empty());
        assert!(!flash.is_busy());
        assert_eq!(
            flash
                .drain_activity()
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [ActivityEventKind::FirmwarePreparationRejected]
        );
        // A later verified attempt starts clean.
        flash
            .request(true, Some("fixture-port"), Some("fixture-hash"))
            .unwrap();
        assert!(flash.is_busy());
        assert_eq!(flash.status().failure, None);
    }
}

fn target(product: u16, board: u16) -> RecoveryDecision {
    RecoveryDecision::Proceed { product, board }
}

fn device(product: u16, board: u16) -> DeviceIdentity {
    DeviceIdentity {
        product,
        board,
        hw_rev: None,
        source: IdentitySource::Chip,
    }
}

fn image(product: u16, board: u16) -> Vec<u8> {
    let mut bytes = vec![0; 52];
    bytes[..6].copy_from_slice(b"\x7fELF\x01\x01");
    bytes[18] = 243;
    bytes.extend_from_slice(
        format!(
            "KIVORI-FW-VERSION:0.1.0\0KIVORI-FW-IDENTITY:p={product};b={board};v=0.1.0;m=0.1\0"
        )
        .as_bytes(),
    );
    bytes
}

#[test]
fn shared_usb_still_requires_a_choice_when_only_one_products_image_exists() {
    let catalog = catalog(&board("board-a", "1234:0001"), "1234:0001");
    let second = image(42, 9);
    let bundle = FirmwareBundle::parse(&[&second]).unwrap();
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), None, None, None),
        RecoveryDecision::Choose(vec![
            RestoreOption {
                id: "first".into(),
                name: "First".into(),
                board: "board-a".into(),
                board_name: "board-a".into(),
            },
            RestoreOption {
                id: "second".into(),
                name: "Second".into(),
                board: "board-a".into(),
                board_name: "board-a".into(),
            },
        ])
    );
    // Hardware selection succeeds independently; the missing target image cannot fall back to B.
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), Some("first"), None, None),
        target(41, 9)
    );
    assert!(bundle.get(41, 9).is_none());
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), Some("second"), None, None),
        target(42, 9)
    );
    assert_eq!(bundle.get(42, 9).unwrap().bytes(), second);
}

#[test]
fn choices_cannot_override_usb_candidates_or_a_reported_identity() {
    let catalog = catalog(&board("board-a", "1234:0001"), "1234:0002");
    for (usb, chosen) in [
        (Some((0x9999, 1)), None),
        (Some(USB_A), Some("second")),
        (Some(USB_A), Some("not-a-product")),
        (Some(USB_A), Some("")),
    ] {
        assert_eq!(
            decide_recovery(&catalog, usb, chosen, None, None),
            RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
        );
    }
    assert_eq!(
        decide_recovery(&catalog, Some(USB_B), Some("second"), None, None),
        target(42, 9)
    );
    assert_eq!(
        decide_recovery(
            &catalog,
            Some(USB_A),
            Some("first"),
            None,
            Some(&device(42, 9))
        ),
        RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
    );
    // The live identity wins over USB hints, even shared/wrong hints.
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), None, None, Some(&device(42, 9))),
        target(42, 9)
    );
    for unknown in [device(99, 9), device(41, 99)] {
        assert_eq!(
            decide_recovery(&catalog, Some(USB_A), None, None, Some(&unknown)),
            RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
        );
    }
}

#[test]
fn shared_board_hints_are_ambiguous_even_with_only_one_board_image() {
    let boards = board("board-a", "1234:0001") + &board("board-b", "1234:0001");
    let catalog = catalog(&boards, "1234:0002");
    let bytes = image(41, 9);
    let bundle = FirmwareBundle::parse(&[&bytes]).unwrap();
    assert!(bundle.get(41, 9).is_some());
    for chosen in [None, Some("first")] {
        assert_eq!(
            decide_recovery(&catalog, Some(USB_A), chosen, None, None),
            RecoveryDecision::Choose(vec![
                RestoreOption {
                    id: "first".into(),
                    name: "First".into(),
                    board: "board-a".into(),
                    board_name: "board-a".into()
                },
                RestoreOption {
                    id: "first".into(),
                    name: "First".into(),
                    board: "board-b".into(),
                    board_name: "board-b".into()
                },
            ])
        );
    }
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), None, None, Some(&device(41, 10))),
        target(41, 10)
    );
    assert!(bundle.get(41, 10).is_none());
}

#[test]
fn distinct_board_hints_select_exact_images_from_a_reversed_bundle() {
    let boards = board("board-a", "1234:0001") + &board("board-b", "1234:0002");
    let catalog = catalog(&boards, "1234:0003");
    let a = image(41, 9);
    let b = image(41, 10);
    let bundle = FirmwareBundle::parse(&[&b, &a]).unwrap();
    for (usb, expected, bytes) in [(USB_A, 9, &a), (USB_B, 10, &b)] {
        let decision = decide_recovery(&catalog, Some(usb), Some("first"), None, None);
        assert_eq!(decision, target(41, expected));
        let RecoveryDecision::Proceed { product, board } = decision else {
            unreachable!()
        };
        assert_eq!(bundle.get(product, board).unwrap().bytes(), bytes);
    }
}

#[test]
fn recovery_binds_live_identity_and_pending_state_to_the_only_selected_port() {
    let catalog = catalog(&board("board-a", "1234:0001"), "1234:0001");
    let port = PortCandidate {
        port_name: "fixture-port".into(),
        vid: Some(USB_A.0),
        pid: Some(USB_A.1),
    };
    let other = PortCandidate {
        port_name: "other-port".into(),
        ..port.clone()
    };
    let id = device(42, 9);
    let resolve = |ports: &[&PortCandidate], live, pending| {
        decide_recovery_for_port(&catalog, ports, None, None, live, Some(&id), pending)
    };
    assert_eq!(
        resolve(&[&port], Some("fixture-port"), false),
        target(42, 9)
    );
    assert!(matches!(
        resolve(&[&port], Some("other-port"), false),
        RecoveryDecision::Choose(_)
    ));
    assert_eq!(
        resolve(&[&port], Some("fixture-port"), true),
        RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
    );
    assert_eq!(
        resolve(&[], None, false),
        RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
    );
    assert_eq!(
        resolve(&[&port, &other], Some("fixture-port"), false),
        RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
    );
    let unknown_usb = PortCandidate {
        vid: None,
        ..port.clone()
    };
    assert_eq!(
        resolve(&[&unknown_usb], None, false),
        RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
    );
}

#[test]
fn explicit_pairs_resolve_board_ambiguity_without_using_image_availability() {
    let boards = board("board-a", "1234:0001") + &board("board-b", "1234:0001");
    let catalog = catalog(&boards, "1234:0002");
    for (chosen, expected) in [("board-a", 9), ("board-b", 10)] {
        assert_eq!(
            decide_recovery(&catalog, Some(USB_A), Some("first"), Some(chosen), None),
            target(41, expected)
        );
    }
    for (product, board) in [
        (None, Some("board-a")),
        (Some("first"), Some("missing")),
        (Some("second"), Some("board-b")),
    ] {
        assert_eq!(
            decide_recovery(&catalog, Some(USB_A), product, board, None),
            RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
        );
    }
    assert_eq!(
        decide_recovery(
            &catalog,
            Some(USB_A),
            Some("first"),
            Some("board-a"),
            Some(&device(41, 10))
        ),
        RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
    );
    assert_eq!(
        decide_recovery(&catalog, Some(USB_B), Some("first"), Some("board-b"), None),
        RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
    );
}
