//! Leaf input value types shared by firmware, the wire protocol, and the desktop core.

use serde::{Deserialize, Serialize};

/// Direction of one completed, validated logical detent.
///
/// Electrical quarter-step transitions are NOT directions; only a fully traversed
/// detent produces one (user-story-contract invariant 46).
///
/// Wire-significant (nested inside `kivori_protocol::message::InputEvent`): the variant index is
/// part of the postcard wire encoding. **Append-only** — variants may only be added at the end,
/// never reordered or removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Clockwise detent.
    Cw,
    /// Counter-clockwise detent.
    Ccw,
}

/// Instantaneous encoder levels sampled from hardware.
///
/// `sw` is carried but unused in Slice 002; the push-switch gesture machine is a later slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputLevels {
    /// Quadrature channel A level.
    pub a: bool,
    /// Quadrature channel B level.
    pub b: bool,
    /// Push-switch level. Unused until the gesture machine (later slice) reads it.
    pub sw: bool,
}
