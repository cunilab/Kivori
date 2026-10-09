//! Heartbeat liveness policy (FR-007).
//!
//! The run loop sends a `Ping` each interval and calls [`HeartbeatMonitor::on_ping_sent`]; each
//! `Pong` resets the miss counter. When the unanswered count reaches the threshold, the connection is
//! declared timed out. Pure and timer-free — the run loop owns the clock.

/// Default number of consecutive missed replies before a heartbeat timeout.
pub const DEFAULT_MISS_THRESHOLD: u32 = 3;

/// Tracks consecutive unanswered heartbeats.
#[derive(Debug, Clone, Copy)]
pub struct HeartbeatMonitor {
    misses: u32,
    threshold: u32,
    /// The host time stamped into the latest unanswered `Ping`.
    last_ping_ms: Option<u32>,
    /// Round trip of the most recent answered `Ping`.
    rtt_ms: Option<u32>,
}

impl HeartbeatMonitor {
    /// A monitor with the given consecutive-miss threshold (clamped to at least 1).
    #[must_use]
    pub const fn new(threshold: u32) -> Self {
        Self {
            misses: 0,
            last_ping_ms: None,
            rtt_ms: None,
            threshold: if threshold == 0 { 1 } else { threshold },
        }
    }

    /// Records that a `Ping` stamped `t_ms` (host ms) was sent (a not-yet-answered heartbeat).
    pub fn on_ping_sent(&mut self, t_ms: u32) {
        self.misses = self.misses.saturating_add(1);
        self.last_ping_ms = Some(t_ms);
    }

    /// Records a `Pong` that echoed `t_ms_echo`, received at host time `now_ms`, clearing the miss
    /// counter. The round trip is only taken from an echo of the latest `Ping`, so a late reply to
    /// an older one cannot report a stretched figure.
    pub fn on_pong(&mut self, t_ms_echo: u32, now_ms: u32) {
        self.misses = 0;
        if self.last_ping_ms == Some(t_ms_echo) {
            self.rtt_ms = Some(now_ms.wrapping_sub(t_ms_echo));
            self.last_ping_ms = None;
        }
    }

    /// The round trip of the latest answered `Ping` in ms (`None` before the first one).
    #[must_use]
    pub const fn rtt_ms(&self) -> Option<u32> {
        self.rtt_ms
    }

    /// The current consecutive-miss count.
    #[must_use]
    pub const fn misses(&self) -> u32 {
        self.misses
    }

    /// Whether the miss count has reached the threshold (the caller then raises `HeartbeatTimeout`).
    #[must_use]
    pub const fn timed_out(&self) -> bool {
        self.misses >= self.threshold
    }
}

impl Default for HeartbeatMonitor {
    fn default() -> Self {
        Self::new(DEFAULT_MISS_THRESHOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtt_comes_from_the_echo_of_the_latest_ping() {
        let mut monitor = HeartbeatMonitor::default();
        assert_eq!(monitor.rtt_ms(), None);
        monitor.on_ping_sent(1_000);
        monitor.on_pong(1_000, 1_012);
        assert_eq!(monitor.rtt_ms(), Some(12));
        assert_eq!(monitor.misses(), 0);
    }

    #[test]
    fn a_late_echo_of_an_older_ping_keeps_the_last_round_trip() {
        let mut monitor = HeartbeatMonitor::default();
        monitor.on_ping_sent(1_000);
        monitor.on_pong(1_000, 1_010);
        monitor.on_ping_sent(2_000);
        monitor.on_pong(1_000, 2_500);
        assert_eq!(monitor.rtt_ms(), Some(10));
    }
}
