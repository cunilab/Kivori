//! Export the M1 desk frames as PNGs for review (the same pixels the goldens pin), plus, for
//! frames that show the Buddy view, `<name>.mascot.png` with the real idle mascot composed in as
//! the device draws it. `sheet_N.png` puts up to 12 frames (mascot versions) on one page.
//! Run: cargo run -p kivori-golden-frames --example desk_review -- <output-dir>
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_golden_frames::{desk_frames, render_desk, render_desk_with, DeskFrame, DIM};
use kivori_model::desk::DisplayMode;
use kivori_model::{CompanionState, Rgb565};
use kivori_renderer::{frame_hash, render_scene};
use std::{env, fs, path::Path};

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (!(crc & 1)).wrapping_add(1));
        }
    }
    !crc
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// A dependency-free PNG: 8-bit RGB, stored (uncompressed) deflate blocks of up to 64 KiB.
fn png(pixels: &[Rgb565], width: usize) -> Vec<u8> {
    let height = pixels.len() / width;
    let mut raw = Vec::new();
    for row in pixels.chunks(width) {
        raw.push(0); // filter: none
        for p in row {
            let (r, g, b) = p.to_rgb888();
            raw.extend_from_slice(&[r, g, b]);
        }
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn main() {
    let dir = env::args().nth(1).expect("usage: desk_review <output-dir>");
    let dir = Path::new(&dir);
    fs::create_dir_all(dir).unwrap();
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).unwrap();
    let scene = blob.scene(CompanionState::Idle).unwrap();
    let dim = usize::from(DIM);
    let mut frames = Vec::new();
    for (name, frame) in desk_frames() {
        let pixels = render_desk(&frame);
        println!("{name}=0x{:016X}", frame_hash(&pixels));
        fs::write(dir.join(format!("{name}.png")), png(&pixels, dim)).unwrap();
        let buddy = matches!(&frame, DeskFrame::View(v, _)
            if v.status.mode == DisplayMode::Buddy || v.previous_mode == Some(DisplayMode::Buddy));
        if buddy {
            let pixels = render_desk_with(&frame, |b| {
                render_scene(&blob, scene, 0, b).unwrap();
            });
            fs::write(dir.join(format!("{name}.mascot.png")), png(&pixels, dim)).unwrap();
            frames.push(pixels);
        } else {
            frames.push(pixels);
        }
    }
    // Contact sheets of up to 12 frames (4 x 3, 8 px gutters), for reviewing many at once.
    let (cols, rows, gap) = (4, 3, 8);
    let cell = dim + gap;
    for (n, page) in frames.chunks(cols * rows).enumerate() {
        let (w, h) = (cols * cell + gap, rows * cell + gap);
        let mut sheet = vec![Rgb565::from_rgb888(0x40, 0x40, 0x40); w * h];
        for (i, pixels) in page.iter().enumerate() {
            let (ox, oy) = (gap + (i % cols) * cell, gap + (i / cols) * cell);
            for (y, row) in pixels.chunks(dim).enumerate() {
                sheet[(oy + y) * w + ox..][..dim].copy_from_slice(row);
            }
        }
        fs::write(dir.join(format!("sheet_{n}.png")), png(&sheet, w)).unwrap();
    }
}
