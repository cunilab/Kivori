//! Board measurements for the device internals snapshot (ADR-0013), physical build only.
//!
//! Boot-time facts (flash layout, chip revision, reset reason) are read once by [`init`] and kept;
//! the live ones (lowest free stack, temperature) are read when a snapshot is asked for. Anything
//! that cannot be read stays `None`, which the desktop shows as unavailable.
//!
//! None of this is covered by the host tests, which see only the pure arithmetic in
//! [`crate::internals`]; it needs checking on a real device (validation rows 13.14 to 13.16).

use core::cell::{Cell, RefCell};
use core::sync::atomic::{AtomicUsize, Ordering};

use critical_section::Mutex;
use esp_hal::peripherals::TSENS;
use esp_hal::tsens::{Config, TemperatureSensor};
use kivori_model::internals::{ChipModel, DeviceInternals};

use crate::internals::{
    app_image_bytes, find_app_partition, flash_size_from_header, reset_reason_from_code,
    untouched_stack_bytes, PARTITION_TABLE_OFFSET, PARTITION_TABLE_READ,
};

extern "C" {
    /// ROM flash read (the same entry point `esp-storage` uses). `len` is in bytes, a multiple of
    /// four, and `src_addr` is four-byte aligned.
    fn esp_rom_spiflash_read(src_addr: u32, data: *mut u32, len: u32) -> i32;

    // Linker-provided: the lowest and highest stack addresses, and the stack-protector word.
    static _stack_end: u32;
    static _stack_start: u32;
    static __stack_chk_guard: u32;
}

/// Where data RAM starts. RAM-resident code shares it, so "static" below counts that too.
const DRAM_START: usize = 0x3FC8_0000;
/// The pattern the unused stack is painted with.
const STACK_PATTERN: u32 = 0xA5C3_5A3C;
/// Bytes left unpainted below the stack pointer, for the painter's own frame.
const STACK_MARGIN: usize = 1024;

/// Words of stack painted at boot, counted from `_stack_end`. Zero until [`paint_stack`] ran.
static PAINTED_WORDS: AtomicUsize = AtomicUsize::new(0);
/// What [`init`] measured at boot.
static BOOT: Mutex<Cell<DeviceInternals>> = Mutex::new(Cell::new(empty()));
static TSENS_DRIVER: Mutex<RefCell<Option<TemperatureSensor<'static>>>> =
    Mutex::new(RefCell::new(None));

const fn empty() -> DeviceInternals {
    DeviceInternals {
        app_image_bytes: None,
        app_partition_bytes: None,
        asset_blob_bytes: None,
        asset_blob_cap_bytes: None,
        flash_total_bytes: None,
        ram_total_bytes: None,
        ram_static_bytes: None,
        ram_free_bytes: None,
        stack_free_min_bytes: None,
        busy_percent: None,
        compose_ms: None,
        flush_ms: None,
        fps: None,
        chip_model: None,
        chip_revision: None,
        temperature_decicelsius: None,
        uptime_ms: None,
        reset_reason: None,
        edge_queue_high_water: None,
        dropped_edges: None,
        invalid_transitions: None,
        frames_in: None,
        frames_out: None,
        frames_rejected: None,
    }
}

fn addr(symbol: *const u32) -> usize {
    symbol as usize
}

