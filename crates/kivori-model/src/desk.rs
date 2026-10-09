//! M1 desk types: what the display shows besides the buddy, and how an action's outcome is known.
//!
//! Every enum nested in a wire message is **append-only**: the postcard variant index is part of
//! the wire encoding, so variants may only be added at the end, never reordered or removed.
//! `None` always means "not observed": the device renders it as unknown, never as a guess
//! (product invariant 2).

use serde::{Deserialize, Serialize};

/// Which full-screen view the device shows. Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DisplayMode {
    /// The mascot, with indicators and cues.
    #[default]
    Buddy,
    /// Local time of day.
    Clock,
    /// Master volume and mute.
    Volume,
    /// Current media playback state.
    Media,
    /// CPU and RAM load.
    System,
}

impl DisplayMode {
    /// Every mode, in display order.
    pub const ALL: [DisplayMode; 5] = [
        DisplayMode::Buddy,
        DisplayMode::Clock,
        DisplayMode::Volume,
        DisplayMode::Media,
        DisplayMode::System,
    ];
}

/// Media playback state as the OS reports it. Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaStatus {
    /// Something is playing.
    Playing,
    /// A session exists and is paused.
    Paused,
    /// No media session, or it stopped.
    Stopped,
}

/// Local wall-clock time of day, as the desktop observed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockTime {
    /// 0..=23.
    pub hour: u8,
    /// 0..=59.
    pub minute: u8,
    /// 0..=59.
    pub second: u8,
}

impl ClockTime {
    /// Seconds in a day.
    const DAY_S: u32 = 24 * 60 * 60;

    /// This time advanced by `elapsed_ms`, wrapping at midnight. The device uses it to keep the
    /// clock running between desktop updates; the desktop re-sends the real time regularly, so
    /// drift never accumulates past one update interval. Out-of-range fields are clamped first.
    #[must_use]
    pub const fn advanced_by(self, elapsed_ms: u32) -> ClockTime {
        let hour = if self.hour > 23 { 23 } else { self.hour } as u32;
        let minute = if self.minute > 59 { 59 } else { self.minute } as u32;
        let second = if self.second > 59 { 59 } else { self.second } as u32;
        let start = hour * 3600 + minute * 60 + second;
        let now = (start + (elapsed_ms / 1000) % Self::DAY_S) % Self::DAY_S;
        ClockTime {
            hour: (now / 3600) as u8,
            minute: (now / 60 % 60) as u8,
            second: (now % 60) as u8,
        }
    }
}

/// The desktop state the device displays. Sent whole on every change (`Message::Status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeskStatus {
    /// The view the user selected.
    pub mode: DisplayMode,
    /// Local time when the desktop sent this status.
    pub clock: Option<ClockTime>,
    /// Master volume, 0..=100, as the OS reports it.
    pub volume_percent: Option<u8>,
    /// Master mute, as the OS reports it.
    pub muted: Option<bool>,
    /// Media playback, as the OS reports it.
    pub media: Option<MediaStatus>,
    /// Whole-machine CPU load, 0..=100.
    pub cpu_percent: Option<u8>,
    /// Physical memory in use, 0..=100.
    pub ram_percent: Option<u8>,
    /// Sustained high CPU load, decided by the desktop with hysteresis. Drives the buddy's load
    /// cue, which must stay distinct from the Busy face.
    pub high_load: bool,
}

impl DeskStatus {
    /// Nothing observed yet: the Buddy view with every value unknown.
    pub const UNKNOWN: DeskStatus = DeskStatus {
        mode: DisplayMode::Buddy,
        clock: None,
        volume_percent: None,
        muted: None,
        media: None,
        cpu_percent: None,
        ram_percent: None,
        high_load: false,
    };
}

impl Default for DeskStatus {
    fn default() -> Self {
        Self::UNKNOWN
    }
}

/// The action a feedback refers to. Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    /// Master volume (rotary).
    Volume,
    /// Media play/pause.
    PlayPause,
    /// Master mute toggle.
    Mute,
    /// A keyboard shortcut.
    Shortcut,
    /// Launching an application.
    Launch,
    /// Media previous track.
    PreviousTrack,
    /// Media next track.
    NextTrack,
}

/// How well an action's outcome is known (docs/product.md, actions and confirmation).
/// Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackKind {
    /// Still running after ~500 ms; the outcome is not known yet.
    Processing,
    /// The resulting state was observed.
    StateConfirmed,
    /// The operation is known to have started or finished, with no lasting state to observe.
    ExecutionConfirmed,
    /// Dispatched, effect unknown. Never shown as success.
    Unverified,
    /// Known failure.
    Error,
}

