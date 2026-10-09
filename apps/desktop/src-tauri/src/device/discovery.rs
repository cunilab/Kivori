//! Candidate serial-port discovery (FR-001, FR-036).
//!
//! Kivori devices are found by USB VID/PID, never a fixed COM port. Port enumeration itself lives in
//! the serial adapter; this module holds the pure, testable filter over enumerated ports and the
//! [`CandidateRotator`] that decides which candidate to try next. A VID/PID match only nominates a
//! port: a stock ESP32-C3 reports the same ids, so only a successful handshake makes a port the
//! active device. Port names are opaque, never logged and never leave this process.

use crate::device::reconnect::{base_delay_ms, Backoff};
use core::time::Duration;

/// A `(vendor, product)` USB id pair.
pub type UsbId = (u16, u16);

/// Espressif's USB vendor id (the ESP32-C3 USB Serial/JTAG).
pub const KIVORI_VID: u16 = 0x303A;
/// Kivori's USB product id.
pub const KIVORI_PID: u16 = 0x1001;

/// The default VID/PID allowlist (a single Kivori id; widen it to onboard new hardware).
pub const DEFAULT_ALLOWLIST: &[UsbId] = &[(KIVORI_VID, KIVORI_PID)];

/// An enumerated serial port and its USB identity (absent for non-USB ports).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortCandidate {
    /// OS port name (e.g. `COM7`, `/dev/ttyACM0`). Opaque — never the selection key.
    pub port_name: String,
    /// USB vendor id, if the port is a USB device.
    pub vid: Option<u16>,
    /// USB product id, if the port is a USB device.
    pub pid: Option<u16>,
}

impl PortCandidate {
    /// Creates a candidate.
    #[must_use]
    pub fn new(port_name: impl Into<String>, vid: Option<u16>, pid: Option<u16>) -> Self {
        Self {
            port_name: port_name.into(),
            vid,
            pid,
        }
    }
}

/// Whether `port` matches the allowlist (both VID and PID must be present and listed).
#[must_use]
pub fn is_candidate(port: &PortCandidate, allowlist: &[UsbId]) -> bool {
    matches!((port.vid, port.pid), (Some(v), Some(p)) if allowlist.contains(&(v, p)))
}

/// Returns the subset of `ports` matching `allowlist`, preserving order.
#[must_use]
pub fn filter_candidates<'a>(
    ports: &'a [PortCandidate],
    allowlist: &[UsbId],
) -> Vec<&'a PortCandidate> {
    ports
        .iter()
        .filter(|p| is_candidate(p, allowlist))
        .collect()
}

/// Consecutive handshake timeouts after which a port is passed over while another candidate exists.
pub const TIMEOUTS_BEFORE_DEMOTION: u32 = 3;

/// Failure bookkeeping for one enumerated candidate port.
#[derive(Debug, Clone)]
struct PortHistory {
    name: String,
    /// Per-port backoff; reset by a successful handshake.
    backoff: Backoff,
    /// Earliest time the port may be tried again (`None` = never failed, eligible now).
    retry_at: Option<Duration>,
    /// Consecutive handshake timeouts since the last success.
    timeouts: u32,
    /// Hard skip for as long as the port stays enumerated (e.g. incompatible protocol major).
    skipped: bool,
}

/// Picks which candidate port to try next, round-robin, so one unresponsive board never starves
/// another behind it.
///
/// Pure: the caller injects `now` (elapsed since the device task started) and the enumerated ports.
/// Per-port failures back off on the [`crate::device::reconnect`] schedule. A port that vanishes from
/// enumeration is forgotten, which also clears its skip mark, so an unplug and replug is a fresh
/// start. With a single never-failed candidate, [`CandidateRotator::next`] returns it immediately.
#[derive(Debug, Clone, Default)]
pub struct CandidateRotator {
    ports: Vec<PortHistory>,
    /// The port handed out last; the next pick starts after it.
    last: Option<String>,
}

