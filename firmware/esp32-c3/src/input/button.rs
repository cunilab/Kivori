//! Push-switch gesture formation: debounce, release-qualified Press, Hold, and the recovery hold
//! (docs/product.md, input and gestures).
//!
//! - Short press: released within [`PRESS_MAX_MS`] of key-down. Fires once, on release.
//! - Hold: released at or after [`PRESS_MAX_MS`] and before [`RECOVERY_START_MS`]. Fires once,
//!   on release.
//! - Recovery: at [`RECOVERY_START_MS`] recovery owns the gesture and any Hold is cancelled; at
//!   [`REBOOT_MS`] from key-down the MCU reboots. Releasing before that cancels recovery and
//!   fires nothing. This needs no host, no session and no capability (invariant 24), and nothing
//!   but the switch can cancel it (invariant 33).
//!
//! All times are measured between debounced edges at the moment the switch first changed, not at
//! the moment the loop noticed, so a long render pass cannot turn a short press into a Hold.
//! Not configurable in v1.

/// The switch level must stay put this long before an edge counts. Mechanical contact bounce is
/// a few milliseconds; this keeps a margin without delaying a press noticeably.
pub const DEBOUNCE_MS: u32 = 20;
/// A press released before this many ms after key-down is a short Press.
pub const PRESS_MAX_MS: u32 = 500;
/// At this many ms after key-down the recovery hold takes the gesture.
pub const RECOVERY_START_MS: u32 = 2_000;
/// At this many ms after key-down the MCU reboots.
pub const REBOOT_MS: u32 = 10_000;
/// With double press enabled, a second key-down within this many ms of a short press's release
/// makes the pair a `DoublePress`; otherwise the first press fires as `Press` when it lapses.
pub const DOUBLE_WINDOW_MS: u32 = 250;

/// What a debounced switch edge or the passage of time produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEvent {
    /// The switch went down: show the local acknowledgement now (not a confirmation).
    Down,
    /// Released within the short-press window (and, with double press enabled, no second press
    /// followed inside [`DOUBLE_WINDOW_MS`]).
    Press,
    /// Two short presses in a row (double press enabled only). Replaces both `Press` events.
    DoublePress,
    /// Released inside the Hold window.
    Hold,
    /// Released, firing nothing (ineligible press, or recovery cancelled).
    Released,
    /// Recovery took the gesture; any Hold is cancelled.
    RecoveryStarted,
    /// The recovery hold completed: reboot now.
    Reboot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Up,
    Down {
        since_ms: u32,
        /// `false` when another gesture owned input at key-down: the press may still become a
        /// recovery hold, but never a Press or Hold (one gesture owns all input at a time).
        eligible: bool,
        recovery: bool,
        /// This press began inside the double-press window of a short press: if it is short
        /// too, the pair is a `DoublePress`.
        second: bool,
    },
}

/// Debounces the switch level and forms Press / Hold / recovery gestures.
#[derive(Debug)]
pub struct ButtonGesture {
    /// The last raw level seen and when it first appeared.
    raw: bool,
    raw_since_ms: u32,
    /// Whether another gesture owned input when `raw` last went down.
    raw_other_open: bool,
    /// The debounced level.
    stable: bool,
    phase: Phase,
    rebooted: bool,
    /// Double-press detection is on (negotiated with the host).
    double_press: bool,
    /// Release time of a short press still waiting to see whether a second press follows.
    pending_press_ms: Option<u32>,
}

impl ButtonGesture {
    /// A released switch.
    pub const fn new() -> Self {
        Self {
            raw: false,
            raw_since_ms: 0,
            raw_other_open: false,
            stable: false,
            phase: Phase::Up,
            rebooted: false,
            double_press: false,
            pending_press_ms: None,
        }
    }

    /// Turns double-press detection on or off. Off, a short press fires on release with no wait.
    /// Turning it off releases a waiting press at once (as `Press` on the next poll).
    pub fn set_double_press(&mut self, enabled: bool) {
        self.double_press = enabled;
    }

    /// The debounced switch is down.
    #[must_use]
    pub const fn is_down(&self) -> bool {
        matches!(self.phase, Phase::Down { .. })
    }

    /// Recovery-hold progress 0..=100 at `now_ms`, while recovery owns the gesture.
    #[must_use]
    pub fn recovery_percent(&self, now_ms: u32) -> Option<u8> {
        let Phase::Down {
            since_ms,
            recovery: true,
            ..
        } = self.phase
        else {
            return None;
        };
        let into = now_ms
            .wrapping_sub(since_ms)
            .saturating_sub(RECOVERY_START_MS);
        let span = REBOOT_MS - RECOVERY_START_MS;
        Some((into.min(span) * 100 / span) as u8)
    }

