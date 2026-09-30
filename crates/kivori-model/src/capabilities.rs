//! Capability bitset for additive, minor-version feature negotiation (data-model §3).
//!
//! Concrete capability flags are allocated centrally as associated constants below (see the bit
//! registry comment in `impl Capabilities`) and never reused once retired. The representation is an
//! opaque `u32` bitset; unknown/higher bits are preserved and ignored by older peers (append-only
//! forward-compatibility). The key negotiation operation is [`Capabilities::intersection`] (features
//! advertised by both peers).

use serde::{Deserialize, Serialize};

/// A set of protocol capability flags, represented as an opaque `u32` bitset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Capabilities(u32);

impl Capabilities {
    /// The empty capability set.
    pub const NONE: Capabilities = Capabilities(0);
    // CAPABILITY BIT REGISTRY — allocate centrally, never reuse a retired bit.
    //   bit 0  MASCOT_INTERACTION  mascot animation (PR #3)
    //   bit 1  PHYSICAL_INPUT_V1   Slice 002
    //   bit 2  PRESENTATION_V1     Slice 002

    /// Bit 0 — device accepts deterministic social mascot actions and returns applied-time
    /// acknowledgments.
    pub const MASCOT_INTERACTION: Capabilities = Capabilities(1 << 0);

    /// Bit 1 — the device may emit `InputEvent` (Slice 002, rotary input).
    pub const PHYSICAL_INPUT_V1: Capabilities = Capabilities(1 << 1);

    /// Bit 2 — the device renders semantic `Presentation` (Slice 002).
    pub const PRESENTATION_V1: Capabilities = Capabilities(1 << 2);

    /// Creates a capability set from a raw bitmask.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Capabilities(bits)
    }

    /// Returns the raw bitmask.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns `true` if the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if every flag in `other` is present in `self`.
    #[must_use]
    pub const fn contains(self, other: Capabilities) -> bool {
        (self.0 & other.0) == other.0
    }

    /// The negotiated set: flags advertised by **both** peers (bitwise intersection).
    #[must_use]
    pub const fn intersection(self, other: Capabilities) -> Capabilities {
        Capabilities(self.0 & other.0)
    }

    /// The union of two capability sets (bitwise or).
    #[must_use]
    pub const fn union(self, other: Capabilities) -> Capabilities {
        Capabilities(self.0 | other.0)
    }
}
