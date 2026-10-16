//! QA policy fixtures never enter shipping discovery and never perform device I/O.
#![cfg(feature = "device-studio")]

use kivori_desktop::catalog::Catalog;
use kivori_desktop::firmware_marker::ImageIdentity;
use kivori_desktop::identity::{DeviceIdentity, IdentitySource};
use kivori_desktop::provision::{
    check_image_matches_plan, evaluate, parse_args, require_burn_backend, resolve_plan,
    FailedCheck, InputCheck, Observations, QaPolicy,
};
use kivori_model::Capabilities;
use kivori_protocol::CONTEXT_BUTTONS;

fn product(board: &str, caps: &[&str], usb: &str) -> String {
    format!("display_name = 'Test product'\nshort_name = 'Test'\nmodel = 'KV-T1'\nrequired_capabilities = {caps:?}\n[boards.{board}]\ndisplay_name = 'Test board'\nflash = 'espflash'\nusb = ['{usb}']\n[boards.{board}.firmware]\ncrate = 'firmware/test'\nartifact = 'image.elf'\nmin_protocol = '0.1'\n")
}

fn catalog(caps: &[&str]) -> Catalog {
    Catalog::parse(
        "[products]\nfirst = 41\nfixture = 42\n[boards]\nfirst-board = 9\ntest-board = 10\n",
        &[
            (
                "first",
                &product(
                    "first-board",
                    &["PHYSICAL_INPUT_V1", "BUTTON_INPUT_V1", "CONTEXT_BUTTONS_V1"],
                    "1234:0001",
                ),
            ),
            ("fixture", &product("test-board", caps, "1234:0002")),
        ],
    )
    .unwrap()
}

fn image(product: u16, board: u16) -> ImageIdentity {
    ImageIdentity {
        product,
        board,
        version: "0.2.3".into(),
        min_protocol: (0, 1),
    }
}

fn observed(policy: &QaPolicy<'_>) -> Observations {
    Observations {
        flash_failure: None,
        handshake: true,
        identity: Some(DeviceIdentity {
            product: policy.image.product,
            board: policy.image.board,
            hw_rev: None,
            source: IdentitySource::Build,
        }),
        negotiated_caps: policy
            .required_capabilities
            .iter()
            .fold(Capabilities::NONE, |caps, (_, flag)| caps.union(*flag)),
        device_version: Some(policy.image.version.clone()),
        bundled_version: Some(policy.image.version.clone()),
        heartbeat_ok: true,
        state_report_seen: true,
        health_seen: true,
        inputs_done: policy.inputs.clone(),
        identity_check: None,
    }
}

#[test]
fn second_product_uses_its_own_requirements_and_board_hints_not_catalog_order() {
    let catalog = catalog(&["PHYSICAL_INPUT_V1"]);
    let second_image = image(42, 10);
    let policy = QaPolicy::for_image(&catalog, Some(&second_image)).unwrap();
    assert_eq!(policy.product.id, "fixture");
    assert_eq!(policy.board.usb, [(0x1234, 2)]);
    assert_eq!(policy.required_capabilities.len(), 1);
    assert_eq!(policy.inputs, [InputCheck::TurnRight, InputCheck::TurnLeft]);
    assert!(evaluate(&observed(&policy), &policy).passed());
    let first_image = image(41, 9);
    let first = QaPolicy::for_image(&catalog, Some(&first_image)).unwrap();
    assert_eq!(first.inputs.len(), 4 + usize::from(CONTEXT_BUTTONS));
}

#[test]
fn no_input_product_has_no_prompts_but_still_needs_baseline_health_and_identity() {
    let catalog = catalog(&[]);
    let image = image(42, 10);
    let policy = QaPolicy::for_image(&catalog, Some(&image)).unwrap();
    assert!(policy.inputs.is_empty());
    let mut obs = observed(&policy);
    assert!(evaluate(&obs, &policy).passed());
    obs.health_seen = false;
    assert_eq!(evaluate(&obs, &policy).failed, [FailedCheck::Health]);
    obs.identity = None;
    assert_eq!(
        evaluate(&obs, &policy).failed,
        [FailedCheck::ProductIdentity]
    );
}

#[test]
fn missing_required_negotiation_cannot_shrink_input_expectations() {
    let catalog = catalog(&[]);
    let image = image(41, 9);
    let policy = QaPolicy::for_image(&catalog, Some(&image)).unwrap();
    let mut obs = observed(&policy);
    obs.negotiated_caps = Capabilities::NONE;
    assert!(policy
        .inputs
        .iter()
        .all(|check| !check.supported_by(obs.negotiated_caps)));
    let verdict = evaluate(&obs, &policy);
    assert!(verdict
        .failed
        .contains(&FailedCheck::Capability("PHYSICAL_INPUT_V1")));
    assert_eq!(
        verdict
            .failed
            .iter()
            .filter(|check| matches!(check, FailedCheck::Input(_)))
            .count(),
        4 + usize::from(CONTEXT_BUTTONS)
    );
}