    /// Feeds one raw level sample (`true` = pressed) captured at `at_ms`, then advances time to
    /// `at_ms`. `other_gesture_open` says whether another gesture owned input at that moment; it
    /// is only read at the raw key-down edge. Returns at most two events (an edge, then a time boundary).
    pub fn update(
        &mut self,
        pressed: bool,
        at_ms: u32,
        other_gesture_open: bool,
    ) -> [Option<ButtonEvent>; 2] {
        if pressed != self.raw {
            self.raw = pressed;
            self.raw_since_ms = at_ms;
            if pressed {
                self.raw_other_open = other_gesture_open;
            }
        }
        [self.settle(at_ms), self.poll(at_ms)]
    }

    /// Advances time with no new sample. Emits `RecoveryStarted` and `Reboot` on time, and a
    /// debounced edge whose level has now been stable long enough.
    pub fn poll(&mut self, now_ms: u32) -> Option<ButtonEvent> {
        if let Some(edge) = self.settle(now_ms) {
            return Some(edge);
        }
        if let Some(released_ms) = self.pending_press_ms {
            // A second key-down still debouncing inside the window keeps the first press waiting.
            let second_coming =
                self.raw && self.raw_since_ms.wrapping_sub(released_ms) < DOUBLE_WINDOW_MS;
            let lapsed = now_ms.wrapping_sub(released_ms) >= DOUBLE_WINDOW_MS;
            if !second_coming && (lapsed || !self.double_press) {
                self.pending_press_ms = None;
                return Some(ButtonEvent::Press);
            }
        }
        let Phase::Down {
            since_ms,
            eligible,
            recovery,
            second,
        } = self.phase
        else {
            return None;
        };
        // A release still inside its debounce window already ended the press at its edge: the
        // hold clock stops there, or a 1.99 s hold would turn into recovery while debouncing.
        let until_ms = if self.raw { now_ms } else { self.raw_since_ms };
        let held = until_ms.wrapping_sub(since_ms);
        if second && held >= PRESS_MAX_MS {
            // The second press is no longer short: no double press. The first one still fires
            // once, and this one goes on as a normal Hold / recovery.
            self.phase = Phase::Down {
                since_ms,
                eligible,
                recovery,
                second: false,
            };
            return Some(ButtonEvent::Press);
        }
        if !recovery && held >= RECOVERY_START_MS {
            self.phase = Phase::Down {
                since_ms,
                eligible,
                recovery: true,
                second,
            };
            return Some(ButtonEvent::RecoveryStarted);
        }
        if recovery && held >= REBOOT_MS && !self.rebooted {
            self.rebooted = true;
            return Some(ButtonEvent::Reboot);
        }
        None
    }

    /// Commits a raw level that has been stable for [`DEBOUNCE_MS`], timed at its first edge.
    fn settle(&mut self, now_ms: u32) -> Option<ButtonEvent> {
        if self.raw == self.stable || now_ms.wrapping_sub(self.raw_since_ms) < DEBOUNCE_MS {
            return None;
        }
        self.stable = self.raw;
        let edge_ms = self.raw_since_ms;
        if self.stable {
            let eligible = !self.raw_other_open;
            let waiting = self.pending_press_ms.take();
            let second = eligible
                && waiting
                    .is_some_and(|released| edge_ms.wrapping_sub(released) < DOUBLE_WINDOW_MS);
            self.phase = Phase::Down {
                since_ms: edge_ms,
                eligible,
                recovery: false,
                second,
            };
            // A waiting first press that is not part of a pair fires now, exactly once.
            return Some(if waiting.is_some() && !second {
                ButtonEvent::Press
            } else {
                ButtonEvent::Down
            });
        }
        self.rebooted = false;
        let Phase::Down {
            since_ms,
            eligible,
            recovery,
            second,
        } = core::mem::replace(&mut self.phase, Phase::Up)
        else {
            return None;
        };
        let held = edge_ms.wrapping_sub(since_ms);
        Some(if recovery || !eligible || held >= RECOVERY_START_MS {
            ButtonEvent::Released
        } else if held < PRESS_MAX_MS {
            if second {
                ButtonEvent::DoublePress
            } else if self.double_press {
                // Wait to see whether a second press follows; `poll` fires it otherwise.
                self.pending_press_ms = Some(edge_ms);
                ButtonEvent::Released
            } else {
                ButtonEvent::Press
            }
        } else {
            ButtonEvent::Hold
        })
    }

    /// Forgets any press in progress, as if the switch were released with no event. Used when a
    /// host session ends, so a half-done Press cannot fire into the next session. Recovery is not
    /// session-scoped and is never reset here: callers only reset when no recovery is running.
    pub fn reset(&mut self) {
        // A key-down still debouncing at the boundary is ineligible too; the next raw key-down
        // edge sets this afresh.
        self.raw_other_open = true;
        self.pending_press_ms = None;
        if let Phase::Down {
            since_ms,
            recovery: false,
            ..
        } = self.phase
        {
            // Keep the real key-down time: recovery stays 2 s / 10 s from key-down.
            self.phase = Phase::Down {
                since_ms,
                eligible: false,
                recovery: false,
                second: false,
            };
        }
    }
}

