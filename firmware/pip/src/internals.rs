//! Device internals (ADR-0013): the hardware-neutral parts of the developer-mode snapshot.
//!
//! The run loop owns the load and frame-time figures ([`LoadMeter`]); the board supplies the rest
//! through a [`ChipProbe`]. Everything that only needs numbers lives here so the host tests cover it:
//! the flash size from the chip's JEDEC id, the stack high-water scan, and the reset-reason mapping. A figure the board cannot produce stays `None` on the wire, never 0.

use kivori_model::internals::{DeviceInternals, ResetReason};

/// Fills the snapshot fields only the board can measure (flash, RAM, chip, temperature). Fields it
/// leaves alone stay unavailable.
pub type ChipProbe = fn(&mut DeviceInternals);

/// The most the compiled asset blob may be (`kivori-asset-compiler` refuses to build a larger one).
pub const ASSET_BLOB_CAP_BYTES: u32 = 131_072;

/// How long a [`LoadMeter`] window is.
const WINDOW_MS: u32 = 1000;

/// Run-loop load and frame timing, over fixed one-second windows.
///
/// The loop never sleeps, so "busy" counts the time of ticks that did something (rendered a frame,
/// handled a received frame, or sent one) against the wall-clock time of the window. Figures appear
/// after the first full window and describe the last completed one.
///
/// Frame timings are averages per frame over that same window, so they agree with `fps`: the
/// window sums each frame's compose and flush time and divides by the frames timed in it. A window
/// that timed no frame leaves the previous averages in place (the cost of a frame does not change
/// because none was drawn, and an idle desk would otherwise lose them every second).
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadMeter {
    window_start: Option<u32>,
    busy_ms: u32,
    frames: u32,
    timed_frames: u32,
    compose_sum_ms: u32,
    flush_sum_ms: u32,
    busy_percent: Option<u8>,
    fps: Option<u8>,
    compose_ms: Option<u16>,
    flush_ms: Option<u16>,
}

impl LoadMeter {
    /// Nothing measured yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            window_start: None,
            busy_ms: 0,
            frames: 0,
            timed_frames: 0,
            compose_sum_ms: 0,
            flush_sum_ms: 0,
            busy_percent: None,
            fps: None,
            compose_ms: None,
            flush_ms: None,
        }
    }

    /// One run-loop tick ended at `now_ms`; it took `busy_ms` of real work (0 for a tick that only
    /// polled), and `rendered` says it composed a frame.
    pub fn on_tick(&mut self, now_ms: u32, busy_ms: u32, rendered: bool) {
        let start = *self.window_start.get_or_insert(now_ms);
        self.busy_ms = self.busy_ms.saturating_add(busy_ms);
        self.frames = self.frames.saturating_add(u32::from(rendered));
        let elapsed = now_ms.wrapping_sub(start);
        if elapsed >= WINDOW_MS {
            // A tick that ran long can make `busy_ms` exceed the window; cap at 100.
            self.busy_percent = Some((self.busy_ms.saturating_mul(100) / elapsed).min(100) as u8);
            let fps = (self.frames.saturating_mul(1000) + elapsed / 2) / elapsed;
            self.fps = Some(fps.min(u32::from(u8::MAX)) as u8);
            if self.timed_frames > 0 {
                let average = |sum: u32| {
                    let frames = self.timed_frames;
                    (sum.saturating_add(frames / 2) / frames).min(u32::from(u16::MAX)) as u16
                };
                self.compose_ms = Some(average(self.compose_sum_ms));
                self.flush_ms = Some(average(self.flush_sum_ms));
            }
            self.window_start = Some(now_ms);
            self.busy_ms = 0;
            self.frames = 0;
            self.timed_frames = 0;
            self.compose_sum_ms = 0;
            self.flush_sum_ms = 0;
        }
    }

    /// A frame took `compose_ms` of drawing and `flush_ms` of panel transfer (input and transport
    /// servicing between rows and windows already taken out). Call it before the tick's
    /// [`on_tick`](Self::on_tick) so the frame lands in the window that tick closes.
    pub fn on_frame(&mut self, compose_ms: u32, flush_ms: u32) {
        self.timed_frames = self.timed_frames.saturating_add(1);
        self.compose_sum_ms = self.compose_sum_ms.saturating_add(compose_ms);
        self.flush_sum_ms = self.flush_sum_ms.saturating_add(flush_ms);
    }

    /// Writes what has been measured into `out`.
    pub fn fill(&self, out: &mut DeviceInternals) {
        out.busy_percent = self.busy_percent;
        out.fps = self.fps;
        out.compose_ms = self.compose_ms;
        out.flush_ms = self.flush_ms;
    }
}

