//! Device internals (ADR-0013): the hardware-neutral parts of the developer-mode snapshot.
//!
//! The run loop owns the load and frame-time figures ([`LoadMeter`]); the board supplies the rest
//! through a [`ChipProbe`]. Everything that only needs bytes lives here so the host tests cover it:
//! reading flash facts out of a partition table and an app image header, the stack high-water scan,
//! and the reset-reason mapping. A figure the board cannot produce stays `None` on the wire, never 0.

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

/// Maps the chip's reset-reason code (the same values ESP-IDF uses) to a reason a person can act on.
#[must_use]
pub const fn reset_reason_from_code(code: u8) -> ResetReason {
    match code {
        0x01 => ResetReason::PowerOn,
        0x03 | 0x0C => ResetReason::Software,
        0x05 => ResetReason::DeepSleep,
        0x07..=0x09 | 0x0B | 0x0D | 0x10..=0x12 => ResetReason::Watchdog,
        0x0F => ResetReason::Brownout,
        _ => ResetReason::Other,
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

/// Flash size in bytes from byte 3 of an ESP image header (upper nibble), `None` for a code this
/// firmware does not know.
#[must_use]
pub const fn flash_size_from_header(byte: u8) -> Option<u32> {
    match byte >> 4 {
        0 => Some(1 << 20),
        1 => Some(2 << 20),
        2 => Some(4 << 20),
        3 => Some(8 << 20),
        4 => Some(16 << 20),
        _ => None,
    }
}

/// Where the partition table sits in flash.
pub const PARTITION_TABLE_OFFSET: u32 = 0x8000;
/// Bytes of the table [`find_app_partition`] looks at: 16 entries of 32 bytes.
pub const PARTITION_TABLE_READ: usize = 512;
const PARTITION_ENTRY: usize = 32;

/// The first app partition (factory, else `ota_0`) in a raw partition table, as
/// `(flash offset, size)`.
#[must_use]
pub fn find_app_partition(table: &[u8]) -> Option<(u32, u32)> {
    let word = |entry: &[u8; PARTITION_ENTRY], at: usize| {
        u32::from_le_bytes([entry[at], entry[at + 1], entry[at + 2], entry[at + 3]])
    };
    let mut ota0 = None;
    for entry in table.as_chunks::<PARTITION_ENTRY>().0 {
        // Entries start with 0xAA 0x50; anything else (the MD5 record, erased flash) ends the table.
        if entry[0] != 0xAA || entry[1] != 0x50 {
            break;
        }
        let (kind, subtype) = (entry[2], entry[3]);
        if kind != 0x00 {
            continue;
        }
        let found = (word(entry, 4), word(entry, 8));
        match subtype {
            0x00 => return Some(found),
            0x10 if ota0.is_none() => ota0 = Some(found),
            _ => {}
        }
    }
    ota0
}

/// Size of the app image stored at flash offset `base`, read through `read(offset, buffer)` (which
/// returns `false` on failure): header, every segment, the checksum byte padded to 16 bytes, and
/// the appended SHA-256 when the header says there is one. `None` when there is no valid image
/// header or the walk would leave `limit` bytes.
pub fn app_image_bytes(
    base: u32,
    limit: u32,
    mut read: impl FnMut(u32, &mut [u8]) -> bool,
) -> Option<u32> {
    const HEADER: u32 = 24;
    const MAX_SEGMENTS: u8 = 16;
    let mut header = [0u8; 24];
    if !read(base, &mut header) || header[0] != 0xE9 || header[1] > MAX_SEGMENTS {
        return None;
    }
    let mut end = HEADER;
    for _ in 0..header[1] {
        let mut segment = [0u8; 8];
        if !read(base.checked_add(end)?, &mut segment) {
            return None;
        }
        let len = u32::from_le_bytes([segment[4], segment[5], segment[6], segment[7]]);
        end = end.checked_add(8)?.checked_add(len)?;
        if end > limit {
            return None;
        }
    }
    // The checksum byte sits at the last byte of a 16-byte row.
    end = end + (15 - end % 16) + 1;
    if header[23] == 1 {
        end = end.checked_add(32)?;
    }
    (end <= limit).then_some(end)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    fn entry(kind: u8, subtype: u8, offset: u32, size: u32) -> [u8; 32] {
        let mut e = [0u8; 32];
        e[0] = 0xAA;
        e[1] = 0x50;
        e[2] = kind;
        e[3] = subtype;
        e[4..8].copy_from_slice(&offset.to_le_bytes());
        e[8..12].copy_from_slice(&size.to_le_bytes());
        e
    }

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
    fn reset_codes_map_to_reasons() {
        assert_eq!(reset_reason_from_code(0x01), ResetReason::PowerOn);
        assert_eq!(reset_reason_from_code(0x03), ResetReason::Software);
        assert_eq!(reset_reason_from_code(0x0C), ResetReason::Software);
        assert_eq!(reset_reason_from_code(0x05), ResetReason::DeepSleep);
        assert_eq!(reset_reason_from_code(0x09), ResetReason::Watchdog);
        assert_eq!(reset_reason_from_code(0x0F), ResetReason::Brownout);
        assert_eq!(reset_reason_from_code(0x13), ResetReason::Other);
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
    fn flash_size_comes_from_the_header_nibble() {
        assert_eq!(flash_size_from_header(0x20), Some(4 << 20));
        assert_eq!(flash_size_from_header(0x2F), Some(4 << 20));
        assert_eq!(flash_size_from_header(0x40), Some(16 << 20));
        assert_eq!(flash_size_from_header(0x90), None);
    }

    #[test]
    fn the_app_partition_is_the_factory_one_else_ota_0() {
        let mut table = std::vec::Vec::new();
        table.extend(entry(0x01, 0x02, 0x9000, 0x6000)); // nvs
        table.extend(entry(0x00, 0x10, 0x20000, 0x1000)); // ota_0
        table.extend(entry(0x00, 0x00, 0x10000, 0x3F0000)); // factory
        assert_eq!(find_app_partition(&table), Some((0x10000, 0x3F0000)));

        let mut ota_only = std::vec::Vec::new();
        ota_only.extend(entry(0x00, 0x10, 0x20000, 0x1000));
        assert_eq!(find_app_partition(&ota_only), Some((0x20000, 0x1000)));

        // Erased flash ends the table; nothing is guessed.
        assert_eq!(find_app_partition(&[0xFF; 64]), None);
        assert_eq!(find_app_partition(&[]), None);
    }

    /// A flash image with an app at `base`: two segments of 100 and 5 bytes.
    fn flash_with_image(base: u32, hash: bool) -> std::vec::Vec<u8> {
        let mut flash = std::vec![0xFFu8; 0x1000];
        let at = base as usize;
        flash[at] = 0xE9;
        flash[at + 1] = 2;
        flash[at + 23] = u8::from(hash);
        let mut cursor = at + 24;
        for len in [100u32, 5] {
            flash[cursor + 4..cursor + 8].copy_from_slice(&len.to_le_bytes());
            cursor += 8 + len as usize;
        }
        flash
    }

    fn reader(flash: &[u8]) -> impl FnMut(u32, &mut [u8]) -> bool + '_ {
        |offset, buffer| {
            let from = offset as usize;
            flash
                .get(from..from + buffer.len())
                .map(|bytes| buffer.copy_from_slice(bytes))
                .is_some()
        }
    }

    #[test]
    fn the_image_size_pads_the_checksum_row_and_adds_an_appended_hash() {
        // 24 + (8 + 100) + (8 + 5) = 145; the checksum is byte 159, so the image ends at 160.
        let flash = flash_with_image(0x100, false);
        assert_eq!(app_image_bytes(0x100, 0x800, reader(&flash)), Some(160));
        let hashed = flash_with_image(0x100, true);
        assert_eq!(app_image_bytes(0x100, 0x800, reader(&hashed)), Some(192));
    }

    #[test]
    fn a_missing_or_oversized_image_is_unavailable() {
        let flash = flash_with_image(0x100, false);
        assert_eq!(app_image_bytes(0x100, 100, reader(&flash)), None);
        assert_eq!(app_image_bytes(0x400, 0x800, reader(&flash)), None); // erased: no magic
        assert_eq!(app_image_bytes(0x100, 0x800, |_, _| false), None);
    }
}