impl Default for ButtonGesture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Presses at `down`, releases at `up`, polling every 10 ms in between and after; collects
    /// every event.
    fn press(down: u32, up: u32, other_gesture_open: bool) -> heapless::Vec<ButtonEvent, 16> {
        let mut b = ButtonGesture::new();
        let mut events = heapless::Vec::new();
        let mut push = |e: [Option<ButtonEvent>; 2]| {
            for e in e.into_iter().flatten() {
                events.push(e).unwrap();
            }
        };
        push(b.update(true, down, other_gesture_open));
        let mut t = down;
        while t + 10 < up {
            t += 10;
            push([b.poll(t), None]);
        }
        push(b.update(false, up, false));
        for dt in (10..=100).step_by(10) {
            push([b.poll(up + dt), None]);
        }
        events
    }

    #[test]
    fn a_release_inside_500_ms_is_one_press() {
        assert_eq!(
            press(1_000, 1_499, false).as_slice(),
            [ButtonEvent::Down, ButtonEvent::Press]
        );
    }

    #[test]
    fn a_release_at_500_ms_or_later_is_a_hold() {
        assert_eq!(
            press(1_000, 1_500, false).as_slice(),
            [ButtonEvent::Down, ButtonEvent::Hold]
        );
        assert_eq!(
            press(1_000, 2_999, false).as_slice(),
            [ButtonEvent::Down, ButtonEvent::Hold]
        );
    }

    #[test]
    fn recovery_takes_over_at_two_seconds_and_cancels_the_hold() {
        assert_eq!(
            press(1_000, 5_000, false).as_slice(),
            [
                ButtonEvent::Down,
                ButtonEvent::RecoveryStarted,
                ButtonEvent::Released
            ]
        );
    }

    #[test]
    fn ten_seconds_from_key_down_reboots_once() {
        let events = press(1_000, 12_000, false);
        assert_eq!(
            events.as_slice(),
            [
                ButtonEvent::Down,
                ButtonEvent::RecoveryStarted,
                ButtonEvent::Reboot,
                ButtonEvent::Released
            ]
        );
    }

    #[test]
    fn a_press_that_starts_during_another_gesture_fires_nothing_but_can_still_recover() {
        assert_eq!(
            press(1_000, 1_200, true).as_slice(),
            [ButtonEvent::Down, ButtonEvent::Released]
        );
        assert!(press(1_000, 12_000, true).contains(&ButtonEvent::Reboot));
    }

    #[test]
    fn bounce_shorter_than_the_debounce_window_is_ignored() {
        let mut b = ButtonGesture::new();
        assert_eq!(b.update(true, 100, false), [None, None]);
        assert_eq!(b.update(false, 105, false), [None, None]);
        assert_eq!(b.update(true, 108, false), [None, None]);
        assert_eq!(b.poll(127), None, "only 19 ms stable");
        assert_eq!(b.poll(128), Some(ButtonEvent::Down));
        // Release bounce, then a clean release: timed from the first edge of the final level.
        assert_eq!(b.update(false, 300, false), [None, None]);
        assert_eq!(b.update(true, 302, false), [None, None]);
        assert_eq!(b.update(false, 305, false), [None, None]);
        assert_eq!(b.poll(325), Some(ButtonEvent::Press));
        assert!(!b.is_down());
    }

    #[test]
    fn edges_are_timed_when_they_happened_not_when_the_loop_noticed() {
        // Down at 0, up at 450 (a short press), but the loop only looks again at 900.
        let mut b = ButtonGesture::new();
        let _ = b.update(true, 0, false);
        let _ = b.update(true, 40, false);
        assert_eq!(b.update(false, 450, false), [None, None]);
        assert_eq!(b.poll(900), Some(ButtonEvent::Press));
    }

    #[test]
    fn recovery_progress_runs_from_two_to_ten_seconds() {
        let mut b = ButtonGesture::new();
        let _ = b.update(true, 0, false);
        let _ = b.poll(30);
        assert_eq!(b.recovery_percent(1_000), None);
        assert_eq!(b.poll(2_000), Some(ButtonEvent::RecoveryStarted));
        assert_eq!(b.recovery_percent(2_000), Some(0));
        assert_eq!(b.recovery_percent(6_000), Some(50));
        assert_eq!(b.recovery_percent(10_000), Some(100));
        assert_eq!(b.recovery_percent(60_000), Some(100));
    }

    #[test]
    fn reset_drops_a_pending_press_but_never_a_running_recovery() {
        let mut b = ButtonGesture::new();
        let _ = b.update(true, 0, false);
        let _ = b.poll(30);
        b.reset();
        let _ = b.update(false, 200, false);
        assert_eq!(
            b.poll(230),
            Some(ButtonEvent::Released),
            "no Press into a new session"
        );

        let mut b = ButtonGesture::new();
        let _ = b.update(true, 0, false);
        let _ = b.poll(30);
        let _ = b.poll(2_000);
        b.reset();
        assert_eq!(b.poll(10_000), Some(ButtonEvent::Reboot));
    }

    #[test]
    fn a_key_down_still_debouncing_at_a_session_boundary_fires_nothing() {
        let mut b = ButtonGesture::new();
        let _ = b.update(true, 1_000, false);
        b.reset();
        let _ = b.poll(1_030);
        let _ = b.update(false, 1_200, false);
        assert_eq!(b.poll(1_230), Some(ButtonEvent::Released));
    }

    #[test]
    fn reset_keeps_the_key_down_time_for_recovery() {
        let mut b = ButtonGesture::new();
        let _ = b.update(true, 0, false);
        let _ = b.poll(30);
        // A contact glitch, then a session boundary.
        let _ = b.update(false, 1_000, false);
        let _ = b.update(true, 1_005, false);
        b.reset();
        assert_eq!(b.poll(2_000), Some(ButtonEvent::RecoveryStarted));
    }

    /// Drives a script of (pressed, at_ms) edges with double press on, polling every 10 ms.
    fn script_double(edges: &[(bool, u32)], until: u32) -> heapless::Vec<ButtonEvent, 16> {
        let mut b = ButtonGesture::new();
        b.set_double_press(true);
        let mut events = heapless::Vec::new();
        let mut t = 0;
        let mut next = 0;
        while t <= until {
            let mut out = [b.poll(t), None];
            if next < edges.len() && edges[next].1 == t {
                out = b.update(edges[next].0, t, false);
                next += 1;
            }
            for e in out.into_iter().flatten() {
                if e != ButtonEvent::Down && e != ButtonEvent::Released {
                    events.push(e).unwrap();
                }
            }
            t += 10;
        }
        events
    }

    #[test]
    fn two_quick_short_presses_are_one_double_press() {
        let events = script_double(
            &[(true, 100), (false, 200), (true, 350), (false, 450)],
            1_500,
        );
        assert_eq!(events.as_slice(), [ButtonEvent::DoublePress]);
    }

    #[test]
    fn a_single_press_waits_out_the_window_then_fires_once() {
        let mut b = ButtonGesture::new();
        b.set_double_press(true);
        let _ = b.update(true, 0, false);
        let _ = b.poll(30);
        let _ = b.update(false, 100, false);
        assert_eq!(b.poll(130), Some(ButtonEvent::Released));
        assert_eq!(
            b.poll(349),
            None,
            "still inside the window from release at 100"
        );
        assert_eq!(b.poll(350), Some(ButtonEvent::Press));
        assert_eq!(b.poll(900), None);
    }

    #[test]
    fn a_second_press_after_the_window_is_two_presses() {
        let events = script_double(
            &[(true, 100), (false, 200), (true, 500), (false, 600)],
            1_500,
        );
        assert_eq!(events.as_slice(), [ButtonEvent::Press, ButtonEvent::Press]);
    }

    #[test]
    fn a_long_second_press_fires_the_first_press_then_a_hold() {
        let events = script_double(
            &[(true, 100), (false, 200), (true, 350), (false, 1_350)],
            2_000,
        );
        assert_eq!(events.as_slice(), [ButtonEvent::Press, ButtonEvent::Hold]);
    }

    #[test]
    fn a_press_then_a_recovery_hold_still_recovers() {
        let events = script_double(&[(true, 100), (false, 200), (true, 350)], 10_500);
        assert_eq!(
            events.as_slice(),
            [
                ButtonEvent::Press,
                ButtonEvent::RecoveryStarted,
                ButtonEvent::Reboot
            ]
        );
    }

    #[test]
    fn a_session_boundary_drops_a_waiting_press() {
        let mut b = ButtonGesture::new();
        b.set_double_press(true);
        let _ = b.update(true, 0, false);
        let _ = b.poll(30);
        let _ = b.update(false, 100, false);
        let _ = b.poll(130);
        b.reset();
        assert_eq!(b.poll(400), None);
    }

    #[test]
    fn timing_survives_a_device_uptime_wrap() {
        let start = u32::MAX - 100;
        let mut b = ButtonGesture::new();
        let _ = b.update(true, start, false);
        let _ = b.poll(start.wrapping_add(30));
        let _ = b.update(false, start.wrapping_add(300), false);
        assert_eq!(b.poll(start.wrapping_add(330)), Some(ButtonEvent::Press));
    }
}