impl FeedbackKind {
    /// How long the device shows this feedback before returning to the underlying view
    /// (docs/product.md transients). `Processing` is replaced by the outcome; its own expiry only
    /// guarantees it can never stick if the desktop disappears.
    #[must_use]
    pub const fn transient_ms(self) -> u32 {
        match self {
            FeedbackKind::StateConfirmed | FeedbackKind::ExecutionConfirmed => 800,
            FeedbackKind::Unverified => 1_200,
            FeedbackKind::Error | FeedbackKind::Processing => 2_000,
        }
    }
}

/// One action outcome to show. The newest replaces any older one; there is no queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionFeedback {
    /// Which action.
    pub action: ActionKind,
    /// What is known about its outcome.
    pub kind: FeedbackKind,
}

/// Up to [`MediaText::CAPACITY`] characters of display text in ISO-8859-1 (one byte per char),
/// which the device's Latin-1 bitmap font can draw. Built only through [`MediaText::from_text`],
/// so it never holds control bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MediaText {
    bytes: [u8; 32],
    len: u8,
}

impl MediaText {
    /// Longest text kept, in characters.
    pub const CAPACITY: usize = 32;

    /// Converts `text` for the device: Latin-1 characters are kept, anything else becomes `?`,
    /// control characters and surrounding whitespace are dropped, runs of whitespace collapse to
    /// one space, and the result is cut to [`Self::CAPACITY`] characters (a cut ends in `~`).
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut out = Self::default();
        let mut pending_space = false;
        for c in text.trim().chars() {
            if c.is_whitespace() {
                pending_space = out.len > 0;
                continue;
            }
            if c.is_control() {
                continue;
            }
            let byte = u8::try_from(u32::from(c)).unwrap_or(b'?');
            let needed = 1 + usize::from(pending_space);
            if usize::from(out.len) + needed > Self::CAPACITY {
                out.bytes[Self::CAPACITY - 1] = b'~';
                out.len = Self::CAPACITY as u8;
                return out;
            }
            if pending_space {
                out.push(b' ');
                pending_space = false;
            }
            out.push(byte);
        }
        out
    }

    fn push(&mut self, byte: u8) {
        self.bytes[usize::from(self.len)] = byte;
        self.len += 1;
    }

    /// The text as Latin-1 bytes (a malformed wire length is clamped, never trusted).
    #[must_use]
    pub fn as_latin1(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len).min(Self::CAPACITY)]
    }

    /// Nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_latin1().is_empty()
    }
}

/// What is playing, as the OS reports it. Either part may be empty when the player gives none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Track title.
    pub title: MediaText,
    /// Artist (or channel / show).
    pub artist: MediaText,
}

/// What kind of context the active profile is, for the buddy's resting face. Only what Desktop
/// knows from the profile it actually runs. Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ContextMood {
    /// No special context.
    #[default]
    Neutral,
    /// A meeting app's profile is active.
    Meeting,
}

/// What the physical controls do right now, as short labels the desktop derives from its active
/// bindings (capability `CONTROL_LABELS_V1`). The device only draws them; it never guesses a
/// binding, so an empty label (or no labels at all) shows nothing for that control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ControlLabels {
    /// The knob.
    pub rotate: MediaText,
    /// A short press of the push switch.
    pub press: MediaText,
    /// A hold of the push switch.
    pub hold: MediaText,
    /// The three contextual buttons, left to right (M2 hardware; empty until they exist).
    pub buttons: [MediaText; 3],
    /// The active profile's display name; empty draws nothing (the General fallback).
    pub profile: MediaText,
    /// The profile was pinned from the device rather than following the foreground app.
    pub pinned: bool,
    /// The active profile's context, for the buddy's resting face.
    pub mood: ContextMood,
}

/// The last CPU samples the device received (one per `Status`), oldest first, for a sparkline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuHistory {
    samples: [u8; CpuHistory::CAPACITY],
    len: u8,
}

impl CpuHistory {
    /// Samples kept (about a minute at one status per second).
    pub const CAPACITY: usize = 60;

    /// No samples yet.
    pub const EMPTY: CpuHistory = CpuHistory {
        samples: [0; CpuHistory::CAPACITY],
        len: 0,
    };

