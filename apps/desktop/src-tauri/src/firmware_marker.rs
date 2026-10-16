//! Pure scan for the firmware version marker.
//!
//! The firmware embeds `KIVORI-FW-VERSION:<major>.<minor>.<patch>\0` in its image. This file has no
//! dependencies on the rest of the crate: build-time validation includes it with `#[path]`, and
//! the library uses the same parser for immutable image-set lookup.

/// The text that precedes the version inside the firmware image.
pub const FIRMWARE_VERSION_MARKER: &[u8] = b"KIVORI-FW-VERSION:";

/// Finds the first well-formed `KIVORI-FW-VERSION:<x>.<y>.<z>\0` marker in `image`.
///
/// A malformed occurrence (missing terminator, non-numeric or short version) is skipped, not
/// treated as an answer, so a stray copy of the marker text cannot hide the real one.
#[must_use]
pub fn scan_firmware_version(image: &[u8]) -> Option<String> {
    scan_firmware_versions(image).next()
}

/// Every well-formed version marker, including duplicates (for bundle validation).
pub fn scan_firmware_versions(image: &[u8]) -> impl Iterator<Item = String> + '_ {
    records(image, FIRMWARE_VERSION_MARKER, 32)
        .filter(|text| is_triple(text))
        .map(str::to_string)
}

/// The text that precedes the image identity record inside the firmware image.
pub const FIRMWARE_IDENTITY_MARKER: &[u8] = b"KIVORI-FW-IDENTITY:";

/// What a firmware image says it is for (ADR-0014): the identity record next to the version marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageIdentity {
    /// Product wire number.
    pub product: u16,
    /// Board wire number.
    pub board: u16,
    /// Firmware version `x.y.z`.
    pub version: String,
    /// Oldest wire protocol `(major, minor)` the image speaks.
    pub min_protocol: (u16, u16),
}

/// Finds the first well-formed `KIVORI-FW-IDENTITY:p=<n>;b=<n>;v=<x.y.z>;m=<major.minor>\0` record in
/// `image`. Like [`scan_firmware_version`], a malformed copy is skipped, not an answer.
#[must_use]
pub fn scan_image_identity(image: &[u8]) -> Option<ImageIdentity> {
    scan_image_identities(image).next()
}

/// Every well-formed identity record, including duplicates (for bundle validation).
pub fn scan_image_identities(image: &[u8]) -> impl Iterator<Item = ImageIdentity> + '_ {
    records(image, FIRMWARE_IDENTITY_MARKER, 64).filter_map(parse_image_identity)
}

fn records<'a>(
    image: &'a [u8],
    marker: &'static [u8],
    limit: usize,
) -> impl Iterator<Item = &'a str> + 'a {
    image
        .windows(marker.len())
        .enumerate()
        .filter_map(move |(offset, window)| {
            if window != marker {
                return None;
            }
            let tail = &image[offset + marker.len()..];
            let bounded = &tail[..tail.len().min(limit)];
            let end = bounded.iter().position(|&byte| byte == 0)?;
            std::str::from_utf8(&bounded[..end]).ok()
        })
}

