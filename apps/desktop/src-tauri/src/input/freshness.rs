//! Same-session input freshness (issue #26): how old an input is when the desktop is about to
//! act on it.
//!
//! The device stamps every `InputEvent` with its own `u32` ms clock, which wraps and is not the
//! host's clock. [`DeviceClock`] estimates the device-to-host offset from heartbeat `Pong`s and
//! converts a stamp into an age. Everything is pure with injected times (host ms are whatever
//! monotonic `u32` the caller uses, the same one its `Ping`s carry).
//!
//! Until a `Pong` has been seen there is no offset, so no age and nothing is dropped (fail open).

use std::time::Duration;

/// A discrete action older than this when the desktop is about to run it is dropped, never run
/// (docs/product.md, input and gestures). Detents and gesture boundaries are exempt.
pub const MAX_ACTION_AGE: Duration = Duration::from_millis(750);

/// A `Pong` whose round trip was longer than this says too little about the offset (its error is
/// up to half the round trip), so it is ignored and the previous estimate stays.
const MAX_SAMPLE_RTT_MS: u32 = 500;

/// The estimated device-to-host clock offset of the current connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeviceClock {
    /// `device_ms - host_ms` (wrapping), from the latest usable sample.
    offset_ms: Option<u32>,
}

impl DeviceClock {
    /// Records a `Pong`: `ping_ms` is the host time stamped into the `Ping` it echoes,
    /// `received_ms` the host time it arrived, `uptime_ms` the device clock when it replied.
    /// The device replied about half a round trip before `received_ms`. The latest usable
    /// sample wins, so slow drift between the two crystals never accumulates.
    pub fn on_pong(&mut self, ping_ms: u32, received_ms: u32, uptime_ms: u32) {
        let rtt = received_ms.wrapping_sub(ping_ms);
        if rtt > MAX_SAMPLE_RTT_MS {
            return;
        }
        let device_at_receipt = uptime_ms.wrapping_add(rtt / 2);
        self.offset_ms = Some(device_at_receipt.wrapping_sub(received_ms));
    }

    /// Forgets the estimate (a new connection: the device clock may have restarted).
    pub fn reset(&mut self) {
        self.offset_ms = None;
    }

    /// How long ago the device stamped `device_ms`, at host time `host_now_ms`. `None` while
    /// there is no offset. A stamp that lands in the future (estimation error) is age zero.
    #[must_use]
    pub fn age(&self, device_ms: u32, host_now_ms: u32) -> Option<Duration> {
        let device_now = host_now_ms.wrapping_add(self.offset_ms?);
        let age = device_now.wrapping_sub(device_ms);
        // Wrapping distance read as signed: the top half of the range means "in the future".
        Some(if age > u32::MAX / 2 {
            Duration::ZERO
        } else {
            Duration::from_millis(u64::from(age))
        })
    }
}

/// When one input reached the desktop's decision point, for the freshness checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Freshness {
    pub clock: DeviceClock,
    /// Host time (ms) at which the batch of inputs was taken from the session.
    pub host_now_ms: u32,
}

impl Freshness {
    /// The age of an input stamped `device_ms`, if it can be known.
    #[must_use]
    pub fn age(&self, device_ms: u32) -> Option<Duration> {
        self.clock.age(device_ms, self.host_now_ms)
    }

    /// Whether a discrete action stamped `device_ms` is too old to run. Unknown age is fresh.
    #[must_use]
    pub fn is_stale(&self, device_ms: u32) -> bool {
        self.age(device_ms).is_some_and(|age| age > MAX_ACTION_AGE)
    }

    /// How long a discrete action stamped `device_ms` may still wait before it is too old.
    #[must_use]
    pub fn remaining(&self, device_ms: u32) -> Duration {
        MAX_ACTION_AGE.saturating_sub(self.age(device_ms).unwrap_or(Duration::ZERO))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synced(offset_probe: (u32, u32, u32)) -> DeviceClock {
        let mut clock = DeviceClock::default();
        clock.on_pong(offset_probe.0, offset_probe.1, offset_probe.2);
        clock
    }

    #[test]
    fn no_pong_means_no_age_and_nothing_stale() {
        let freshness = Freshness {
            clock: DeviceClock::default(),
            host_now_ms: 1_000_000,
        };
        assert_eq!(freshness.age(0), None);
        assert!(!freshness.is_stale(0));
        assert_eq!(freshness.remaining(0), MAX_ACTION_AGE);
    }

    #[test]
    fn offset_uses_half_the_round_trip() {
        // Ping at host 1000, Pong back at 1040; the device replied at its 5020, so at host
        // receipt its clock read about 5040: offset 4000.
        let clock = synced((1_000, 1_040, 5_020));
        // A stamp of 5_000 seen at host 1_100 (device 5_100) is 100 ms old.
        assert_eq!(clock.age(5_000, 1_100), Some(Duration::from_millis(100)));
    }

    #[test]
    fn a_future_stamp_is_age_zero() {
        let clock = synced((1_000, 1_000, 5_000));
        assert_eq!(clock.age(5_300, 1_100), Some(Duration::ZERO));
    }

    #[test]
    fn age_survives_u32_wrap_of_the_device_clock() {
        // Device clock is 100 ms short of wrapping at host 1_000.
        let clock = synced((1_000, 1_000, u32::MAX - 99));
        // Stamped just before the wrap, seen 300 ms later (device clock wrapped to ~200).
        let stamp = u32::MAX - 99;
        assert_eq!(clock.age(stamp, 1_300), Some(Duration::from_millis(300)));
        // Stamped after the wrap.
        assert_eq!(clock.age(50, 1_300), Some(Duration::from_millis(150)));
    }

    #[test]
    fn age_survives_u32_wrap_of_the_host_clock() {
        let host = u32::MAX - 10;
        let clock = synced((host, host, 7_000));
        assert_eq!(
            clock.age(7_000, host.wrapping_add(900)),
            Some(Duration::from_millis(900))
        );
    }

    #[test]
    fn a_slow_pong_is_ignored_and_the_previous_offset_stays() {
        let mut clock = synced((1_000, 1_000, 5_000));
        clock.on_pong(2_000, 2_900, 9_999);
        assert_eq!(clock.age(5_100, 1_200), Some(Duration::from_millis(100)));
    }

    #[test]
    fn reset_forgets_the_offset() {
        let mut clock = synced((1_000, 1_000, 5_000));
        clock.reset();
        assert_eq!(clock.age(0, 10), None);
    }

    #[test]
    fn staleness_is_strictly_beyond_the_limit() {
        let freshness = Freshness {
            clock: synced((0, 0, 0)),
            host_now_ms: 2_000,
        };
        assert!(!freshness.is_stale(1_250));
        assert!(freshness.is_stale(1_249));
        assert_eq!(freshness.remaining(1_500), Duration::from_millis(250));
        assert_eq!(freshness.remaining(0), Duration::ZERO);
    }
}
