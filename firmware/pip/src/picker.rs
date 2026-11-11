//! The profile picker's device-side state (capability `PROFILE_PICKER_V1`): hold the knob switch and
//! turn to step through Auto and the pinned profiles, release to send the pick.
//!
//! Everything here is the device's own immediate response. The pick is only *sent*; the picker
//! claims it succeeded only when the desktop's next `ProfileChoices` says it now runs that entry
//! (acknowledgement is not confirmation, product invariant 3), and when no answer comes it says
//! "not confirmed" and lets the real state return.

use kivori_model::input::Direction;
use kivori_model::picker::{
    PickerName, PickerPhase, PickerView, ProfileChoices, PICKER_CONFIRMED_MS,
    PICKER_CONFIRM_TIMEOUT_MS, PICKER_UNCONFIRMED_MS,
};

#[derive(Debug, Clone, Copy)]
struct Active {
    /// The `ProfileChoices::generation` the entries were read from.
    generation: u8,
    /// Auto plus the pinned profiles.
    entries: u8,
    index: u8,
    from: u8,
    moved_at: u32,
    phase: PickerPhase,
    phase_at: u32,
}

/// The picker: closed, or open on one entry.
#[derive(Debug, Default)]
pub struct Picker {
    active: Option<Active>,
    held_detents: u8,
}

impl Picker {
    /// A closed picker.
    pub const fn new() -> Self {
        Self {
            active: None,
            held_detents: 0,
        }
    }

