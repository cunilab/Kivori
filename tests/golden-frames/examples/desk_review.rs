//! Export the M1 desk frames as PNGs for review (the same pixels the goldens pin).
//! Run: cargo run -p kivori-golden-frames --example desk_review -- <output-dir>
use kivori_golden_frames::{desk_frames, render_desk, DIM};
use kivori_model::Rgb565;
use kivori_renderer::frame_hash;
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
fn png(pixels: &[Rgb565]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in pixels.chunks(usize::from(DIM)) {
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
    ihdr.extend_from_slice(&u32::from(DIM).to_be_bytes());
    ihdr.extend_from_slice(&u32::from(DIM).to_be_bytes());
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
    for (name, frame) in desk_frames() {
        let pixels = render_desk(&frame);
        println!("{name}=0x{:016X}", frame_hash(&pixels));
        fs::write(dir.join(format!("{name}.png")), png(&pixels)).unwrap();
    }
}