impl CandidateRotator {
    /// A rotator that has seen no ports.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Syncs with `ports` (filtered by `allowlist`) and returns the next eligible port name.
    ///
    /// Eligible means not hard-skipped, past its own backoff, and not demoted by repeated handshake
    /// timeouts while another non-skipped candidate exists. Returns `None` when nothing is eligible.
    pub fn next(
        &mut self,
        ports: &[PortCandidate],
        allowlist: &[UsbId],
        now: Duration,
    ) -> Option<String> {
        let names: Vec<&str> = filter_candidates(ports, allowlist)
            .into_iter()
            .map(|p| p.port_name.as_str())
            .collect();
        self.ports.retain(|h| names.contains(&h.name.as_str()));
        for name in &names {
            if !self.ports.iter().any(|h| h.name == *name) {
                self.ports.push(PortHistory {
                    name: (*name).to_string(),
                    backoff: Backoff::new(),
                    retry_at: None,
                    timeouts: 0,
                    skipped: false,
                });
            }
        }
        // Visit in enumeration order, starting after the last pick.
        let start = self
            .last
            .as_deref()
            .and_then(|last| names.iter().position(|n| *n == last))
            .map_or(0, |i| i + 1);
        let others_available =
            |history: &Self, name: &str| history.ports.iter().any(|h| h.name != name && !h.skipped);
        let pick = (0..names.len())
            .map(|offset| names[(start + offset) % names.len()])
            .find(|name| {
                let h = self.ports.iter().find(|h| h.name == *name).expect("synced");
                !h.skipped
                    && h.retry_at.is_none_or(|at| now >= at)
                    && !(h.timeouts >= TIMEOUTS_BEFORE_DEMOTION && others_available(self, name))
            })
            .map(str::to_string);
        if let Some(name) = &pick {
            self.last = Some(name.clone());
        }
        pick
    }

    /// Records a failed attempt (open error, I/O loss, heartbeat loss) and schedules its backoff.
    pub fn record_failure(&mut self, name: &str, now: Duration) {
        if let Some(h) = self.ports.iter_mut().find(|h| h.name == name) {
            h.retry_at = Some(now + h.backoff.next_delay());
        }
    }

    /// Records a handshake timeout: a failure that also counts toward demoting the port.
    pub fn record_handshake_timeout(&mut self, name: &str, now: Duration) {
        self.record_failure(name, now);
        if let Some(h) = self.ports.iter_mut().find(|h| h.name == name) {
            h.timeouts = h.timeouts.saturating_add(1);
        }
    }

    /// Skips `name` until it disappears from enumeration (incompatible protocol major).
    pub fn mark_skipped(&mut self, name: &str) {
        if let Some(h) = self.ports.iter_mut().find(|h| h.name == name) {
            h.skipped = true;
        }
    }

    /// Whether any currently known (enumerated) port is hard-skipped.
    #[must_use]
    pub fn any_skipped(&self) -> bool {
        self.ports.iter().any(|h| h.skipped)
    }

    /// Records a successful handshake: the port's failure history is cleared.
    pub fn record_success(&mut self, name: &str) {
        if let Some(h) = self.ports.iter_mut().find(|h| h.name == name) {
            h.backoff.reset();
            h.retry_at = None;
            h.timeouts = 0;
        }
    }

    /// How long until the earliest non-skipped known port may be tried (zero if one is ready now).
    /// `None` when every known port is skipped or none is known.
    #[must_use]
    pub fn earliest_retry_in(&self, now: Duration) -> Option<Duration> {
        self.ports
            .iter()
            .filter(|h| !h.skipped)
            .map(|h| {
                h.retry_at
                    .map_or(Duration::ZERO, |at| at.saturating_sub(now))
            })
            .min()
    }

