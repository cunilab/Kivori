//! Board measurements for the device internals snapshot (ADR-0013), physical build only.
//!
//! Boot-time facts (flash size, image size, reset reason) are read once by [`init`] and kept; the
//! live ones (lowest free stack, die temperature) are read when a snapshot is asked for. Anything
//! that cannot be read stays `None`, which the desktop shows as unavailable.
//!
//! None of this is covered by the host tests, which see only the pure arithmetic in
//! [`crate::internals`]; it needs checking on a real device (validation rows 13.14 to 13.16).

use core::cell::Cell;
use core::sync::atomic::{AtomicUsize, Ordering};

use critical_section::Mutex;
use kivori_model::internals::{ChipModel, DeviceInternals};
use rp2040_hal::{pac, Adc};

use crate::internals::{flash_size_from_jedec, reset_reason_from_flags, untouched_stack_bytes};

extern "C" {
    // Linker-provided (cortex-m-rt): the lowest and highest stack addresses (no heap, so the stack
    // region is all RAM past the statics), and the load address / bounds of `.data` in flash.
    static _stack_end: u32;
    static _stack_start: u32;
    static __sidata: u32;
    static __sdata: u32;
    static __edata: u32;
}

/// Where RAM and flash (XIP) start.
const RAM_START: usize = 0x2000_0000;
const FLASH_START: usize = 0x1000_0000;
/// The pattern the unused stack is painted with.
const STACK_PATTERN: u32 = 0xA5C3_5A3C;
/// Bytes left unpainted below the stack pointer, for the painter's own frame.
const STACK_MARGIN: usize = 1024;

/// Words of stack painted at boot, counted from `_stack_end`. Zero until [`paint_stack`] ran.
static PAINTED_WORDS: AtomicUsize = AtomicUsize::new(0);
/// What [`init`] measured at boot.
static BOOT: Mutex<Cell<DeviceInternals>> = Mutex::new(Cell::new(empty()));

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
    let sp = cortex_m::register::msp::read() as usize;
    let low = addr(core::ptr::addr_of!(_stack_end));
    let high = sp.saturating_sub(STACK_MARGIN) & !3;
    if high <= low {
        return;
    }
    critical_section::with(|_| {
        let mut at = low;
        while at < high {
            // SAFETY: `[low, high)` lies in the stack region below the live stack, is
            // word-aligned, and nothing else runs while interrupts are off.
            unsafe { core::ptr::write_volatile(at as *mut u32, STACK_PATTERN) };
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
    let low = addr(core::ptr::addr_of!(_stack_end));
    Some(untouched_stack_bytes(words, None, STACK_PATTERN, |i| {
        // SAFETY: `i < words`, so the address is inside the region painted at boot.
        unsafe { core::ptr::read_volatile((low + i * 4) as *const u32) }
    }))
}

/// RAM not in use right now: with no heap, the stack headroom between its bottom and the stack
/// pointer.
fn ram_free_now() -> Option<u32> {
    let sp = cortex_m::register::msp::read() as usize;
    let low = addr(core::ptr::addr_of!(_stack_end));
    u32::try_from(sp.checked_sub(low)?).ok()
}

/// Measures the boot-time facts and starts the temperature sensor sampling. `asset_blob_bytes` is
/// the size of the asset blob baked into this image; `flash_jedec_id` is what the flash chip
/// answered to the JEDEC command (read by the caller, which also reads the unique id).
pub fn init(adc: pac::ADC, resets: &mut pac::RESETS, asset_blob_bytes: usize, flash_jedec_id: u32) {
    let mut facts = empty();

    facts.flash_total_bytes = flash_size_from_jedec(flash_jedec_id);
    // No partition table: the whole flash is the application's, and the image is the code and
    // read-only data plus the initial values of `.data` that follow it.
    facts.app_partition_bytes = facts.flash_total_bytes;
    let data_load = addr(core::ptr::addr_of!(__sidata));
    let data_len = addr(core::ptr::addr_of!(__edata)) - addr(core::ptr::addr_of!(__sdata));
    facts.app_image_bytes = u32::try_from(data_load + data_len - FLASH_START).ok();

    facts.asset_blob_bytes = u32::try_from(asset_blob_bytes).ok();

    let (stack_end, stack_start) = (
        addr(core::ptr::addr_of!(_stack_end)),
        addr(core::ptr::addr_of!(_stack_start)),
    );
    facts.ram_static_bytes = u32::try_from(stack_end.saturating_sub(RAM_START)).ok();
    facts.ram_total_bytes = u32::try_from(stack_start.saturating_sub(RAM_START)).ok();

    facts.chip_model = Some(ChipModel::Rp2040);
    // SAFETY: read-only access to two status registers.
    let (chip_reset, watchdog_reason) = unsafe {
        (
            (*pac::VREG_AND_CHIP_RESET::PTR).chip_reset().read().bits(),
            (*pac::WATCHDOG::PTR).reason().read().bits(),
        )
    };
    facts.reset_reason = Some(reset_reason_from_flags(chip_reset, watchdog_reason));

    // The temperature sensor is channel 4; free-running keeps the latest sample in `RESULT` so a
    // snapshot never waits for a conversion. Dropping the `Adc` handle leaves the block running.
    let mut adc = Adc::new(adc, resets);
    if let Some(sensor) = adc.take_temp_sensor() {
        adc.free_running(&sensor);
        TEMP_SENSOR.store(true, Ordering::Release);
    }

    critical_section::with(|cs| BOOT.borrow(cs).set(facts));
}

/// Set once the ADC is sampling the die temperature.
static TEMP_SENSOR: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// The die temperature in tenths of a degree (datasheet 4.9.5: 27 C at 0.706 V, -1.721 mV per C,
/// 3.3 V reference), if it reads inside the sensor's range.
fn temperature_decicelsius() -> Option<i16> {
    if !TEMP_SENSOR.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: read-only access to the ADC result register.
    let raw = unsafe { (*pac::ADC::PTR).result().read().result().bits() };
    let millivolts = i32::from(raw) * 3300 / 4096;
    let decicelsius = 270 - (millivolts - 706) * 581 / 100;
    // The sensor's range is -40 to 125 degrees; anything else is a bad read, not a temperature.
    (-400..=1250)
        .contains(&decicelsius)
        .then_some(decicelsius as i16)
}

/// The [`crate::internals::ChipProbe`] the physical runtime installs.
pub fn probe(out: &mut DeviceInternals) {
    *out = critical_section::with(|cs| BOOT.borrow(cs).get());
    out.ram_free_bytes = ram_free_now();
    out.stack_free_min_bytes = stack_free_min();
    out.temperature_decicelsius = temperature_decicelsius();
}