/// Paints the unused stack so [`probe`] can later find how deep it ever went. Call once, as early
/// as possible: it paints only below the current stack pointer (less a margin), with interrupts
/// off, so nothing live is touched.
#[inline(never)]
pub fn paint_stack() {
    let sp: usize;
    // SAFETY: reads the stack pointer register only.
    unsafe { core::arch::asm!("mv {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    let (low, guard) = (
        addr(core::ptr::addr_of!(_stack_end)),
        addr(core::ptr::addr_of!(__stack_chk_guard)),
    );
    let high = sp.saturating_sub(STACK_MARGIN) & !3;
    if high <= low {
        return;
    }
    critical_section::with(|_| {
        let mut at = low;
        while at < high {
            // The guard word is not stack; leave it as the HAL wrote it.
            if at != guard {
                // SAFETY: `[low, high)` lies in the stack region below the live stack, is
                // word-aligned, and nothing else runs while interrupts are off.
                unsafe { core::ptr::write_volatile(at as *mut u32, STACK_PATTERN) };
            }
            at += 4;
        }
    });
    PAINTED_WORDS.store((high - low) / 4, Ordering::Release);
}

/// Least stack ever free, in bytes: the painted words from the bottom that are still untouched.
fn stack_free_min() -> Option<u32> {
    let words = PAINTED_WORDS.load(Ordering::Acquire);
    if words == 0 {
        return None;
    }
    let (low, guard) = (
        addr(core::ptr::addr_of!(_stack_end)),
        addr(core::ptr::addr_of!(__stack_chk_guard)),
    );
    let skip = guard.checked_sub(low).map(|bytes| bytes / 4);
    Some(untouched_stack_bytes(words, skip, STACK_PATTERN, |i| {
        // SAFETY: `i < words`, so the address is inside the region painted at boot.
        unsafe { core::ptr::read_volatile((low + i * 4) as *const u32) }
    }))
}

/// RAM not in use right now: with no heap, the stack headroom between its bottom and the stack
/// pointer.
fn ram_free_now() -> Option<u32> {
    let sp: usize;
    // SAFETY: reads the stack pointer register only.
    unsafe { core::arch::asm!("mv {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    let low = addr(core::ptr::addr_of!(_stack_end));
    u32::try_from(sp.checked_sub(low)?).ok()
}

/// Reads `out.len()` bytes of flash at `offset`; `false` if the ROM reports an error.
#[esp_hal::ram]
fn flash_read(offset: u32, out: &mut [u8]) -> bool {
    let mut words = [0u32; 16];
    let mut done = 0;
    while done < out.len() {
        let at = offset + done as u32;
        let aligned = at & !3;
        let skew = (at - aligned) as usize;
        let n_words = (out.len() - done + skew).min(64).div_ceil(4);
        // SAFETY: `words` holds `n_words` words and the ROM writes only that many.
        let status = critical_section::with(|_| unsafe {
            esp_rom_spiflash_read(aligned, words.as_mut_ptr(), (n_words * 4) as u32)
        });
        if status != 0 {
            return false;
        }
        let mut chunk = [0u8; 64];
        for (bytes, word) in chunk
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&words[..n_words])
        {
            *bytes = word.to_le_bytes();
        }
        let take = (n_words * 4 - skew).min(out.len() - done);
        out[done..done + take].copy_from_slice(&chunk[skew..skew + take]);
        done += take;
    }
    true
}

/// Measures the boot-time facts and starts the temperature sensor. `asset_blob_bytes` is the size
/// of the asset blob baked into this image.
pub fn init(tsens: TSENS<'static>, asset_blob_bytes: usize) {
    let mut facts = empty();

    let mut header = [0u8; 4];
    if flash_read(0, &mut header) {
        facts.flash_total_bytes = flash_size_from_header(header[3]);
    }
    let mut table = [0u8; PARTITION_TABLE_READ];
    if flash_read(PARTITION_TABLE_OFFSET, &mut table) {
        if let Some((offset, size)) = find_app_partition(&table) {
            facts.app_partition_bytes = Some(size);
            facts.app_image_bytes = app_image_bytes(offset, size, flash_read);
        }
    }

    facts.asset_blob_bytes = u32::try_from(asset_blob_bytes).ok();

    let (stack_end, stack_start) = (
        addr(core::ptr::addr_of!(_stack_end)),
        addr(core::ptr::addr_of!(_stack_start)),
    );
    facts.ram_static_bytes = u32::try_from(stack_end.saturating_sub(DRAM_START)).ok();
    facts.ram_total_bytes = u32::try_from(stack_start.saturating_sub(DRAM_START)).ok();

    facts.chip_model = Some(ChipModel::Esp32C3);
    facts.chip_revision = Some((
        esp_hal::efuse::major_chip_version(),
        esp_hal::efuse::minor_chip_version(),
    ));
    facts.reset_reason = esp_hal::system::reset_reason().map(|reason| {
        // The enum's values are the chip's reset-reason codes.
        reset_reason_from_code(reason as u32 as u8)
    });

    if let Ok(sensor) = TemperatureSensor::new(tsens, Config::default()) {
        // The sensor wants a few hundred microseconds to settle after power-up.
        esp_hal::delay::Delay::new().delay_micros(300);
        critical_section::with(|cs| TSENS_DRIVER.borrow_ref_mut(cs).replace(sensor));
    }

    critical_section::with(|cs| BOOT.borrow(cs).set(facts));
}

/// The chip's internal temperature in tenths of a degree, if the sensor reads inside its range.
fn temperature_decicelsius() -> Option<i16> {
    let celsius = critical_section::with(|cs| {
        TSENS_DRIVER
            .borrow_ref(cs)
            .as_ref()
            .map(|sensor| sensor.get_temperature().to_celsius())
    })?;
    // The sensor's range is -40 to 125 degrees; anything else is a bad read, not a temperature.
    (-40.0..=125.0)
        .contains(&celsius)
        .then_some((celsius * 10.0) as i16)
}

/// The [`crate::internals::ChipProbe`] the physical runtime installs.
pub fn probe(out: &mut DeviceInternals) {
    *out = critical_section::with(|cs| BOOT.borrow(cs).get());
    out.ram_free_bytes = ram_free_now();
    out.stack_free_min_bytes = stack_free_min();
    out.temperature_decicelsius = temperature_decicelsius();
}
