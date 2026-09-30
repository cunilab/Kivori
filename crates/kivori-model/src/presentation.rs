//! Semantic presentation value types.
//!
//! These live in `kivori-model` rather than `kivori-protocol` so the shared
//! renderer can draw from them without depending on the wire crate.

use serde::{Deserialize, Serialize};

/// The underlying state that remains true beneath any transient overlay.
///
/// Wire-significant (nested inside `kivori_protocol::message::Presentation`): the variant index is
/// part of the postcard wire encoding. **Append-only** — variants may only be added at the end,
/// never reordered or removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimaryState {
    /// No active interaction; nothing to show beyond the base scene.
    Idle,
    /// An interaction (e.g. a rotary gesture) is currently open.
    Active,
    /// The device or session is in an error state.
    Error,
    /// The state has not yet been established (e.g. before the first report).
    Unknown,
}

/// Which kind of value a [`ValueDisplay`] represents.
///
/// Wire-significant (nested inside `kivori_protocol::message::Presentation` via [`ValueDisplay`]):
/// the variant index is part of the postcard wire encoding. **Append-only** — variants may only be
/// added at the end, never reordered or removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueKind {
    /// Master volume, 0..=100.
    Volume,
}

/// How well the displayed value is known.
///
/// An optimistic local preview MUST NOT be rendered as observed desktop truth
/// (user-story-contract invariant 3).
///
/// Wire-significant (nested inside `kivori_protocol::message::Presentation` via [`ValueDisplay`]):
/// the variant index is part of the postcard wire encoding. **Append-only** — variants may only be
/// added at the end, never reordered or removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueConfidence {
    /// Kivori's local target during an open gesture; not yet observed from the OS.
    Preview,
    /// The value the OS reported back.
    Confirmed,
    /// Dispatched, but read-back was unavailable.
    Unverified,
}

/// A transient value overlay. Expires after `Presentation::transient_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueDisplay {
    /// Which value this overlay represents.
    pub kind: ValueKind,
    /// 0..=100.
    pub current_percent: u8,
    /// How well this value is known.
    pub confidence: ValueConfidence,
    /// Whether the value is currently pinned at an endpoint (0 or 100).
    pub at_boundary: bool,
}
