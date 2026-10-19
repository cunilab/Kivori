//! Synthetic image sets; no files under products/ and no device I/O.
use kivori_desktop::firmware_bundle::{FirmwareBundle, FirmwareImage};

fn elf(markers: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 52];
    bytes[..6].copy_from_slice(b"\x7fELF\x01\x01");
    bytes[18] = 243;
    bytes.extend_from_slice(markers);
    bytes
}

fn image(product: u16, board: u16, version: &str, protocol: &str) -> Vec<u8> {
    elf(format!("KIVORI-FW-VERSION:{version}\0KIVORI-FW-IDENTITY:p={product};b={board};v={version};m={protocol}\0").as_bytes())
}

#[test]
fn exact_lookup_and_order_are_independent_of_input_order() {
    let first = image(41, 9, "0.1.0", "0.1");
    let second = image(42, 10, "0.2.0", "0.2");
    let bundle = FirmwareBundle::parse(&[&second, &first]).unwrap();
    assert_eq!(
        bundle
            .images()
            .iter()
            .map(|image| (image.identity().product, image.identity().board))
            .collect::<Vec<_>>(),
        [(41, 9), (42, 10)]
    );
    assert_eq!(bundle.get(41, 9).unwrap().bytes(), first);
    let selected = bundle.get(42, 10).unwrap();
    assert_eq!(selected.bytes(), second);
    assert_eq!(selected.identity().version, "0.2.0");
    assert_eq!(selected.identity().min_protocol, (0, 2));
    assert!(bundle.get(41, 10).is_none());
    assert!(bundle.get(99, 9).is_none());
    assert!(bundle.get(0, 0).is_none());
    assert!(bundle.single().is_none());
}

#[test]
fn only_a_single_image_can_use_the_compatibility_bridge() {
    let empty = FirmwareBundle::parse(&[]).unwrap();
    assert!(empty.single().is_none());
    let bytes = image(41, 9, "0.1.0", "0.1");
    let one = FirmwareBundle::parse(&[&bytes]).unwrap();
    assert_eq!(one.single().unwrap().bytes(), bytes);
    assert!(FirmwareBundle::parse(&[&bytes, &bytes])
        .unwrap_err()
        .contains("duplicate"));
}

#[test]
fn wrong_elf_formats_fail_at_the_shared_boundary() {
    let valid = image(41, 9, "0.1.0", "0.1");
    for (position, value) in [(0, 0), (4, 2), (5, 2), (18, 1), (19, 1)] {
        let mut invalid = valid.clone();
        invalid[position] = value;
        assert!(FirmwareImage::parse(&invalid)
            .unwrap_err()
            .contains("ELF32"));
    }
    assert!(FirmwareImage::parse(&valid[..20]).is_err());
}

#[test]
fn absent_duplicate_or_disagreeing_records_are_refused() {
    let identity = b"KIVORI-FW-IDENTITY:p=41;b=9;v=0.1.0;m=0.1\0";
    let version = b"KIVORI-FW-VERSION:0.1.0\0";
    let valid = [version.as_slice(), identity.as_slice()].concat();
    for (markers, reason) in [
        (version.to_vec(), "no identity"),
        (identity.to_vec(), "no version"),
        (
            [valid.as_slice(), identity.as_slice()].concat(),
            "ambiguous identity",
        ),
        (
            [valid.as_slice(), version.as_slice()].concat(),
            "ambiguous or differing version",
        ),
        (
            [b"KIVORI-FW-VERSION:0.2.0\0".as_slice(), identity.as_slice()].concat(),
            "differing version",
        ),
    ] {
        assert!(FirmwareImage::parse(&elf(&markers))
            .unwrap_err()
            .contains(reason));
    }
}

#[test]
fn malformed_marker_copies_do_not_hide_or_duplicate_the_real_record() {
    let mut bytes = image(41, 9, "0.1.0", "0.1");
    bytes.extend_from_slice(b"KIVORI-FW-VERSION:bad\0KIVORI-FW-IDENTITY:bad\0");
    assert_eq!(FirmwareImage::parse(&bytes).unwrap().identity().product, 41);
}

#[test]
fn compiled_index_entries_match_their_lookup_keys() {
    let bundle = kivori_desktop::firmware::bundled_images();
    for image in bundle.images() {
        let identity = image.identity();
        let found = kivori_desktop::firmware::image_for(identity.product, identity.board).unwrap();
        assert_eq!(found.bytes(), image.bytes());
    }
    let status = kivori_desktop::firmware::initial_status();
    assert_eq!(status.available, !bundle.images().is_empty());
    assert_eq!(status.image_size, 0);
    assert_eq!(status.bundled_version, None);
}

#[test]
fn status_follows_exact_device_images_and_retains_an_active_target() {
    use kivori_desktop::firmware::{FlashWorkflow, UpdateAdvice};
    let a = image(41, 9, "0.1.0", "0.1");
    let b = image(42, 10, "0.3.0", "0.1");
    let bundle = FirmwareBundle::parse(&[&b, &a]).unwrap();
    let mut workflow = FlashWorkflow::new(true, 0);
    let mut published = workflow.status().clone();
    for (product, board, version, expected, bytes) in [
        (41, 9, "0.1.0", UpdateAdvice::UpToDate, a.len()),
        (42, 10, "0.2.0", UpdateAdvice::UpdateAvailable, b.len()),
    ] {
        workflow.refresh_image(bundle.get(product, board), Some(version), &mut published);
        assert_eq!(published.image_size, bytes as u64);
        assert_eq!(published.advice, expected);
        assert_eq!(
            published.bundled_version.as_deref(),
            Some(
                bundle
                    .get(product, board)
                    .unwrap()
                    .identity()
                    .version
                    .as_str()
            )
        );
    }
    for key in [(99, 9), (41, 10)] {
        workflow.refresh_image(bundle.get(key.0, key.1), Some("0.0.1"), &mut published);
        assert_eq!(published.bundled_version, None);
        assert_eq!(published.image_size, 0);
        assert_eq!(published.advice, UpdateAdvice::Unknown);
        assert!(published.available, "recovery remains available");
    }
    workflow.select_image(bundle.get(42, 10));
    workflow
        .request(true, Some("fixture-port"), Some("fixture-hash"))
        .unwrap();
    let accepted = workflow.status().clone();
    published = accepted.clone();
    workflow.select_image(bundle.get(41, 9));
    workflow.refresh_image(None, None, &mut published);
    assert_eq!(workflow.status(), &accepted);
    assert_eq!(published, accepted);
    workflow.finish(Ok(()));
    let reconnecting = workflow.status().clone();
    workflow.select_image(None);
    assert_eq!(workflow.status(), &reconnecting);
}
