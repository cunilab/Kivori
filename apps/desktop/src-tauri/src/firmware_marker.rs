//! Pure scan for the firmware version marker.
//!
//! The firmware embeds `KIVORI-FW-VERSION:<major>.<minor>.<patch>\0` in its image. This file has no
//! dependencies on the rest of the crate: `build.rs` includes it with `#[path]` to stamp the bundled
//! version at compile time, and the library re-exports it so tests exercise the very same code.

/// The text that precedes the version inside the firmware image.
pub const FIRMWARE_VERSION_MARKER: &[u8] = b"KIVORI-FW-VERSION:";

/// Finds the first well-formed `KIVORI-FW-VERSION:<x>.<y>.<z>\0` marker in `image`.
///
/// A malformed occurrence (missing terminator, non-numeric or short version) is skipped, not
/// treated as an answer, so a stray copy of the marker text cannot hide the real one.
#[must_use]
pub fn scan_firmware_version(image: &[u8]) -> Option<String> {
    let marker = FIRMWARE_VERSION_MARKER;
    let mut from = 0;
    while let Some(offset) = image
        .get(from..)
        .and_then(|rest| rest.windows(marker.len()).position(|w| w == marker))
    {
        let start = from + offset + marker.len();
        let tail = &image[start..];
        // A version is short; look at a bounded window so a missing terminator cannot scan far.
        let window = &tail[..tail.len().min(32)];
        if let Some(end) = window.iter().position(|&b| b == 0) {
            if let Ok(text) = std::str::from_utf8(&window[..end]) {
                if is_triple(text) {
                    return Some(text.to_string());
                }
            }
        }
        from = start;
    }
    None
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
    use super::scan_firmware_version;

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
