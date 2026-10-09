//! Protocol error taxonomy. [`ErrorCategory`] and [`ByeReason`] travel on the wire (safe to log);
//! [`ProtoError`] is a local decode/encode result and is never transmitted.

use serde::{Deserialize, Serialize};

/// Category of a protocol error or diagnostic. Safe to log (no payload contents).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Serial I/O failure.
    Io,
    /// Handshake failure.
    Handshake,
    /// Version incompatibility.
    Version,
    /// Framing / COBS error.
    Framing,
    /// CRC mismatch.
    Checksum,
    /// A timeout elapsed.
    Timeout,
    /// The port/device was busy.
    Busy,
    /// A payload failed to decode.
    BadPayload,
}

/// Reason a peer is closing the session (wire value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ByeReason {
    /// The peer's protocol major version is unsupported.
    IncompatibleVersion,
    /// Normal shutdown.
    Shutdown,
    /// A protocol error occurred.
    ProtocolError,
    /// The host is going to sleep or locking; the device shows Sleeping until a new `Hello`
    /// (needs `HOST_TAKEOVERS_V1`).
    HostSleeping,
    /// The host is about to flash the device; it shows Updating for at most 120 s
    /// (needs `HOST_TAKEOVERS_V1`).
    FirmwareUpdate,
}

/// A local framing/codec error. Returned by decode/encode; never sent on the wire and never panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoError {
    /// An output buffer was too small.
    BufferOverflow,
    /// COBS decoding failed (unexpected `0x00` or truncated block).
    Cobs,
    /// The decoded frame is shorter than the minimum header + CRC.
    TooShort,
    /// The frame magic did not match.
    BadMagic,
    /// The frame's protocol major version is not supported.
    UnsupportedVersion,
    /// The declared payload length exceeds `MAX_PAYLOAD`.
    PayloadTooLarge,
    /// The declared payload length does not match the received frame length.
    LengthMismatch,
    /// The CRC did not validate.
    BadCrc,
    /// The `postcard` payload failed to (de)serialize.
    Postcard,
}
