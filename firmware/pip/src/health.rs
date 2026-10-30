//! Heartbeat and safe diagnostics (docs/architecture.md, wire protocol; FR-031/032; ADR-0005).
//!
//! `Pong` echoes the desktop's ping timestamp and reports uptime; `Health` carries only free SRAM; and
//! [`DeviceDiagnostic`] is **the safe-category allowlist made a type** — the device mirror of the desktop's
//! `SafeDiagnostic`. Each variant maps to one fixed `(ErrorCategory, code)` pair, and no variant can carry
//! payload bytes, a raw device identity, a path, or a free-form string, so an unsafe diagnostic is
//! unrepresentable rather than merely discouraged.

use kivori_model::ElapsedMs;
use kivori_protocol::{Diagnostic, ErrorCategory, Health, Pong, ProtoError, SeqClass};

/// Builds the `Pong` reply to a `Ping` whose timestamp was `ping_t_ms`, at device `uptime_ms`.
#[must_use]
pub const fn build_pong(ping_t_ms: u32, uptime_ms: ElapsedMs) -> Pong {
    Pong {
        t_ms_echo: ping_t_ms,
        uptime_ms,
    }
}

/// Builds a safe `Health` report from the current free SRAM, in bytes.
#[must_use]
pub const fn build_health(free_bytes: u32) -> Health {
    Health { free_bytes }
}

/// Why the decoder rejected a frame, reduced to the categories the wire protocol defines.
///
/// Deliberately coarser than [`ProtoError`]: the desktop needs to know *what class* of thing went wrong,
/// never the offending bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// COBS/length/magic framing problem.
    Framing,
    /// CRC-32 mismatch.
    Checksum,
    /// Unsupported protocol major version.
    Version,
    /// The postcard payload did not decode, or was too large.
    Payload,
}

impl RejectReason {
    /// Maps a decoder error onto a safe category, discarding everything else about it.
    #[must_use]
    pub const fn of(error: &ProtoError) -> Self {
        match error {
            ProtoError::Cobs
            | ProtoError::TooShort
            | ProtoError::BadMagic
            | ProtoError::LengthMismatch
            | ProtoError::BufferOverflow => Self::Framing,
            ProtoError::BadCrc => Self::Checksum,
            ProtoError::UnsupportedVersion => Self::Version,
            ProtoError::PayloadTooLarge | ProtoError::Postcard => Self::Payload,
        }
    }

    /// The wire category for this reason.
    #[must_use]
    pub const fn category(self) -> ErrorCategory {
        match self {
            Self::Framing => ErrorCategory::Framing,
            Self::Checksum => ErrorCategory::Checksum,
            Self::Version => ErrorCategory::Version,
            Self::Payload => ErrorCategory::BadPayload,
        }
    }
}

/// The device-side safe-diagnostic allowlist (T075; FR-031, FR-032; ADR-0005 §2).
///
/// Every variant is a fixed, bounded fact about the device's own operation. There is intentionally no
/// variant carrying bytes, an identifier, or text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceDiagnostic {
    /// The decoder rejected an inbound frame; the device stayed up and resynchronised.
    FrameRejected(RejectReason),
    /// A forward jump in sequence numbers was observed (frames were lost in transit).
    SequenceGap,
    /// A tile write to the display sink failed.
    DisplayFault,
    /// The session ended: a `Bye` arrived, or the transport failed.
    LinkLost,
}

impl DeviceDiagnostic {
    /// The wire category.
    #[must_use]
    pub const fn category(self) -> ErrorCategory {
        match self {
            Self::FrameRejected(reason) => reason.category(),
            Self::SequenceGap => ErrorCategory::Framing,
            Self::DisplayFault | Self::LinkLost => ErrorCategory::Io,
        }
    }

    /// A stable, category-specific code. These are part of the observable contract, so they are fixed
    /// numbers rather than a discriminant that would shift if a variant were reordered.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::FrameRejected(RejectReason::Framing) => 1,
            Self::FrameRejected(RejectReason::Checksum) => 2,
            Self::FrameRejected(RejectReason::Version) => 3,
            Self::FrameRejected(RejectReason::Payload) => 4,
            Self::SequenceGap => 5,
            Self::DisplayFault => 6,
            Self::LinkLost => 7,
        }
    }

    /// A short, allowlisted label for local logging. Never interpolates any runtime value.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FrameRejected(RejectReason::Framing) => "frame-rejected-framing",
            Self::FrameRejected(RejectReason::Checksum) => "frame-rejected-checksum",
            Self::FrameRejected(RejectReason::Version) => "frame-rejected-version",
            Self::FrameRejected(RejectReason::Payload) => "frame-rejected-payload",
            Self::SequenceGap => "sequence-gap",
            Self::DisplayFault => "display-fault",
            Self::LinkLost => "link-lost",
        }
    }
}

/// Builds the wire `Diagnostic` for an allowlisted device diagnostic.
#[must_use]
pub const fn build_diagnostic(diagnostic: DeviceDiagnostic) -> Diagnostic {
    Diagnostic {
        category: diagnostic.category(),
        code: diagnostic.code(),
    }
}

/// Maps a sequence classification onto a diagnostic, when it warrants one.
///
/// `First`, `Ok`, and `Duplicate` are normal traffic and produce nothing; only a gap is reportable.
#[must_use]
pub const fn diagnostic_for_sequence(class: SeqClass) -> Option<DeviceDiagnostic> {
    match class {
        SeqClass::Gap(_) => Some(DeviceDiagnostic::SequenceGap),
        SeqClass::First | SeqClass::Ok | SeqClass::Duplicate => None,
    }
}
