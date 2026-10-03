//! The monotonic device clock (T073).
//!
//! Backed by esp-hal's system timer, so it is the same implementation on real hardware and in the
//! Wokwi simulator — only the surrounding board bring-up differs. Time is exposed as canonical integer
//! milliseconds ([`crate::ports::Clock`]), which is what the deterministic renderer consumes (ADR-0003).

use crate::ports::Clock;
use kivori_model::ElapsedMs;

/// Monotonic millisecond clock over the ESP32-C3 system timer.
#[derive(Debug, Clone, Copy, Default)]
pub struct EspClock;

impl EspClock {
    /// A clock reading the system timer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Clock for EspClock {
    fn now_ms(&self) -> ElapsedMs {
        let micros = esp_hal::time::Instant::now()
            .duration_since_epoch()
            .as_micros();
        // Wrapping (every ~49.7 days), never saturating: a clock stuck at `u32::MAX` would freeze
        // every timer, including the recovery hold. All consumers compare with `wrapping_sub`.
        (micros / 1000) as u32
    }
}
