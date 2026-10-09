//! Protocol version value type and the major-match compatibility rule (data-model §3, FR-003).
//!
//! Wire framing/codec live in `kivori-protocol`; only the value types and the pure
//! compatibility/negotiation helpers belong here.

use serde::{Deserialize, Serialize};

/// A protocol version `major.minor`. Compatibility is decided by matching `major` (clarified);
/// minor differences are backward-compatible and capability-negotiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Major version. A change is backward-incompatible; valid shipped majors are `>= 1`.
    pub major: u16,
    /// Minor version. Additive, backward-compatible changes bump the minor.
    pub minor: u16,
}

/// Reason a [`ProtocolVersion`] is structurally invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionError {
    /// `major` was `0` (reserved; shipped protocol versions start at major `1`).
    ZeroMajor,
}

impl ProtocolVersion {
    /// Creates a protocol version.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Validates structural invariants (currently: `major >= 1`).
    ///
    /// # Errors
    /// Returns [`VersionError::ZeroMajor`] if `major == 0`.
    pub const fn validate(self) -> Result<(), VersionError> {
        if self.major == 0 {
            return Err(VersionError::ZeroMajor);
        }
        Ok(())
    }

    /// Returns `true` if a peer advertising `self` is compatible with a peer that supports the given
    /// major versions: the major must match one supported major (minor differences are allowed).
    #[must_use]
    pub fn is_compatible_with(self, supported_majors: &[u16]) -> bool {
        supported_majors.contains(&self.major)
    }

    /// The negotiated minor for a session: the lower of the two peers' minors.
    #[must_use]
    pub const fn negotiate_minor(a: ProtocolVersion, b: ProtocolVersion) -> u16 {
        if a.minor < b.minor {
            a.minor
        } else {
            b.minor
        }
    }
}