/// `CHIP_RESET` (`VREG_AND_CHIP_RESET`) bit: the last reset was power-on or brown-out.
const HAD_POR: u32 = 1 << 8;
/// `WATCHDOG.REASON` bits: the watchdog timer ran out / a firmware-forced watchdog reset.
const WATCHDOG_TIMER: u32 = 1 << 0;
const WATCHDOG_FORCE: u32 = 1 << 1;

/// Maps the RP2040's reset registers (`CHIP_RESET`, `WATCHDOG.REASON`) to a reason a person can
/// act on. The watchdog is read first: its reason survives a watchdog reset, while a power-on
/// clears it. The chip cannot tell a brown-out from a power-on, and a reset requested through the
/// core's `SYSRESETREQ` leaves both registers as they were, so that one reads as the earlier reason.
#[must_use]
pub const fn reset_reason_from_flags(chip_reset: u32, watchdog_reason: u32) -> ResetReason {
    if watchdog_reason & WATCHDOG_FORCE != 0 {
        ResetReason::Software
    } else if watchdog_reason & WATCHDOG_TIMER != 0 {
        ResetReason::Watchdog
    } else if chip_reset & HAD_POR != 0 {
        ResetReason::PowerOn
    } else {
        ResetReason::Other
    }
}

/// Bytes at the start of the stack region that no code has ever written, given a stack painted with
/// `pattern` at boot. `read(i)` returns the `i`th word of the region (lowest address first),
/// `words` is the painted length, and `skip` is one word to step over (the stack-protector guard,
/// which was never painted). The scan stops at the first word that is not the pattern.
pub fn untouched_stack_bytes(
    words: usize,
    skip: Option<usize>,
    pattern: u32,
    mut read: impl FnMut(usize) -> u32,
) -> u32 {
    let mut untouched = 0usize;
    for i in 0..words {
        if skip != Some(i) && read(i) != pattern {
            break;
        }
        untouched += 1;
    }
    (untouched * 4) as u32
}

/// Flash size in bytes from a JEDEC id (`manufacturer << 16 | type << 8 | capacity`): the capacity
/// byte is the power of two, `0x15` for 2 MB. `None` for a code outside 64 KiB to 16 MiB.
#[must_use]
pub const fn flash_size_from_jedec(id: u32) -> Option<u32> {
    let capacity = (id & 0xFF) as u8;
    if matches!(capacity, 0x10..=0x18) {
        Some(1 << capacity)
    } else {
        None
    }
}