    /// The backoff a port's next failure will use, in milliseconds (for diagnostics and tests).
    #[must_use]
    pub fn next_delay_ms(&self, name: &str) -> Option<u64> {
        self.ports
            .iter()
            .find(|h| h.name == name)
            .map(|h| base_delay_ms(h.backoff.attempt()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    fn port(name: &str) -> PortCandidate {
        PortCandidate::new(name, Some(KIVORI_VID), Some(KIVORI_PID))
    }

    fn next(r: &mut CandidateRotator, names: &[&str], now: Duration) -> Option<String> {
        let ports: Vec<_> = names.iter().map(|n| port(n)).collect();
        r.next(&ports, DEFAULT_ALLOWLIST, now)
    }

    #[test]
    fn single_fresh_candidate_is_returned_immediately_and_repeatedly() {
        let mut r = CandidateRotator::new();
        assert_eq!(next(&mut r, &["A"], MS(0)).as_deref(), Some("A"));
        assert_eq!(next(&mut r, &["A"], MS(0)).as_deref(), Some("A"));
        assert_eq!(r.earliest_retry_in(MS(0)), Some(Duration::ZERO));
    }

    #[test]
    fn silent_board_ahead_of_kivori_is_rotated_past() {
        let mut r = CandidateRotator::new();
        let ports = ["silent", "kivori"];
        let mut now = MS(0);
        let first = next(&mut r, &ports, now).unwrap();
        assert_eq!(first, "silent");
        r.record_handshake_timeout(&first, now);
        now += MS(10);
        // The silent board is backing off, so the next pick is the other board.
        assert_eq!(next(&mut r, &ports, now).as_deref(), Some("kivori"));
    }

    #[test]
    fn repeated_timeouts_demote_a_port_while_another_exists() {
        let mut r = CandidateRotator::new();
        let ports = ["silent", "kivori"];
        let mut now = MS(0);
        let _ = next(&mut r, &ports, now);
        for _ in 0..TIMEOUTS_BEFORE_DEMOTION {
            r.record_handshake_timeout("silent", now);
            now += MS(10_000);
        }
        r.record_failure("kivori", now);
        // Kivori is backing off; the demoted silent board must still not be picked.
        assert_eq!(next(&mut r, &ports, now), None);
        now += MS(10_000);
        assert_eq!(next(&mut r, &ports, now).as_deref(), Some("kivori"));
        // Alone, a demoted port is tried again.
        assert_eq!(next(&mut r, &["silent"], now).as_deref(), Some("silent"));
    }

    #[test]
    fn failed_open_ahead_of_kivori_is_rotated_past() {
        let mut r = CandidateRotator::new();
        let ports = ["busy", "kivori"];
        let first = next(&mut r, &ports, MS(0)).unwrap();
        assert_eq!(first, "busy");
        r.record_failure(&first, MS(0));
        assert_eq!(next(&mut r, &ports, MS(1)).as_deref(), Some("kivori"));
        // The first failure consumed the 250 ms step; the next one backs off longer.
        assert_eq!(r.next_delay_ms("busy"), Some(500));
    }

    #[test]
    fn wrong_major_port_is_skipped_until_unplugged_and_replugged() {
        let mut r = CandidateRotator::new();
        let ports = ["old", "kivori"];
        let first = next(&mut r, &ports, MS(0)).unwrap();
        assert_eq!(first, "old");
        r.mark_skipped(&first);
        assert!(r.any_skipped());
        for t in [1, 100_000] {
            assert_eq!(next(&mut r, &ports, MS(t)).as_deref(), Some("kivori"));
        }
        // Only the incompatible board plugged in: nothing eligible, no attempt.
        assert_eq!(next(&mut r, &["old"], MS(200_000)), None);
        assert!(r.any_skipped());
        // Unplug clears the skip, replug is a fresh start.
        assert_eq!(next(&mut r, &[], MS(200_001)), None);
        assert!(!r.any_skipped());
        assert_eq!(next(&mut r, &["old"], MS(200_002)).as_deref(), Some("old"));
    }

    #[test]
    fn vanished_ports_are_forgotten_with_their_backoff() {
        let mut r = CandidateRotator::new();
        let _ = next(&mut r, &["A"], MS(0));
        r.record_failure("A", MS(0));
        assert_eq!(next(&mut r, &["A"], MS(1)), None);
        assert_eq!(next(&mut r, &[], MS(2)), None);
        assert_eq!(r.earliest_retry_in(MS(2)), None);
        assert_eq!(next(&mut r, &["A"], MS(3)).as_deref(), Some("A"));
    }

    #[test]
    fn success_clears_failure_history() {
        let mut r = CandidateRotator::new();
        let _ = next(&mut r, &["A"], MS(0));
        r.record_handshake_timeout("A", MS(0));
        r.record_success("A");
        assert_eq!(next(&mut r, &["A"], MS(1)).as_deref(), Some("A"));
    }

    #[test]
    fn ports_outside_the_allowlist_are_never_picked() {
        let mut r = CandidateRotator::new();
        let other = PortCandidate::new("other", Some(0x1234), Some(0x5678));
        assert_eq!(r.next(&[other], DEFAULT_ALLOWLIST, MS(0)), None);
    }
}
