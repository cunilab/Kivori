//! Message-level encode/decode over the [`crate::frame`] layer, plus the sequence policy
//! (docs/architecture.md, wire protocol, §6).

use crate::error::ProtoError;
use crate::frame::{decode_frame, encode_frame, Header};
use crate::message::Message;
use crate::{MAX_FRAME, MAX_PAYLOAD, MAX_WIRE};
use heapless::Vec;
use kivori_model::ProtocolVersion;

/// Serializes `msg` with `postcard` and encodes a complete wire packet into `wire`.
///
/// # Errors
/// [`ProtoError::Postcard`] if serialization fails; framing errors otherwise.
pub fn encode_message(
    msg: &Message,
    version: ProtocolVersion,
    seq: u16,
    wire: &mut Vec<u8, MAX_WIRE>,
) -> Result<(), ProtoError> {
    let mut pbuf = [0u8; MAX_PAYLOAD];
    let payload = postcard::to_slice(msg, &mut pbuf).map_err(|_| ProtoError::Postcard)?;
    let header = Header {
        version,
        seq,
        payload_len: payload.len() as u16,
    };
    encode_frame(&header, payload, wire)
}

/// Decodes one wire packet into a [`Header`] and [`Message`], validating framing, CRC, and that the
/// frame's major version is in `supported_majors`.
///
/// # Errors
/// [`ProtoError::UnsupportedVersion`] if the major is unsupported; framing/postcard errors
/// otherwise. Never panics (SC-008).
pub fn decode_message(
    packet: &[u8],
    scratch: &mut Vec<u8, MAX_FRAME>,
    supported_majors: &[u16],
) -> Result<(Header, Message), ProtoError> {
    let (header, payload) = decode_frame(packet, scratch)?;
    if !supported_majors.contains(&header.version.major) {
        return Err(ProtoError::UnsupportedVersion);
    }
    let msg = postcard::from_bytes::<Message>(payload).map_err(|_| ProtoError::Postcard)?;
    Ok((header, msg))
}

/// Classification of an inbound sequence number relative to the last accepted one (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqClass {
    /// The first frame of the session.
    First,
    /// The expected next number (including `u16` wraparound).
    Ok,
    /// A repeat of the last accepted number — side effects must not be re-applied.
    Duplicate,
    /// A forward jump; the field is the number of skipped values.
    Gap(u16),
}

/// Tracks inbound sequence numbers and classifies each per the protocol's sequence policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct SequenceTracker {
    last: Option<u16>,
}

impl SequenceTracker {
    /// A fresh tracker with no accepted frames yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { last: None }
    }

    /// The last accepted sequence number, if any.
    #[must_use]
    pub const fn last(self) -> Option<u16> {
        self.last
    }

    /// Classifies `seq` and updates state. A `Duplicate` does not advance the tracker.
    pub fn classify(&mut self, seq: u16) -> SeqClass {
        match self.last {
            None => {
                self.last = Some(seq);
                SeqClass::First
            }
            Some(prev) if seq == prev => SeqClass::Duplicate,
            Some(prev) => {
                let expected = prev.wrapping_add(1);
                let class = if seq == expected {
                    SeqClass::Ok
                } else {
                    SeqClass::Gap(seq.wrapping_sub(expected))
                };
                self.last = Some(seq);
                class
            }
        }
    }
}
