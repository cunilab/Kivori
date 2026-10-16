//! Build-time product artwork validation. Never linked into the shipping runtime.

use std::fs;
use std::io::Read;
use std::path::Path;

use crate::catalog::CatalogError;

const MAX_BYTES: usize = 512 * 1024;
const MAX_SIDE: u32 = 1024;

/// Reads a static PNG; `image` is a filename already validated by `Catalog::parse`.
pub fn read(product_dir: &Path, image: &str) -> Result<Vec<u8>, CatalogError> {
    let invalid = |reason: &str| CatalogError(format!("product art `{image}`: {reason}"));
    let path = product_dir.join(image);
    let metadata = fs::symlink_metadata(&path).map_err(|e| invalid(&e.to_string()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_BYTES as u64
    {
        return Err(invalid("expected a regular file of at most 512 KiB"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| invalid(&e.to_string()))?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| invalid(&e.to_string()))?;
    if bytes.len() > MAX_BYTES {
        return Err(invalid("file exceeds 512 KiB"));
    }
    let decoder = png::Decoder::new_with_limits(
        std::io::Cursor::new(&bytes),
        png::Limits {
            bytes: 16 * 1024 * 1024,
        },
    );
    let mut reader = decoder.read_info().map_err(|e| invalid(&e.to_string()))?;
    let info = reader.info();
    if info.width == 0 || info.height == 0 || info.width > MAX_SIDE || info.height > MAX_SIDE {
        return Err(invalid(
            "dimensions must be between 1 and 1024 pixels per side",
        ));
    }
    if info.animation_control.is_some() {
        return Err(invalid("animated PNGs are not supported"));
    }
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| invalid("decoded image is too large"))?
    ];
    reader
        .next_frame(&mut pixels)
        .map_err(|e| invalid(&e.to_string()))?;
    reader.finish().map_err(|e| invalid(&e.to_string()))?;
    if reader.info().animation_control.is_some() {
        return Err(invalid("animated PNGs are not supported"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, animated: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            if animated {
                encoder.set_animated(1, 0).unwrap();
            }
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![0; (width * height * 4) as usize])
                .unwrap();
        }
        bytes
    }

    #[test]
    fn accepts_static_pngs_and_the_dimension_boundary() {
        let root = tempfile::tempdir().unwrap();
        let bytes = png(MAX_SIDE, 1, false);
        fs::write(root.path().join("unit.png"), &bytes).unwrap();
        assert_eq!(read(root.path(), "unit.png").unwrap(), bytes);
    }

    #[test]
    fn refuses_missing_files_and_directories() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("dir.png")).unwrap();
        assert!(read(root.path(), "missing.png").is_err());
        assert!(read(root.path(), "dir.png").is_err());
    }

    #[test]
    fn refuses_corrupt_truncated_oversized_and_animated_images() {
        let root = tempfile::tempdir().unwrap();
        let good = png(1, 1, false);
        let mut corrupt = good.clone();
        corrupt[29] ^= 1; // IHDR checksum.
        for bytes in [
            Vec::new(),
            b"<svg/>".to_vec(),
            good[..good.len() - 8].to_vec(),
            corrupt,
            vec![0; MAX_BYTES + 1],
            png(MAX_SIDE + 1, 1, false),
            png(1, 1, true),
        ] {
            fs::write(root.path().join("art.png"), bytes).unwrap();
            assert!(read(root.path(), "art.png").is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_files() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("real")).unwrap();
        fs::write(root.path().join("real/art.png"), png(1, 1, false)).unwrap();
        symlink(
            root.path().join("real/art.png"),
            root.path().join("art.png"),
        )
        .unwrap();
        assert!(read(root.path(), "art.png").is_err());
    }
}
