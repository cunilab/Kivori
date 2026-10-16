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
    // The shipping gate limits this check to zero/one; the generated host harness tests multiple.

    let bundle = kivori_desktop::firmware::bundled_images();
    for image in bundle.images() {
        let identity = image.identity();
        let found = kivori_desktop::firmware::image_for(identity.product, identity.board).unwrap();
        assert_eq!(found.bytes(), image.bytes());
    }
    assert_eq!(
        kivori_desktop::firmware::single_image().is_some(),
        bundle.images().len() == 1
    );
    let status = kivori_desktop::firmware::initial_status();
    assert_eq!(status.available, bundle.single().is_some());
    assert_eq!(
        status.image_size,
        bundle
            .single()
            .map_or(0, |image| image.bytes().len() as u64)
    );
    assert_eq!(
        status.bundled_version,
        bundle
            .single()
            .map(|image| image.identity().version.clone())
    );
}
