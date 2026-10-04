//! Leaf input value types shared by firmware, the wire protocol, and the desktop core.

use serde::{Deserialize, Serialize};

/// Direction of one completed, validated logical detent.
///
/// Electrical quarter-step transitions are NOT directions; only a fully traversed
/// detent produces one (product invariant 46).
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
/// Pressed is `true` for every switch, whatever the electrical polarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputLevels {
    /// Quadrature channel A level.
    pub a: bool,
    /// Quadrature channel B level.
    pub b: bool,
    /// The encoder's push switch.
    pub sw: bool,
    /// The three contextual buttons, left to right (`false` on boards without them).
    pub keys: [bool; 3],
}
