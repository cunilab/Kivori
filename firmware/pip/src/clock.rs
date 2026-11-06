//! The monotonic device clock (T073).
//!
//! Backed by the RP2040's 1 MHz timer. Time is exposed as canonical integer milliseconds
//! ([`crate::ports::Clock`]), which is what the deterministic renderer consumes (ADR-0003).

use crate::ports::Clock;
use kivori_model::ElapsedMs;
use rp2040_hal::pac;

/// Monotonic millisecond clock over the RP2040 timer. Reading is stateless, so the interrupt
/// handler builds its own; the timer must have been started by [`crate::bsp::init`] first.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimerClock;

impl TimerClock {
    /// A clock reading the timer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Clock for TimerClock {
    fn now_ms(&self) -> ElapsedMs {
        // SAFETY: read-only access to the free-running counter, as `rp2040_hal::timer::Timer` does.
        let timer = unsafe { &*pac::TIMER::PTR };
        // Glitch-free 64-bit read: retry if the high word rolled over between the two reads.
        let mut high = timer.timerawh().read().bits();
        let micros = loop {
            let low = timer.timerawl().read().bits();
            let again = timer.timerawh().read().bits();
            if high == again {
                break (u64::from(high) << 32) | u64::from(low);
            }
            high = again;
        };
        // Wrapping (every ~49.7 days), never saturating: a clock stuck at `u32::MAX` would freeze
        // every timer, including the recovery hold. All consumers compare with `wrapping_sub`.
        (micros / 1000) as u32
    }
}