#[test]
fn missing_unknown_or_inconsistent_catalog_inputs_are_refused() {
    let valid = catalog(&[]);
    assert!(QaPolicy::for_image(&valid, None).is_err());
    for image in [image(0, 10), image(99, 10), image(42, 99)] {
        assert!(QaPolicy::for_image(&valid, Some(&image)).is_err());
    }
    for caps in [
        &["UNKNOWN_CAP"][..],
        &["BUTTON_INPUT_V1"],
        &["CONTEXT_BUTTONS_V1"],
        &["DOUBLE_PRESS_V1"],
        &["PHYSICAL_INPUT_V1", "DOUBLE_PRESS_V1"],
    ] {
        let invalid = catalog(caps);
        assert!(QaPolicy::for_image(&invalid, Some(&image(42, 10))).is_err());
    }
}

#[test]
fn post_flash_identity_and_both_versions_must_match_the_selected_image() {
    let catalog = catalog(&[]);
    let image = image(42, 10);
    let policy = QaPolicy::for_image(&catalog, Some(&image)).unwrap();
    let mut obs = observed(&policy);
    let matching = obs.identity.unwrap();
    for identity in [
        None,
        Some(DeviceIdentity {
            source: IdentitySource::Legacy,
            ..matching
        }),
        Some(DeviceIdentity {
            product: 41,
            ..matching
        }),
        Some(DeviceIdentity {
            board: 9,
            ..matching
        }),
        Some(DeviceIdentity {
            product: 99,
            ..matching
        }),
    ] {
        obs.identity = identity;
        assert_eq!(
            evaluate(&obs, &policy).failed,
            [FailedCheck::ProductIdentity]
        );
    }
    for source in [IdentitySource::Build, IdentitySource::Chip] {
        obs.identity = Some(DeviceIdentity { source, ..matching });
        assert!(evaluate(&obs, &policy).passed());
    }
    obs.device_version = Some("0.1.0".into());
    obs.bundled_version = Some("0.1.0".into());
    assert_eq!(
        evaluate(&obs, &policy).failed,
        [FailedCheck::FirmwareVersion]
    );
}

#[test]
fn identity_selection_asserts_the_image_even_without_burning_and_other_boards_cannot_burn() {
    let catalog = catalog(&[]);
    let args = [
        "--product",
        "fixture",
        "--board",
        "test-board",
        "--rev",
        "1",
    ]
    .map(String::from)
    .to_vec();
    let mut args = parse_args(&args).unwrap();
    let plan = resolve_plan(&catalog, &args).unwrap().unwrap();
    assert!(check_image_matches_plan(Some(&image(42, 10)), &plan).is_ok());
    assert!(check_image_matches_plan(Some(&image(41, 9)), &plan).is_err());
    args.burn = true;
    assert!(resolve_plan(&catalog, &args)
        .unwrap_err()
        .contains("not supported"));
    assert!(require_burn_backend("c3-supermini").is_ok());
    assert!(require_burn_backend("test-board").is_err());
}

#[test]
fn provisioning_selects_the_exact_plan_and_never_a_first_image_default() {
    use kivori_desktop::firmware_bundle::FirmwareBundle;
    use kivori_desktop::provision::{select_image, IdentityPlan};
    let bytes = |product, board, version| {
        let mut elf = vec![0; 52];
        elf[..6].copy_from_slice(b"\x7fELF\x01\x01");
        elf[18] = 243;
        elf.extend_from_slice(format!("KIVORI-FW-VERSION:{version}\0KIVORI-FW-IDENTITY:p={product};b={board};v={version};m=0.1\0").as_bytes());
        elf
    };
    let first = bytes(41, 9, "0.1.0");
    let second = bytes(42, 10, "0.2.0");
    let bundle = FirmwareBundle::parse(&[&second, &first]).unwrap();
    let plan = IdentityPlan {
        product_id: "fixture".into(),
        board_id: "test-board".into(),
        product: 42,
        board: 10,
        rev: 1,
    };
    assert_eq!(select_image(&bundle, Some(&plan)).unwrap().bytes(), second);
    assert!(select_image(&bundle, None).is_err());
    assert!(select_image(&bundle, Some(&IdentityPlan { board: 9, ..plan })).is_err());
    let one = FirmwareBundle::parse(&[&first]).unwrap();
    assert_eq!(select_image(&one, None).unwrap().bytes(), first);
    assert!(select_image(&FirmwareBundle::default(), None).is_err());
}
