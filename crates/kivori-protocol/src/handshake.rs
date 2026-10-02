//! Handshake evaluation helpers (docs/architecture.md, wire protocol). Pure decisions; the connection actor
//! (Phase 9) orchestrates timing and I/O.

use crate::message::{Hello, HelloAck, Ready};
use crate::negotiate::negotiate;
use kivori_model::ProtocolVersion;

/// The outcome of evaluating a device's `HelloAck` (with the version taken from the frame header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeOutcome {
    /// Compatible; proceed by sending the given [`Ready`].
    Compatible(Ready),
    /// The device's protocol major version is unsupported.
    Incompatible {
        /// The device's reported major version.
        device_major: u16,
    },
    /// The device echoed the wrong nonce — identity not confirmed.
    BadNonce,
}

/// Evaluates a `HelloAck` against the `Hello` that was sent, the device's protocol version (from the
/// frame header), the desktop's version, and the desktop's supported majors. Pure — performs no I/O.
#[must_use]
pub fn evaluate_hello_ack(
    sent: &Hello,
    ack: &HelloAck,
    device_version: ProtocolVersion,
    desktop_version: ProtocolVersion,
    supported_majors: &[u16],
) -> HandshakeOutcome {
    if ack.nonce_echo != sent.nonce {
        return HandshakeOutcome::BadNonce;
    }
    if !supported_majors.contains(&device_version.major) {
        return HandshakeOutcome::Incompatible {
            device_major: device_version.major,
        };
    }
    let (negotiated_minor, negotiated_caps) = negotiate(
        desktop_version.minor,
        device_version.minor,
        sent.desktop_caps,
        ack.device_caps,
    );
    HandshakeOutcome::Compatible(Ready {
        negotiated_minor,
        negotiated_caps,
    })
}
