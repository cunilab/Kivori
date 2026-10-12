//! T029 — header validation: invalid magic, unsupported header version, truncated header.

use heapless::Vec;
use kivori_protocol::frame::cobs_encode;
use kivori_protocol::{
    crc32, decode_frame, decode_message, ProtoError, MAGIC, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR,
};

fn raw_frame(magic: u16, major: u16, minor: u16, seq: u16, payload: &[u8]) -> Vec<u8, MAX_FRAME> {
    let mut f: Vec<u8, MAX_FRAME> = Vec::new();
    f.extend_from_slice(&magic.to_le_bytes()).unwrap();
    f.extend_from_slice(&major.to_le_bytes()).unwrap();
    f.extend_from_slice(&minor.to_le_bytes()).unwrap();
    f.extend_from_slice(&seq.to_le_bytes()).unwrap();
    f.extend_from_slice(&(payload.len() as u16).to_le_bytes())
        .unwrap();
    f.extend_from_slice(payload).unwrap();
    let crc = crc32(f.as_slice());
    f.extend_from_slice(&crc.to_le_bytes()).unwrap();
    f
}

fn cobs(frame: &[u8]) -> Vec<u8, MAX_WIRE> {
    let mut w: Vec<u8, MAX_WIRE> = Vec::new();
    cobs_encode(frame, &mut w).unwrap();
    w
}

#[test]
fn bad_magic_is_rejected() {
    let frame = raw_frame(0x0000, 1, 0, 1, &[1, 2, 3]);
    let packet = cobs(frame.as_slice());
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    assert_eq!(
        decode_frame(&packet, &mut scratch).unwrap_err(),
        ProtoError::BadMagic
    );
}

#[test]
fn truncated_header_is_rejected() {
    // A COBS packet decoding to fewer than HEADER_LEN + CRC_LEN bytes.
    let packet = cobs(&[1, 2, 3, 4, 5]);
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    assert_eq!(
        decode_frame(&packet, &mut scratch).unwrap_err(),
        ProtoError::TooShort
    );
}

#[test]
fn unsupported_major_is_rejected() {
    // Well-formed frame carrying major 1 (a board flashed before the version reset); this side
    // supports only the current major, 0.
    let frame = raw_frame(MAGIC, PROTOCOL_MAJOR + 1, 6, 1, &[]);
    let packet = cobs(frame.as_slice());
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    assert_eq!(
        decode_message(&packet, &mut scratch, &[PROTOCOL_MAJOR]).unwrap_err(),
        ProtoError::UnsupportedVersion
    );
}

#[test]
fn magic_bytes_are_stable() {
    assert_eq!(MAGIC.to_le_bytes(), [0x56, 0x4B]); // little-endian "KV" (0x4B56)
}
