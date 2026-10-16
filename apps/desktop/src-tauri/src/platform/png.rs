//! A minimal PNG writer and a base64 encoder, for app icons and bundled product artwork. The
//! pixel data is stored uncompressed (deflate "stored" blocks): an icon is a few kilobytes, so a
//! compressor would only add a dependency.

#![cfg_attr(not(any(windows, test)), allow(dead_code))]

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// The largest payload of one stored deflate block.
const STORED_MAX: usize = 65_535;

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut crc = !0u32;
    for chunk in chunks {
        for &byte in *chunk {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// An 8-bit RGBA PNG of `rgba` (`width * height * 4` bytes, row-major); `None` when the length
/// does not match or the image is empty.
#[must_use]
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let row = usize::try_from(width).ok()?.checked_mul(4)?;
    if width == 0 || height == 0 || rgba.len() != row.checked_mul(usize::try_from(height).ok()?)? {
        return None;
    }
    // Every scanline is preceded by its filter type, 0 (none).
    let mut raw = Vec::with_capacity(rgba.len() + rgba.len() / row);
    for line in rgba.chunks(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks = raw.chunks(STORED_MAX).collect::<Vec<_>>();
    for (i, block) in blocks.iter().enumerate() {
        zlib.push(u8::from(i + 1 == blocks.len()));
        let len = u16::try_from(block.len()).ok()?;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    // 8-bit, colour type 6 (RGBA), default compression, filter and no interlacing.
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = SIGNATURE.to_vec();
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    Some(out)
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard padded base64.
#[must_use]
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for group in data.chunks(3) {
        let n = group
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= group.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A PNG as a `data:` URL the webview can use as an image source.
#[must_use]
pub fn png_data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64(png))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), coded);
        }
    }

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn a_tiny_image_has_the_png_layout() {
        let png = encode_rgba(2, 1, &[255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
        assert_eq!(&png[..8], &SIGNATURE);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..24], &[0, 0, 0, 2, 0, 0, 0, 1]);
        assert_eq!(&png[png.len() - 12..png.len() - 8], &[0, 0, 0, 0]);
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        assert!(png_data_url(&png).starts_with("data:image/png;base64,iVBORw0KGgo"));
    }

    #[test]
    fn a_large_image_is_split_into_stored_blocks() {
        let png = encode_rgba(128, 128, &vec![7; 128 * 128 * 4]).unwrap();
        assert!(png.len() > 128 * 128 * 4);
    }

    #[test]
    fn a_wrong_length_or_an_empty_image_is_refused() {
        assert!(encode_rgba(2, 2, &[0; 15]).is_none());
        assert!(encode_rgba(0, 0, &[]).is_none());
    }
}
