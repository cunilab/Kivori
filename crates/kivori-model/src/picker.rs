//! The profile picker and the hold ring: what the device shows while the knob switch is held and
//! turned, and while any switch is held towards its Hold.
//!
//! The wire structs ([`ProfileChoices`], [`HoldActions`]) are what the desktop tells the device;
//! the rest ([`PickerView`], [`HoldRing`]) is device-local render state, never on the wire. Like
//! every shown value, they come from what the desktop actually runs: the picker offers only the
//! profiles the desktop listed, and the ring appears only for a Hold the desktop said does
//! something (product invariants 2 and 3).

use serde::{Deserialize, Serialize};

use crate::desk::MediaText;

/// The most pinned profiles the picker lists after Auto.
pub const PICKER_PINNED_MAX: usize = 10;

/// Longest profile name the picker shows, in characters (what one 240 px line holds at the
/// picker's font). Shorter than a config name on purpose: the whole list rides one wire message
/// that must stay no bigger than `ControlLabelsUpdate`.
pub const PICKER_NAME_MAX: usize = 20;

/// A profile's display name as the picker shows it: Latin-1, at most [`PICKER_NAME_MAX`]
/// characters, a longer name cut with a trailing `~`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PickerName {
    bytes: [u8; PICKER_NAME_MAX],
    len: u8,
}

impl PickerName {
    /// No name (Auto).
    pub const EMPTY: PickerName = PickerName {
        bytes: [0; PICKER_NAME_MAX],
        len: 0,
    };

    /// `text` as the device can draw it (see [`MediaText::from_text`]), cut to the picker's width.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let full = MediaText::from_text(text);
        let src = full.as_latin1();
        let mut out = Self::EMPTY;
        let n = src.len().min(PICKER_NAME_MAX);
        out.bytes[..n].copy_from_slice(&src[..n]);
        if src.len() > PICKER_NAME_MAX {
            out.bytes[PICKER_NAME_MAX - 1] = b'~';
        }
        out.len = n as u8;
        out
    }

    /// The name as Latin-1 bytes (a malformed wire length is clamped, never trusted).
    #[must_use]
    pub fn as_latin1(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len).min(PICKER_NAME_MAX)]
    }

    /// Nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_latin1().is_empty()
    }
}

/// How long the picker's highlight takes to slide to the next entry.
pub const PICKER_SLIDE_MS: u32 = 160;

/// How long the picker waits for the desktop to confirm a pick before it stops claiming anything.
pub const PICKER_CONFIRM_TIMEOUT_MS: u32 = 1_500;

/// How long the confirmed profile stays up after the desktop confirmed it.
pub const PICKER_CONFIRMED_MS: u32 = 900;

/// How long "not confirmed" stays up when the desktop never answered.
pub const PICKER_UNCONFIRMED_MS: u32 = 1_200;

/// What the picker offers (capability `PROFILE_PICKER_V1`): Auto, then the pinned profiles in
/// order. Sent whenever the list or the desktop's pin changes; session scoped like `Status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProfileChoices {
    /// Changes whenever the list of names changes (wraps). A [`ProfilePick`] names the generation
    /// it was made from, so a pick from an older list is never applied to a newer one.
    pub generation: u8,
    /// How many of `names` are in use, `0..=PICKER_PINNED_MAX`.
    pub count: u8,
    /// What the desktop actually runs right now: 0 is Auto (following the app in front), `n` is
    /// `names[n - 1]` pinned. The device shows a pick as confirmed only when this says so.
    pub selected: u8,
    /// The pinned profiles' display names, in picker order.
    pub names: [PickerName; PICKER_PINNED_MAX],
}

impl ProfileChoices {
    /// Nothing sent yet: Auto alone. A constant, so the device can borrow it instead of copying
    /// 270 bytes onto the stack.
    pub const NONE: ProfileChoices = ProfileChoices {
        generation: 0,
        count: 0,
        selected: 0,
        names: [PickerName::EMPTY; PICKER_PINNED_MAX],
    };

    /// Entries the picker steps through: Auto plus the pinned profiles (a malformed count is
    /// clamped, never trusted).
    #[must_use]
    pub fn entries(&self) -> u8 {
        1 + self.count.min(PICKER_PINNED_MAX as u8)
    }

    /// The name of entry `index` (0 is Auto and has none).
    #[must_use]
    pub fn name(&self, index: u8) -> PickerName {
        match usize::from(index).checked_sub(1) {
            Some(i) if i < usize::from(self.count).min(PICKER_PINNED_MAX) => self.names[i],
            _ => PickerName::EMPTY,
        }
    }
}

/// Device -> desktop (capability `PROFILE_PICKER_V1`): the knob switch was released on this entry.
/// The desktop pins the profile (or returns to Auto) and answers with `ProfileChoices` whose
/// `selected` is what it now runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilePick {
    /// The `ProfileChoices::generation` the pick was made from.
    pub generation: u8,
    /// The entry picked: 0 is Auto, `n` is the `n`-th pinned profile.
    pub index: u8,
}

