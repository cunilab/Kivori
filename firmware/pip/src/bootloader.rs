//! The "1200-baud touch": how a host asks the running firmware to reboot into the RP2040 USB
//! bootloader (UF2 mass storage) so it can be updated without touching the BOOTSEL button.
//! Hardware-neutral so the decision is host-tested; `transport` acts on it.

/// The line coding a host sets to request the bootloader.
pub const TOUCH_BAUD: u32 = 1200;

/// `true` when the host has set the CDC line coding to [`TOUCH_BAUD`] and dropped DTR (closed the
/// port). A normal session never does this: the app opens at another rate and keeps DTR up.
#[must_use]
pub const fn is_touch(baud: u32, dtr: bool) -> bool {
    baud == TOUCH_BAUD && !dtr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_1200_baud_with_dtr_dropped_is_a_touch() {
        assert!(is_touch(1200, false));
        assert!(!is_touch(1200, true), "port still open at 1200 baud");
        assert!(!is_touch(115_200, false), "an ordinary close");
        assert!(!is_touch(9600, false), "the CDC power-on default");
    }

    #[test]
    fn the_touch_is_decided_from_the_line_state_alone() {
        // (baud, dtr, touch): every combination around the one that counts.
        for (baud, dtr, touch) in [
            (1200, false, true),
            (1200, true, false),
            (0, false, false), // a coding the host never set is not 1200
            (0, true, false),
            (1199, false, false),
            (1201, false, false),
            (300, false, false), // another legacy rate
            (2400, false, false),
            (9600, false, false), // `usbd-serial`'s power-on default, and after every bus reset
            (115_200, true, false),
            (u32::MAX, false, false),
        ] {
            assert_eq!(is_touch(baud, dtr), touch, "baud {baud}, dtr {dtr}");
        }
    }

    /// The USB handler calls `is_touch` on every poll with whatever the line state is then, and acts
    /// on the first `true`. Replays host sequences poll by poll; the index of the first touch.
    fn first_touch(polls: &[(u32, bool)]) -> Option<usize> {
        polls.iter().position(|&(baud, dtr)| is_touch(baud, dtr))
    }

    #[test]
    fn the_touch_fires_on_the_first_poll_that_sees_1200_baud_with_dtr_low() {
        // Held open at 1200 with DTR up: nothing until DTR drops.
        assert_eq!(
            first_touch(&[(1200, true), (1200, true), (1200, false)]),
            Some(2)
        );
        assert_eq!(
            first_touch(&[(9600, false), (1200, true), (1200, true)]),
            None
        );
        // The coding is set before DTR rises, so a poll in between sees 1200 with DTR low: a touch,
        // as in the Arduino convention this follows.
        assert_eq!(
            first_touch(&[(9600, false), (1200, false), (1200, true)]),
            Some(1)
        );
    }

    #[test]
    fn leaving_1200_baud_before_the_port_closes_never_touches() {
        // The app opens at 1200 for a moment, then switches to its working rate, then closes.
        let session = [
            (1200, true),
            (115_200, true),
            (115_200, true),
            (115_200, false),
        ];
        assert_eq!(first_touch(&session), None);
        // A normal session never closes at 1200 baud, so the closing poll is an ordinary one.
        assert_eq!(
            first_touch(&[(115_200, true), (115_200, false), (9600, false)]),
            None
        );
    }
}