fn parse_image_identity(text: &str) -> Option<ImageIdentity> {
    let mut fields = text.split(';');
    let product = fields.next()?.strip_prefix("p=")?;
    let board = fields.next()?.strip_prefix("b=")?;
    let version = fields.next()?.strip_prefix("v=")?;
    let min = fields.next()?.strip_prefix("m=")?;
    if fields.next().is_some() || !is_triple(version) {
        return None;
    }
    let number = |text: &str| {
        (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
            .then(|| text.parse::<u16>().ok())
            .flatten()
    };
    let (major, minor) = min.split_once('.')?;
    Some(ImageIdentity {
        product: number(product)?,
        board: number(board)?,
        version: version.to_string(),
        min_protocol: (number(major)?, number(minor)?),
    })
}

/// `major.minor.patch`, each a non-empty run of ASCII digits that fits in a `u32`.
fn is_triple(text: &str) -> bool {
    let mut parts = text.split('.');
    let ok = (0..3).all(|_| {
        parts.next().is_some_and(|p| {
            !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u32>().is_ok()
        })
    });
    ok && parts.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::{scan_firmware_version, scan_image_identity, ImageIdentity};

    fn identity(product: u16, board: u16) -> ImageIdentity {
        ImageIdentity {
            product,
            board,
            version: "0.1.0".into(),
            min_protocol: (0, 1),
        }
    }

    #[test]
    fn finds_the_identity_record_beside_the_version_marker() {
        let mut image = b"KIVORI-FW-VERSION:0.1.0\0".to_vec();
        image.extend_from_slice(b"\x01\x02KIVORI-FW-IDENTITY:p=1;b=1;v=0.1.0;m=0.1\0\x03");
        assert_eq!(scan_image_identity(&image), Some(identity(1, 1)));
        assert_eq!(scan_firmware_version(&image).as_deref(), Some("0.1.0"));
    }

    #[test]
    fn absent_or_malformed_identity_records_are_none() {
        assert_eq!(scan_image_identity(b""), None);
        assert_eq!(scan_image_identity(b"KIVORI-FW-VERSION:0.1.0\0"), None);
        for bad in [
            &b"KIVORI-FW-IDENTITY:p=1;b=1;v=0.1.0;m=0.1"[..], // no terminator
            b"KIVORI-FW-IDENTITY:\0",
            b"KIVORI-FW-IDENTITY:p=1;b=1;v=0.1;m=0.1\0",
            b"KIVORI-FW-IDENTITY:p=x;b=1;v=0.1.0;m=0.1\0",
            b"KIVORI-FW-IDENTITY:p=1;b=1;v=0.1.0\0",
            b"KIVORI-FW-IDENTITY:p=1;b=1;v=0.1.0;m=01\0",
            b"KIVORI-FW-IDENTITY:p=70000;b=1;v=0.1.0;m=0.1\0",
            b"KIVORI-FW-IDENTITY:p=1;b=1;v=0.1.0;m=0.1;x=1\0",
        ] {
            assert_eq!(scan_image_identity(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_malformed_identity_copy_does_not_hide_the_real_one() {
        let image = b"KIVORI-FW-IDENTITY:oops\0 KIVORI-FW-IDENTITY:p=2;b=3;v=1.0.0;m=1.2\0";
        let found = scan_image_identity(image).expect("real record");
        assert_eq!(
            (found.product, found.board, found.min_protocol),
            (2, 3, (1, 2))
        );
    }

    #[test]
    fn finds_a_present_marker_among_other_bytes() {
        let mut image = vec![0x7f, b'E', b'L', b'F', 1, 2, 3];
        image.extend_from_slice(b"KIVORI-FW-VERSION:1.3.0\0");
        image.extend_from_slice(&[9, 9, 9]);
        assert_eq!(scan_firmware_version(&image).as_deref(), Some("1.3.0"));
    }

    #[test]
    fn absent_marker_is_none() {
        assert_eq!(scan_firmware_version(b""), None);
        assert_eq!(scan_firmware_version(b"no marker here\0"), None);
    }

    #[test]
    fn malformed_markers_are_none() {
        for bad in [
            &b"KIVORI-FW-VERSION:1.3.0"[..], // no terminator
            b"KIVORI-FW-VERSION:\0",
            b"KIVORI-FW-VERSION:1.3\0",
            b"KIVORI-FW-VERSION:1.3.0.1\0",
            b"KIVORI-FW-VERSION:1.x.0\0",
            b"KIVORI-FW-VERSION:v1.3.0\0",
            b"KIVORI-FW-VERSION:1.3.0-rc1\0",
            b"KIVORI-FW-VERSION:99999999999.0.0\0",
            b"KIVORI-FW-VERSION:\xff\xfe.1.0\0",
        ] {
            assert_eq!(scan_firmware_version(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_malformed_copy_does_not_hide_the_real_marker() {
        let image = b"KIVORI-FW-VERSION:oops\0 ... KIVORI-FW-VERSION:2.0.11\0";
        assert_eq!(scan_firmware_version(image).as_deref(), Some("2.0.11"));
    }
}
