//! T030 — payload validation: declared length > received, payload > max, CRC mismatch, malformed
//! postcard, unknown message discriminant.

use heapless::Vec;
use kivori_protocol::frame::cobs_encode;
use kivori_protocol::{
    crc32, decode_frame, decode_message, ProtoError, MAGIC, MAX_FRAME, MAX_WIRE,
};

/// Builds a raw frame with an explicit (possibly lying) `payload_len` field; CRC is computed over the
/// actual header+payload bytes.
fn frame_with_len_field(len_field: u16, payload: &[u8]) -> Vec<u8, MAX_FRAME> {
    let mut f: Vec<u8, MAX_FRAME> = Vec::new();
    f.extend_from_slice(&MAGIC.to_le_bytes()).unwrap();
    f.extend_from_slice(&1u16.to_le_bytes()).unwrap(); // major
    f.extend_from_slice(&0u16.to_le_bytes()).unwrap(); // minor
    f.extend_from_slice(&1u16.to_le_bytes()).unwrap(); // seq
    f.extend_from_slice(&len_field.to_le_bytes()).unwrap();
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

fn decode_err(frame: &[u8]) -> ProtoError {
    let packet = cobs(frame);
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    decode_frame(&packet, &mut scratch).unwrap_err()
}

#[test]
fn declared_length_greater_than_received_is_rejected() {
    let frame = frame_with_len_field(100, &[1, 2, 3]);
    assert_eq!(decode_err(frame.as_slice()), ProtoError::LengthMismatch);
}

#[test]
fn payload_larger_than_max_is_rejected() {
    let frame = frame_with_len_field(1000, &[1, 2, 3]); // 1000 > MAX_PAYLOAD (512)
    assert_eq!(decode_err(frame.as_slice()), ProtoError::PayloadTooLarge);
}

#[test]
fn crc_mismatch_is_rejected() {
    let mut frame = frame_with_len_field(3, &[1, 2, 3]); // structurally valid, CRC correct
    let last = frame.len() - 1;
    frame[last] ^= 0xFF; // corrupt the CRC
    assert_eq!(decode_err(frame.as_slice()), ProtoError::BadCrc);
}

#[test]
fn malformed_postcard_payload_is_rejected() {
    // Valid frame + CRC, but the payload is not a decodable Message (variant 0 = Hello, truncated).
    let frame = frame_with_len_field(1, &[0x00]);
    let packet = cobs(frame.as_slice());
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    assert_eq!(
        decode_message(&packet, &mut scratch, &[1]).unwrap_err(),
        ProtoError::Postcard
    );
}

#[test]
fn unknown_message_discriminant_is_rejected() {
    // Variant index 13 does not exist (Message has 13 variants, indices 0..=12).
    let frame = frame_with_len_field(1, &[0x0D]);
    let packet = cobs(frame.as_slice());
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    assert_eq!(
        decode_message(&packet, &mut scratch, &[1]).unwrap_err(),
        ProtoError::Postcard
    );
}
