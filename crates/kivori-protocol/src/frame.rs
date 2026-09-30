//! Frame layer: CRC-32, COBS framing, and the fixed header (ADR-0002, docs/architecture.md, wire protocol).
//!
//! Wire packet = `COBS(magic || ver_major || ver_minor || seq || payload_len || payload || crc32)`
//! followed by a `0x00` delimiter. All multi-byte fields are little-endian.

use crate::error::ProtoError;
use crate::{CRC_LEN, HEADER_LEN, MAGIC, MAX_FRAME, MAX_PAYLOAD, MAX_WIRE};
use heapless::Vec;
use kivori_model::ProtocolVersion;

/// A parsed frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// The sender's protocol version.
    pub version: ProtocolVersion,
    /// The frame sequence number.
    pub seq: u16,
    /// The declared payload length, in bytes.
    pub payload_len: u16,
}

/// Computes the CRC-32 (IEEE 802.3, reflected, polynomial `0xEDB88320`) of `data`.
#[must_use]
pub const fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    let mut i = 0;
    while i < data.len() {
        crc ^= data[i] as u32;
        let mut bit = 0;
        while bit < 8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            bit += 1;
        }
        i += 1;
    }
    !crc
}

/// COBS-encodes `data` into `out` (cleared first). The output contains no `0x00` bytes.
///
/// # Errors
/// [`ProtoError::BufferOverflow`] if `out` runs out of capacity.
pub fn cobs_encode<const N: usize>(data: &[u8], out: &mut Vec<u8, N>) -> Result<(), ProtoError> {
    out.clear();
    let mut code_index = out.len();
    out.push(0).map_err(|_| ProtoError::BufferOverflow)?;
    let mut code: u8 = 1;
    for &b in data {
        if b != 0 {
            out.push(b).map_err(|_| ProtoError::BufferOverflow)?;
            code += 1;
            if code == 0xFF {
                out[code_index] = code;
                code_index = out.len();
                out.push(0).map_err(|_| ProtoError::BufferOverflow)?;
                code = 1;
            }
        } else {
            out[code_index] = code;
            code_index = out.len();
            out.push(0).map_err(|_| ProtoError::BufferOverflow)?;
            code = 1;
        }
    }
    out[code_index] = code;
    Ok(())
}

/// COBS-decodes a single packet `data` (WITHOUT the trailing `0x00` delimiter) into `out` (cleared).
///
/// # Errors
/// [`ProtoError::Cobs`] on malformed input; [`ProtoError::BufferOverflow`] if `out` is too small.
pub fn cobs_decode<const N: usize>(data: &[u8], out: &mut Vec<u8, N>) -> Result<(), ProtoError> {
    out.clear();
    let mut i = 0;
    while i < data.len() {
        let code = data[i];
        if code == 0 {
            return Err(ProtoError::Cobs);
        }
        i += 1;
        let block = code as usize - 1;
        if i + block > data.len() {
            return Err(ProtoError::Cobs);
        }
        let mut k = 0;
        while k < block {
            out.push(data[i]).map_err(|_| ProtoError::BufferOverflow)?;
            i += 1;
            k += 1;
        }
        if code != 0xFF && i < data.len() {
            out.push(0).map_err(|_| ProtoError::BufferOverflow)?;
        }
    }
    Ok(())
}

/// Assembles the raw (pre-COBS) frame `header || payload || crc32` into `frame` (cleared).
fn assemble(
    header: &Header,
    payload: &[u8],
    frame: &mut Vec<u8, MAX_FRAME>,
) -> Result<(), ProtoError> {
    if payload.len() > MAX_PAYLOAD {
        return Err(ProtoError::PayloadTooLarge);
    }
    frame.clear();
    let push = |frame: &mut Vec<u8, MAX_FRAME>, bytes: &[u8]| {
        frame
            .extend_from_slice(bytes)
            .map_err(|()| ProtoError::BufferOverflow)
    };
    push(frame, &MAGIC.to_le_bytes())?;
    push(frame, &header.version.major.to_le_bytes())?;
    push(frame, &header.version.minor.to_le_bytes())?;
    push(frame, &header.seq.to_le_bytes())?;
    push(frame, &(payload.len() as u16).to_le_bytes())?;
    push(frame, payload)?;
    let crc = crc32(frame.as_slice());
    push(frame, &crc.to_le_bytes())?;
    Ok(())
}

/// Encodes a header + payload into a complete wire packet (COBS + `0x00` delimiter) in `wire`.
///
/// # Errors
/// [`ProtoError::PayloadTooLarge`] or [`ProtoError::BufferOverflow`].
pub fn encode_frame(
    header: &Header,
    payload: &[u8],
    wire: &mut Vec<u8, MAX_WIRE>,
) -> Result<(), ProtoError> {
    let mut frame: Vec<u8, MAX_FRAME> = Vec::new();
    assemble(header, payload, &mut frame)?;
    cobs_encode(frame.as_slice(), wire)?;
    wire.push(0).map_err(|_| ProtoError::BufferOverflow)?;
    Ok(())
}

/// Decodes one wire packet (WITHOUT the trailing `0x00` delimiter) using `scratch`, validating
/// framing and CRC. Returns the header and the payload slice (borrowed from `scratch`).
///
/// Version is not checked here (framing only); [`crate::codec::decode_message`] applies the
/// major-version policy.
///
/// # Errors
/// A typed [`ProtoError`] on any malformed input; never panics (SC-008).
pub fn decode_frame<'a>(
    packet: &[u8],
    scratch: &'a mut Vec<u8, MAX_FRAME>,
) -> Result<(Header, &'a [u8]), ProtoError> {
    cobs_decode(packet, scratch)?;
    let (header, crc_off) = {
        let b = scratch.as_slice();
        let n = b.len();
        if n < HEADER_LEN + CRC_LEN {
            return Err(ProtoError::TooShort);
        }
        if u16::from_le_bytes([b[0], b[1]]) != MAGIC {
            return Err(ProtoError::BadMagic);
        }
        let major = u16::from_le_bytes([b[2], b[3]]);
        let minor = u16::from_le_bytes([b[4], b[5]]);
        let seq = u16::from_le_bytes([b[6], b[7]]);
        let payload_len = u16::from_le_bytes([b[8], b[9]]) as usize;
        if payload_len > MAX_PAYLOAD {
            return Err(ProtoError::PayloadTooLarge);
        }
        if n != HEADER_LEN + payload_len + CRC_LEN {
            return Err(ProtoError::LengthMismatch);
        }
        let crc_off = HEADER_LEN + payload_len;
        let stored =
            u32::from_le_bytes([b[crc_off], b[crc_off + 1], b[crc_off + 2], b[crc_off + 3]]);
        if stored != crc32(&b[..crc_off]) {
            return Err(ProtoError::BadCrc);
        }
        (
            Header {
                version: ProtocolVersion::new(major, minor),
                seq,
                payload_len: payload_len as u16,
            },
            crc_off,
        )
    };
    Ok((header, &scratch[HEADER_LEN..crc_off]))
}
