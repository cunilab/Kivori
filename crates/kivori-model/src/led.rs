//! RGB LED settings the desktop sends (capability `RGB_LED_V1`). The device runs the animation
//! itself. Every enum is **append-only**: the postcard variant index is part of the wire encoding.

use serde::{Deserialize, Serialize};

/// What the LED does. Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LedMode {
    /// Dark. Also what the device does until the desktop has sent any settings.
    #[default]
    Off,
    /// The chosen colour, steady.
    Solid,
    /// The chosen colour, slowly fading in and out.
    Breathe,
    /// The whole colour wheel, one turn per period; the chosen colour is not used.
    Cycle,
    /// The chosen colour, overridden by the device's own state: muted microphone, no host, a press.
    Status,
    /// The colour of the buddy's current expression, with a soft breathe; the chosen colour is not used.
    Mood,
}

/// How fast an animated mode moves. Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LedSpeed {
    /// Breathe 6 s, colour wheel 20 s.
    Slow,
    /// Breathe 4 s, colour wheel 10 s.
    #[default]
    Medium,
    /// Breathe 2 s, colour wheel 5 s.
    Fast,
}
