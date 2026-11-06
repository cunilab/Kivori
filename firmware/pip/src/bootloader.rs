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
}