    /// Appends one sample (clamped to 100), dropping the oldest when full.
    pub fn push(&mut self, percent: u8) {
        let percent = percent.min(100);
        if usize::from(self.len) == Self::CAPACITY {
            self.samples.copy_within(1.., 0);
            self.samples[Self::CAPACITY - 1] = percent;
        } else {
            self.samples[usize::from(self.len)] = percent;
            self.len += 1;
        }
    }

    /// The samples, oldest first.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.samples[..usize::from(self.len)]
    }
}

impl Default for CpuHistory {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// How long switching views animates. `DeskView::previous_mode` is set only within this window.
pub const VIEW_TRANSITION_MS: u32 = 320;

/// Everything the desk renderer needs for one frame besides the mascot pose. Device-local; not on
/// the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeskView {
    /// The last desktop status, with `clock` already advanced to this frame.
    pub status: DeskStatus,
    /// The feedback still in force, if any.
    pub feedback: Option<ActionFeedback>,
    /// Any switch (knob or contextual button) is held down: the device's own immediate
    /// acknowledgement, shown before the desktop knows anything (acknowledgement is not
    /// confirmation, invariant 3).
    pub button_down: bool,
    /// The knob's own push switch is held down (its Press / Hold labels show meanwhile).
    pub switch_down: bool,
    /// Recovery-hold progress, 0..=100, while the recovery takeover owns the screen.
    pub recovery_percent: Option<u8>,
    /// The host is flashing new firmware: the Updating takeover owns the screen (below recovery).
    pub updating: bool,
    /// Device time for subtle animation only (never shown as a value).
    pub elapsed_ms: u32,
    /// What is playing, when the desktop could observe it (`None` = unknown).
    pub media_info: Option<MediaInfo>,
    /// Recent CPU samples for a sparkline; empty when none arrived this session.
    pub cpu_history: CpuHistory,
    /// The view shown before the current one, while the switch animation runs.
    pub previous_mode: Option<DisplayMode>,
    /// Milliseconds since the current view appeared (drives the switch animation).
    pub mode_age_ms: u32,
    /// What the controls do, when the desktop sent it this session (`None` = not known).
    pub controls: Option<ControlLabels>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn t(hour: u8, minute: u8, second: u8) -> ClockTime {
        ClockTime {
            hour,
            minute,
            second,
        }
    }

    #[test]
    fn the_clock_advances_and_wraps_at_midnight() {
        assert_eq!(t(9, 59, 59).advanced_by(1_000), t(10, 0, 0));
        assert_eq!(t(23, 59, 30).advanced_by(45_000), t(0, 0, 15));
        assert_eq!(t(12, 0, 0).advanced_by(999), t(12, 0, 0));
        assert_eq!(t(0, 0, 0).advanced_by(u32::MAX), t(17, 2, 47));
    }

    #[test]
    fn out_of_range_fields_are_clamped_not_overflowed() {
        assert_eq!(t(99, 99, 99).advanced_by(0), t(23, 59, 59));
    }

    #[test]
    fn media_text_keeps_latin1_replaces_the_rest_and_cuts_with_a_marker() {
        assert_eq!(
            MediaText::from_text("  Café  del\tMar ").as_latin1(),
            b"Caf\xe9 del Mar"
        );
        assert_eq!(MediaText::from_text("夜に駆ける").as_latin1(), b"?????");
        assert_eq!(MediaText::from_text("a\u{7}b").as_latin1(), b"ab");
        let long = MediaText::from_text(&"x".repeat(40));
        assert_eq!(long.as_latin1().len(), MediaText::CAPACITY);
        assert_eq!(long.as_latin1()[MediaText::CAPACITY - 1], b'~');
        assert!(MediaText::from_text("   ").is_empty());
    }

    #[test]
    fn cpu_history_keeps_the_latest_samples_oldest_first() {
        let mut h = CpuHistory::EMPTY;
        for n in 0..65u8 {
            h.push(n);
        }
        assert_eq!(h.as_slice().len(), CpuHistory::CAPACITY);
        assert_eq!(h.as_slice()[0], 5);
        assert_eq!(*h.as_slice().last().unwrap(), 64);
        h.push(250);
        assert_eq!(*h.as_slice().last().unwrap(), 100);
    }

    #[test]
    fn unverified_and_error_outlast_confirmed_feedback() {
        assert!(
            FeedbackKind::Unverified.transient_ms() > FeedbackKind::StateConfirmed.transient_ms()
        );
        assert!(FeedbackKind::Error.transient_ms() > FeedbackKind::Unverified.transient_ms());
    }
}