/// The die temperature in tenths of a degree from a 12-bit ADC reading of the RP2040's sensor
/// (datasheet 4.9.5: 27 C at 0.706 V, -1.721 mV per C, 3.3 V reference), if it lies inside the
/// sensor's range of -40 to 125 degrees; anything else is a bad read, not a temperature.
#[must_use]
pub const fn temperature_decicelsius_from_adc(raw: u16) -> Option<i16> {
    let millivolts = raw as i32 * 3300 / 4096;
    let decicelsius = 270 - (millivolts - 706) * 581 / 100;
    if decicelsius >= -400 && decicelsius <= 1250 {
        Some(decicelsius as i16)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_reports_busy_percent_and_fps_once_it_has_closed() {
        let mut meter = LoadMeter::new();
        let mut out = DeviceInternals::default();
        meter.fill(&mut out);
        assert_eq!((out.busy_percent, out.fps), (None, None));

        // 30 frames of 10 ms of work, 34 ms apart.
        meter.on_tick(0, 0, false);
        for tick in 1..=30u32 {
            meter.on_tick(tick * 34, 10, true);
        }
        meter.fill(&mut out);
        assert_eq!(out.fps, Some(29)); // 30 frames over 1020 ms
        assert_eq!(out.busy_percent, Some(29)); // 300 ms over 1020 ms
    }

    #[test]
    fn a_new_window_replaces_the_figures_and_load_is_capped_at_100() {
        let mut meter = LoadMeter::new();
        meter.on_tick(0, 0, false);
        meter.on_tick(1000, 5000, true);
        let mut out = DeviceInternals::default();
        meter.fill(&mut out);
        assert_eq!(out.busy_percent, Some(100));
        assert_eq!(out.fps, Some(1));
        meter.on_tick(2000, 0, false);
        meter.fill(&mut out);
        assert_eq!((out.busy_percent, out.fps), (Some(0), Some(0)));
    }

    #[test]
    fn the_window_survives_the_millisecond_clock_wrapping() {
        let mut meter = LoadMeter::new();
        meter.on_tick(u32::MAX - 400, 0, false);
        meter.on_tick(599, 500, true);
        let mut out = DeviceInternals::default();
        meter.fill(&mut out);
        assert_eq!(out.busy_percent, Some(50));
    }

    #[test]
    fn frame_times_are_the_per_frame_average_of_the_completed_window() {
        let mut meter = LoadMeter::new();
        let mut out = DeviceInternals::default();
        meter.on_tick(0, 0, false);
        // Three frames in the window: compose 10, 20 and 90 ms; flush 30, 30 and 60 ms.
        for (tick, (compose, flush)) in [(10, 30), (20, 30), (90, 60)].into_iter().enumerate() {
            meter.on_frame(compose, flush);
            meter.on_tick((tick as u32 + 1) * 300, 100, true);
        }
        // Nothing is reported while the window is still open (900 ms in).
        meter.fill(&mut out);
        assert_eq!((out.compose_ms, out.flush_ms), (None, None));
        meter.on_tick(1000, 0, false);
        meter.fill(&mut out);
        assert_eq!((out.compose_ms, out.flush_ms), (Some(40), Some(40)));
        assert_eq!(out.fps, Some(3));
    }

    #[test]
    fn each_window_averages_only_its_own_frames_and_an_idle_one_keeps_the_last() {
        let mut meter = LoadMeter::new();
        let mut out = DeviceInternals::default();
        meter.on_tick(0, 0, false);
        meter.on_frame(100, 10); // drawn by the tick that closes the first window
        meter.on_tick(1000, 110, true);
        meter.fill(&mut out);
        assert_eq!((out.compose_ms, out.flush_ms), (Some(100), Some(10)));
        meter.on_frame(20, 6);
        meter.on_tick(1500, 26, true);
        meter.on_frame(40, 10);
        meter.on_tick(2000, 50, true);
        meter.fill(&mut out);
        // Not the first window's frame, and not just the last frame either.
        assert_eq!((out.compose_ms, out.flush_ms), (Some(30), Some(8)));
        // A window with no frame leaves the averages as they were, while fps drops to 0.
        meter.on_tick(3000, 0, false);
        meter.fill(&mut out);
        assert_eq!((out.compose_ms, out.flush_ms), (Some(30), Some(8)));
        assert_eq!(out.fps, Some(0));
    }

    #[test]
    fn frame_time_averages_round_to_nearest_and_are_clamped() {
        let mut meter = LoadMeter::new();
        let mut out = DeviceInternals::default();
        meter.on_tick(0, 0, false);
        meter.on_frame(8, 70_000);
        meter.on_frame(7, 70_000);
        meter.on_tick(1000, 0, true);
        meter.fill(&mut out);
        assert_eq!((out.compose_ms, out.flush_ms), (Some(8), Some(u16::MAX))); // 7.5 rounds up
    }

    #[test]
    fn reset_registers_map_to_reasons() {
        assert_eq!(reset_reason_from_flags(HAD_POR, 0), ResetReason::PowerOn);
        assert_eq!(
            reset_reason_from_flags(0, WATCHDOG_FORCE),
            ResetReason::Software
        );
        assert_eq!(
            reset_reason_from_flags(0, WATCHDOG_TIMER),
            ResetReason::Watchdog
        );
        // The watchdog reason outlives a power-on flag left over from an earlier boot.
        assert_eq!(
            reset_reason_from_flags(HAD_POR, WATCHDOG_TIMER),
            ResetReason::Watchdog
        );
        assert_eq!(reset_reason_from_flags(1 << 16, 0), ResetReason::Other);
    }

    #[test]
    fn the_stack_scan_counts_untouched_words_and_steps_over_the_guard() {
        const P: u32 = 0xDEAD_BEEF;
        let stack = [P, P, 0xAAAA_AAAA, P, 7];
        assert_eq!(untouched_stack_bytes(5, None, P, |i| stack[i]), 8);
        // The guard word was never painted; it must not end the scan.
        let guarded = [P, 0x1234_5678, P, 1];
        assert_eq!(untouched_stack_bytes(4, Some(1), P, |i| guarded[i]), 12);
        assert_eq!(untouched_stack_bytes(0, None, P, |_| 0), 0);
        assert_eq!(untouched_stack_bytes(3, None, P, |_| P), 12);
    }

    #[test]
    fn flash_size_comes_from_the_jedec_capacity_byte() {
        assert_eq!(flash_size_from_jedec(0xEF_40_15), Some(2 << 20)); // W25Q16
        assert_eq!(flash_size_from_jedec(0xEF_70_15), Some(2 << 20)); // W25Q16JV
        assert_eq!(flash_size_from_jedec(0xEF_40_18), Some(16 << 20));
        assert_eq!(flash_size_from_jedec(0xEF_40_00), None);
        assert_eq!(flash_size_from_jedec(0xFF_FF_FF), None); // no chip answered
    }

    #[test]
    fn the_sensor_reads_27_degrees_at_0_706_volts() {
        // 0.706 V of 3.3 V over 12 bits is a raw reading of about 876.
        let at_27 = temperature_decicelsius_from_adc(876).unwrap();
        assert!((260..=280).contains(&at_27), "{at_27}");
    }

    #[test]
    fn only_a_contiguous_window_of_readings_is_a_temperature_and_it_never_rises_with_the_reading() {
        let (mut first, mut last, mut count) = (None, 0u16, 0u16);
        let mut previous: Option<i16> = None;
        let (mut saw_20, mut saw_60) = (false, false);
        for raw in 0..=4095u16 {
            let Some(t) = temperature_decicelsius_from_adc(raw) else {
                continue;
            };
            assert!((-400..=1250).contains(&t), "raw {raw}: {t}");
            // A hotter die has a lower voltage, so the temperature never rises with the reading
            // (a millivolt is a little more than one count, so two counts can share a value).
            assert!(previous.is_none_or(|p| t <= p), "raw {raw} rose");
            previous = Some(t);
            first.get_or_insert(raw);
            last = raw;
            count += 1;
            saw_20 |= (195..=205).contains(&t);
            saw_60 |= (595..=605).contains(&t);
        }
        let first = first.expect("some reading is a temperature");
        assert!(
            count > 300,
            "the window spans the sensor's whole range: {count} counts"
        );
        assert_eq!(last - first + 1, count, "the window is contiguous");
        // Just outside it on both sides is rejected, not clamped.
        assert_eq!(temperature_decicelsius_from_adc(first - 1), None);
        assert_eq!(temperature_decicelsius_from_adc(last + 1), None);
        // A plausible desk (20 to 60 C) sits well inside it.
        assert!(saw_20 && saw_60);
    }

    #[test]
    fn a_shorted_or_saturated_sensor_is_unavailable_not_a_temperature() {
        assert_eq!(temperature_decicelsius_from_adc(0), None); // pulled to ground
        assert_eq!(temperature_decicelsius_from_adc(4095), None); // pulled to the reference
        assert_eq!(temperature_decicelsius_from_adc(u16::MAX), None); // the 12-bit field is never wider
    }

    #[test]
    fn every_reset_register_combination_maps_to_one_reason() {
        const HAD_RUN: u32 = 1 << 16;
        const HAD_PSM_RESTART: u32 = 1 << 20;
        // The watchdog force bit wins over everything, then the timer, then power-on, else other.
        for chip in [
            0,
            HAD_POR,
            HAD_RUN,
            HAD_PSM_RESTART,
            HAD_POR | HAD_RUN | HAD_PSM_RESTART,
        ] {
            for (watchdog, expected) in [
                (WATCHDOG_FORCE | WATCHDOG_TIMER, Some(ResetReason::Software)),
                (WATCHDOG_FORCE, Some(ResetReason::Software)),
                (WATCHDOG_TIMER, Some(ResetReason::Watchdog)),
                (0, None),
            ] {
                let expected = expected.unwrap_or(if chip & HAD_POR != 0 {
                    ResetReason::PowerOn
                } else {
                    ResetReason::Other
                });
                assert_eq!(
                    reset_reason_from_flags(chip, watchdog),
                    expected,
                    "chip {chip:#x}, watchdog {watchdog:#x}"
                );
            }
        }
        // Bits no reason owns change nothing.
        assert_eq!(
            reset_reason_from_flags(0, !(WATCHDOG_FORCE | WATCHDOG_TIMER)),
            ResetReason::Other
        );
    }

    #[test]
    fn jedec_capacity_codes_are_a_power_of_two_between_64_kib_and_16_mib() {
        assert_eq!(flash_size_from_jedec(0x10), Some(64 << 10));
        assert_eq!(flash_size_from_jedec(0x0F), None);
        assert_eq!(flash_size_from_jedec(0x19), None);
        // Only the capacity byte counts: the manufacturer and type bytes never change the size.
        assert_eq!(flash_size_from_jedec(0x00_00_15), Some(2 << 20));
        assert_eq!(flash_size_from_jedec(0xAB_CD_15), Some(2 << 20));
        assert_eq!(flash_size_from_jedec(0xFF00_0015), Some(2 << 20));
        // A high bit beyond the capacity byte must not turn an unknown code into a size.
        assert_eq!(flash_size_from_jedec(0x1_00), None);
        assert_eq!(flash_size_from_jedec(u32::MAX), None);
        for code in 0x10u32..=0x18 {
            assert_eq!(flash_size_from_jedec(code), Some(1 << code));
        }
    }
}
