#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
//! `kivori-protocol` — the Kivori device USB-serial wire protocol (ADR-0002).
//!
//! COBS framing + a fixed little-endian header + CRC-32 + `postcard` payloads, plus handshake,
//! version/capability negotiation, and sequence policy. `no_std` by default (the `std` feature is
//! host-only convenience). CRC-32 and COBS are implemented in-crate; see
//! [`frame`] for the byte layout and [`codec`] for message encode/decode.

pub mod codec;
pub mod error;
pub mod frame;
pub mod handshake;
pub mod message;
pub mod negotiate;

pub use codec::{decode_message, encode_message, SeqClass, SequenceTracker};
pub use error::{ByeReason, ErrorCategory, ProtoError};
pub use frame::{crc32, decode_frame, encode_frame, Header};
pub use handshake::{evaluate_hello_ack, HandshakeOutcome};
pub use message::{
    Bye, ControlId, DeviceId, Diagnostic, ErrorReport, FirmwareVersion, Health, Hello, HelloAck,
    InputEvent, InputKind, MascotActionApplied, Message, Nonce, Ping, PlayMascotAction, Pong,
    Presentation, Ready, SetState, StateReport,
};
pub use negotiate::negotiate;

/// Current protocol major version.
pub const PROTOCOL_MAJOR: u16 = 1;
/// Current protocol minor version.
pub const PROTOCOL_MINOR: u16 = 2;
/// Frame magic (`"KV"`, little-endian `0x4B56`).
pub const MAGIC: u16 = 0x4B56;
/// Maximum payload length, in bytes.
pub const MAX_PAYLOAD: usize = 512;
/// Fixed frame header length, in bytes (`magic + ver_major + ver_minor + seq + payload_len`).
pub const HEADER_LEN: usize = 10;
/// Trailing CRC length, in bytes.
pub const CRC_LEN: usize = 4;
/// Maximum raw (pre-COBS) frame length: header + payload + CRC.
pub const MAX_FRAME: usize = HEADER_LEN + MAX_PAYLOAD + CRC_LEN;
/// Maximum COBS-encoded wire length, including the `0x00` delimiter.
pub const MAX_WIRE: usize = MAX_FRAME + MAX_FRAME / 254 + 2;