    /// The picker owns the screen (choosing, or showing its outcome).
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.active.is_some()
    }

    /// The knob switch is held and the highlight follows the knob.
    #[must_use]
    pub fn is_choosing(&self) -> bool {
        matches!(
            self.active,
            Some(Active {
                phase: PickerPhase::Choosing,
                ..
            })
        )
    }

    /// One detent with the switch held. The second one opens the picker on the pin the desktop
    /// reported, then moves the highlight one step. Further detents step and stop at the ends.
    pub fn turn(&mut self, direction: Direction, choices: &ProfileChoices, now: u32) {
        if !self.is_choosing() {
            self.held_detents = self.held_detents.saturating_add(1);
            if self.held_detents < 2 {
                return;
            }
        }
        let mut a = match self.active {
            Some(a) if a.phase == PickerPhase::Choosing => a,
            _ => Active {
                generation: choices.generation,
                entries: choices.entries(),
                index: choices.selected.min(choices.entries() - 1),
                from: choices.selected.min(choices.entries() - 1),
                moved_at: now,
                phase: PickerPhase::Choosing,
                phase_at: now,
            },
        };
        let next = match direction {
            Direction::Cw => (a.index + 1).min(a.entries - 1),
            Direction::Ccw => a.index.saturating_sub(1),
        };
        if next != a.index {
            a.from = a.index;
            a.index = next;
            a.moved_at = now;
        }
        self.active = Some(a);
    }

    /// The switch was released: the entry to send, as `(generation, index)`, once.
    pub fn release(&mut self, now: u32) -> Option<(u8, u8)> {
        self.held_detents = 0;
        let a = self
            .active
            .as_mut()
            .filter(|a| a.phase == PickerPhase::Choosing)?;
        a.phase = PickerPhase::Waiting;
        a.phase_at = now;
        Some((a.generation, a.index))
    }

    /// The pick could not be sent: nothing will confirm it.
    pub fn send_failed(&mut self, now: u32) {
        if let Some(a) = self.active.as_mut() {
            a.phase = PickerPhase::Unconfirmed;
            a.phase_at = now;
        }
    }

    /// The desktop sent its choices. While choosing, a changed list re-syncs the entries (the
    /// names on screen are always the desktop's). While waiting, a list that is the one picked from
    /// and says the desktop runs the pick confirms it; anything else confirms nothing.
    pub fn on_choices(&mut self, choices: &ProfileChoices, now: u32) {
        let Some(a) = self.active.as_mut() else {
            return;
        };
        match a.phase {
            PickerPhase::Choosing => {
                a.generation = choices.generation;
                a.entries = choices.entries();
                a.index = a.index.min(a.entries - 1);
                a.from = a.from.min(a.entries - 1);
            }
            PickerPhase::Waiting
                if choices.generation == a.generation && choices.selected == a.index =>
            {
                a.phase = PickerPhase::Confirmed;
                a.phase_at = now;
            }
            _ => {}
        }
    }

    /// Advances time: an unanswered pick turns into "not confirmed", and an outcome that has been
    /// up long enough closes. Returns whether the picker changed what it shows.
    pub fn poll(&mut self, now: u32) -> bool {
        let Some(a) = self.active.as_mut() else {
            return false;
        };
        let age = now.wrapping_sub(a.phase_at);
        match a.phase {
            PickerPhase::Waiting if age >= PICKER_CONFIRM_TIMEOUT_MS => {
                a.phase = PickerPhase::Unconfirmed;
                a.phase_at = now;
                true
            }
            PickerPhase::Confirmed if age >= PICKER_CONFIRMED_MS => {
                self.active = None;
                true
            }
            PickerPhase::Unconfirmed if age >= PICKER_UNCONFIRMED_MS => {
                self.active = None;
                true
            }
            _ => false,
        }
    }

    /// Forgets everything: a session boundary, a takeover.
    pub fn close(&mut self) {
        self.held_detents = 0;
        self.active = None;
    }

    /// What to draw at `now`; names come from `choices` while their list is the one in force.
    #[must_use]
    pub fn view(&self, choices: &ProfileChoices, now: u32) -> Option<PickerView> {
        let a = self.active?;
        let name = if choices.generation == a.generation {
            choices.name(a.index)
        } else {
            PickerName::EMPTY
        };
        Some(PickerView {
            count: a.entries,
            index: a.index,
            from: a.from,
            moved_ms: now.wrapping_sub(a.moved_at),
            name,
            phase: a.phase,
            phase_ms: now.wrapping_sub(a.phase_at),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kivori_model::picker::PICKER_PINNED_MAX;

    fn choices(names: &[&str], selected: u8, generation: u8) -> ProfileChoices {
        let mut c = ProfileChoices {
            generation,
            count: names.len() as u8,
            selected,
            ..ProfileChoices::default()
        };
        for (i, n) in names.iter().enumerate().take(PICKER_PINNED_MAX) {
            c.names[i] = PickerName::from_text(n);
        }
        c
    }

    #[test]
    fn it_opens_on_the_desktops_pin_and_steps_without_wrapping() {
        let c = choices(&["Code", "Media"], 1, 4);
        let mut p = Picker::new();
        p.turn(Direction::Cw, &c, 0);
        assert!(!p.is_open(), "one held detent must not open the picker");
        p.turn(Direction::Cw, &c, 100);
        let v = p.view(&c, 100).unwrap();
        assert_eq!(
            (v.index, v.from, v.count),
            (2, 1, 3),
            "Code was pinned; one step on is Media"
        );
        assert_eq!(v.name.as_latin1(), b"Media");
        p.turn(Direction::Cw, &c, 120);
        assert_eq!(p.view(&c, 120).unwrap().index, 2, "stops at the last entry");
        for _ in 0..5 {
            p.turn(Direction::Ccw, &c, 140);
        }
        let v = p.view(&c, 140).unwrap();
        assert_eq!(v.index, 0, "Auto is the first stop");
        assert!(v.name.is_empty());
    }

    #[test]
    fn a_pick_is_confirmed_only_by_the_desktop_saying_it_runs_it() {
        let c = choices(&["Code", "Media"], 0, 4);
        let mut p = Picker::new();
        p.turn(Direction::Cw, &c, 0);
        p.turn(Direction::Cw, &c, 0);
        assert_eq!(p.release(500), Some((4, 1)));
        assert_eq!(p.release(510), None, "sent once");
        assert_eq!(p.view(&c, 600).unwrap().phase, PickerPhase::Waiting);
        // The desktop still runs Auto: nothing is confirmed.
        p.on_choices(&c, 700);
        assert_eq!(p.view(&c, 700).unwrap().phase, PickerPhase::Waiting);
        // A list from another generation confirms nothing either.
        p.on_choices(&choices(&["Code", "Media"], 1, 5), 800);
        assert_eq!(p.view(&c, 800).unwrap().phase, PickerPhase::Waiting);
        p.on_choices(&choices(&["Code", "Media"], 1, 4), 900);
        assert_eq!(p.view(&c, 900).unwrap().phase, PickerPhase::Confirmed);
        assert!(!p.poll(900 + PICKER_CONFIRMED_MS - 1));
        assert!(p.poll(900 + PICKER_CONFIRMED_MS));
        assert!(!p.is_open());
    }

    #[test]
    fn silence_becomes_not_confirmed_and_then_closes() {
        let c = choices(&["Code"], 0, 1);
        let mut p = Picker::new();
        p.turn(Direction::Cw, &c, 0);
        p.turn(Direction::Cw, &c, 0);
        p.release(100);
        assert!(!p.poll(100 + PICKER_CONFIRM_TIMEOUT_MS - 1));
        assert!(p.poll(100 + PICKER_CONFIRM_TIMEOUT_MS));
        assert_eq!(
            p.view(&c, 100 + PICKER_CONFIRM_TIMEOUT_MS).unwrap().phase,
            PickerPhase::Unconfirmed
        );
        // A late answer does not turn "not confirmed" into success.
        p.on_choices(&choices(&["Code"], 1, 1), 2_000);
        assert_ne!(p.view(&c, 2_000).unwrap().phase, PickerPhase::Confirmed);
        assert!(p.poll(100 + PICKER_CONFIRM_TIMEOUT_MS + PICKER_UNCONFIRMED_MS));
        assert!(!p.is_open());
    }

    #[test]
    fn a_failed_send_says_so_at_once_and_a_changed_list_resyncs_while_choosing() {
        let c = choices(&["Code", "Media"], 0, 1);
        let mut p = Picker::new();
        p.turn(Direction::Cw, &c, 0);
        p.turn(Direction::Cw, &c, 0);
        p.turn(Direction::Cw, &c, 10);
        // The list shrinks under the highlight: it stays on a real entry.
        let smaller = choices(&["Code"], 0, 2);
        p.on_choices(&smaller, 20);
        let v = p.view(&smaller, 20).unwrap();
        assert_eq!((v.index, v.count), (1, 2));
        assert_eq!(p.release(30), Some((2, 1)));
        p.send_failed(30);
        assert_eq!(
            p.view(&smaller, 31).unwrap().phase,
            PickerPhase::Unconfirmed
        );
    }

    #[test]
    fn with_nothing_pinned_there_is_only_auto() {
        let c = ProfileChoices::default();
        let mut p = Picker::new();
        p.turn(Direction::Cw, &c, 0);
        p.turn(Direction::Cw, &c, 0);
        let v = p.view(&c, 0).unwrap();
        assert_eq!((v.count, v.index), (1, 0));
    }
    #[test]
    fn release_and_session_close_reset_the_two_detent_threshold() {
        let c = choices(&["Code"], 0, 1);
        let mut p = Picker::new();
        p.turn(Direction::Cw, &c, 0);
        assert!(!p.is_open());
        assert_eq!(p.release(10), None);
        p.turn(Direction::Cw, &c, 20);
        assert!(!p.is_open());
        p.close();
        p.turn(Direction::Ccw, &c, 30);
        assert!(!p.is_open());
        p.turn(Direction::Cw, &c, 40);
        assert!(p.is_choosing());
    }
}
