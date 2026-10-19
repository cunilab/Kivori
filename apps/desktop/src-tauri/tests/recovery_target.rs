//! Recovery target fixtures never enter shipping discovery or perform device I/O.
use kivori_desktop::activity::ActivityEventKind;
use kivori_desktop::catalog::Catalog;
use kivori_desktop::firmware::{FirmwarePhase, FlashFailure, FlashWorkflow};
use kivori_desktop::firmware_marker::ImageIdentity;
use kivori_desktop::identity::{
    decide_recovery, RecoveryDecision, RecoveryRefusal, RestoreOption, WrongProduct,
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

fn image(product: u16, board: u16) -> ImageIdentity {
    ImageIdentity {
        product,
        board,
        version: "0.1.0".into(),
        min_protocol: (0, 1),
    }
}

#[test]
fn missing_or_unknown_image_never_proceeds_or_prompts_for_a_product() {
    let catalog = catalog(&board("board-a", "1234:0001"), "1234:0001");
    for image in [
        None,
        Some(image(99, 9)),
        Some(image(41, 99)),
        Some(image(41, 10)),
    ] {
        for chosen in [None, Some("first"), Some("not-a-product")] {
            assert_eq!(
                decide_recovery(&catalog, Some(USB_A), chosen, image.as_ref(), None),
                RecoveryDecision::Refuse(RecoveryRefusal::ImageUnavailable)
            );
        }
    }
}

#[test]
fn product_choices_cannot_bypass_the_ports_candidates() {
    let catalog = catalog(&board("board-a", "1234:0001"), "1234:0002");
    let image = image(41, 9);
    for (usb, chosen) in [
        (Some((0x9999, 1)), None),
        (Some(USB_A), Some("second")),
        (Some(USB_A), Some("not-a-product")),
        (Some(USB_A), Some("")),
        (None, Some("not-a-product")),
    ] {
        assert_eq!(
            decide_recovery(&catalog, usb, chosen, Some(&image), None),
            RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
        );
    }
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), None, Some(&image), None),
        RecoveryDecision::Proceed
    );
    assert_eq!(
        decide_recovery(&catalog, Some(USB_B), Some("second"), Some(&image), None),
        RecoveryDecision::Refuse(RecoveryRefusal::WrongProduct(WrongProduct {
            device: "Second on board-a".into(),
            image: "First on board-a".into(),
        }))
    );
}

#[test]
fn several_products_require_a_choice_even_when_only_one_matches_the_image() {
    let catalog = catalog(&board("board-a", "1234:0001"), "1234:0001");
    let image = image(41, 9);
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), None, Some(&image), None),
        RecoveryDecision::Choose(vec![
            RestoreOption {
                id: "first".into(),
                name: "First".into()
            },
            RestoreOption {
                id: "second".into(),
                name: "Second".into()
            },
        ])
    );
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), Some("first"), Some(&image), None),
        RecoveryDecision::Proceed
    );
}

#[test]
fn an_image_cannot_choose_between_boards_with_shared_usb_hints() {
    let boards = board("board-a", "1234:0001") + &board("board-b", "1234:0001");
    let catalog = catalog(&boards, "1234:0002");
    for board in [9, 10] {
        let image = image(41, board);
        for chosen in [None, Some("first")] {
            assert_eq!(
                decide_recovery(&catalog, Some(USB_A), chosen, Some(&image), None),
                RecoveryDecision::Refuse(RecoveryRefusal::BoardAmbiguous)
            );
        }
    }
}

#[test]
fn distinct_board_hints_resolve_the_pair_without_catalog_order() {
    let boards = board("board-a", "1234:0001") + &board("board-b", "1234:0002");
    let catalog = catalog(&boards, "1234:0003");
    for (usb, number) in [(USB_A, 9), (USB_B, 10)] {
        assert_eq!(
            decide_recovery(
                &catalog,
                Some(usb),
                Some("first"),
                Some(&image(41, number)),
                None
            ),
            RecoveryDecision::Proceed
        );
    }
    assert_eq!(
        decide_recovery(&catalog, None, Some("first"), Some(&image(41, 9)), None),
        RecoveryDecision::Refuse(RecoveryRefusal::BoardAmbiguous)
    );
}

#[test]
fn policy_refusals_clear_old_choices_and_never_reserve_or_start_a_flash() {
    for (reason, expected) in [
        (
            RecoveryRefusal::ImageUnavailable,
            FlashFailure::ImageUnavailable,
        ),
        (RecoveryRefusal::NoCandidate, FlashFailure::Unknown),
        (RecoveryRefusal::BoardAmbiguous, FlashFailure::Unknown),
    ] {
        let mut flash = FlashWorkflow::new(true, 512);
        flash.fail_wrong_product(WrongProduct {
            device: "Old device".into(),
            image: "Old image".into(),
        });
        flash.ask_restore_choice(vec![RestoreOption {
            id: "old".into(),
            name: "Old choice".into(),
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

#[test]
fn a_resolved_mismatch_keeps_both_real_names_and_drops_stale_choices() {
    let catalog = catalog(&board("board-a", "1234:0001"), "1234:0002");
    let RecoveryDecision::Refuse(reason) = decide_recovery(
        &catalog,
        Some(USB_B),
        Some("second"),
        Some(&image(41, 9)),
        None,
    ) else {
        panic!("the image is not for the resolved pair");
    };
    let mut flash = FlashWorkflow::new(true, 512);
    flash.ask_restore_choice(vec![RestoreOption {
        id: "old".into(),
        name: "Old choice".into(),
    }]);
    flash.drain_activity();
    flash.refuse_recovery(reason);
    let status = flash.status();
    assert_eq!(status.failure, Some(FlashFailure::WrongProduct));
    let names = status.wrong_product.as_ref().unwrap();
    assert_eq!(names.device, "Second on board-a");
    assert_eq!(names.image, "First on board-a");
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
}

#[test]
fn reported_identity_cannot_be_overridden_by_usb_or_a_product_choice() {
    use kivori_desktop::identity::{DeviceIdentity, IdentitySource};
    let catalog = catalog(&board("board-a", "1234:0001"), "1234:0001");
    let image = image(41, 9);
    for (product, board) in [(42, 9), (41, 10), (99, 9)] {
        let device = DeviceIdentity {
            product,
            board,
            hw_rev: None,
            source: IdentitySource::Chip,
        };
        for chosen in [None, Some("first"), Some("second")] {
            assert!(matches!(
                decide_recovery(&catalog, Some(USB_A), chosen, Some(&image), Some(&device)),
                RecoveryDecision::Refuse(RecoveryRefusal::WrongProduct(_))
            ));
        }
    }
    let device = DeviceIdentity {
        product: 41,
        board: 9,
        hw_rev: None,
        source: IdentitySource::Build,
    };
    // Live identity settles the shared-USB product question without a chooser.
    assert_eq!(
        decide_recovery(&catalog, Some(USB_A), None, Some(&image), Some(&device)),
        RecoveryDecision::Proceed
    );
    assert_eq!(
        decide_recovery(
            &catalog,
            Some(USB_A),
            Some("second"),
            Some(&image),
            Some(&device)
        ),
        RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate)
    );

    let boards = board("board-a", "1234:0001") + &board("board-b", "1234:0001");
    let multiple_boards = self::catalog(&boards, "1234:0002");
    let device = DeviceIdentity {
        board: 10,
        ..device
    };
    assert_eq!(
        decide_recovery(
            &multiple_boards,
            Some(USB_A),
            None,
            Some(&self::image(41, 10)),
            Some(&device)
        ),
        RecoveryDecision::Proceed
    );
}