/// Which contextual buttons have a Hold, and what it does, as short labels the desktop derives
/// from its active bindings (capability `HOLD_RING_V1`). An empty label means the Hold does
/// nothing (or is suspended): the device draws no ring for it, never a guessed one. The knob
/// switch's Hold label already travels in `ControlLabels::hold`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HoldActions {
    /// The three contextual buttons, left to right.
    pub buttons: [MediaText; 3],
}

/// The hold ring to draw: a switch is held towards its Hold. Device-local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldRing {
    /// Which switch: 0 is the knob's push switch, 1 to 3 the contextual buttons left to right.
    pub control: u8,
    /// How far the hold has come towards the Hold threshold, `0..=100`. At 100 a release is a Hold.
    pub percent: u8,
    /// What the Hold does, in the active profile's words.
    pub label: MediaText,
}

impl HoldRing {
    /// The hold is long enough: releasing now runs the Hold.
    #[must_use]
    pub const fn ready(&self) -> bool {
        self.percent >= 100
    }
}

/// Where the picker is in its short life. Device-local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerPhase {
    /// The knob switch is held and the highlight follows the knob.
    #[default]
    Choosing,
    /// Released: the pick went to the desktop and nothing is confirmed yet.
    Waiting,
    /// The desktop reports it now runs the pick.
    Confirmed,
    /// The desktop did not confirm in time. Nothing is claimed; the real state returns.
    Unconfirmed,
}

/// What the picker draws. Device-local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickerView {
    /// Entries, Auto included: `1..=PICKER_PINNED_MAX + 1`.
    pub count: u8,
    /// The highlighted entry (0 is Auto).
    pub index: u8,
    /// The entry the highlight is sliding away from.
    pub from: u8,
    /// Milliseconds since the highlight last moved (drives the slide).
    pub moved_ms: u32,
    /// The highlighted profile's name; empty for Auto.
    pub name: PickerName,
    /// Where the pick is.
    pub phase: PickerPhase,
    /// Milliseconds since `phase` began (drives the waiting pulse and the confirmation).
    pub phase_ms: u32,
}

impl PickerView {
    /// How far the highlight has travelled from `from` to `index`, `0..=4096` (ease-out cubic over
    /// [`PICKER_SLIDE_MS`]).
    #[must_use]
    pub fn slide_q12(&self) -> i32 {
        if self.moved_ms >= PICKER_SLIDE_MS || self.from == self.index {
            return 4096;
        }
        let t = (i64::from(self.moved_ms) * 4096 / i64::from(PICKER_SLIDE_MS)) as i32;
        let u = 4096 - t;
        4096 - u * u / 4096 * u / 4096
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_clamp_a_malformed_count_and_auto_has_no_name() {
        let mut c = ProfileChoices {
            count: 200,
            ..ProfileChoices::default()
        };
        assert_eq!(usize::from(c.entries()), 1 + PICKER_PINNED_MAX);
        assert!(c.name(0).is_empty());
        c.count = 2;
        c.names[0] = PickerName::from_text("Code");
        c.names[1] = PickerName::from_text("Media");
        assert_eq!(c.entries(), 3);
        assert_eq!(c.name(2).as_latin1(), b"Media");
        assert!(c.name(3).is_empty(), "past the count is empty");
    }

    #[test]
    fn picker_names_are_cut_to_the_line_with_a_marker() {
        assert_eq!(
            PickerName::from_text("Claude Code").as_latin1(),
            b"Claude Code"
        );
        let long = PickerName::from_text("Adobe Premiere Pro Ultimate Edition");
        assert_eq!(long.as_latin1().len(), PICKER_NAME_MAX);
        assert_eq!(long.as_latin1()[PICKER_NAME_MAX - 1], b'~');
        assert!(PickerName::from_text("  ").is_empty());
    }

    #[test]
    fn the_whole_list_fits_the_biggest_existing_message() {
        // ControlLabelsUpdate is 7 texts of 33 bytes plus a session: the choices must not grow
        // `Message` (the dispatcher's stack frame budget depends on it).
        assert!(core::mem::size_of::<ProfileChoices>() + 4 <= 7 * 33 + 8);
    }

    #[test]
    fn the_slide_eases_out_and_lands_exactly() {
        let mut v = PickerView {
            count: 3,
            index: 2,
            from: 1,
            moved_ms: 0,
            name: PickerName::EMPTY,
            phase: PickerPhase::Choosing,
            phase_ms: 0,
        };
        assert_eq!(v.slide_q12(), 0);
        v.moved_ms = PICKER_SLIDE_MS / 2;
        let mid = v.slide_q12();
        assert!(mid > 2048 && mid < 4096, "ease-out is past halfway: {mid}");
        v.moved_ms = PICKER_SLIDE_MS;
        assert_eq!(v.slide_q12(), 4096);
    }
}
