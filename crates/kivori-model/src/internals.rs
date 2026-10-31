//! Device internals (ADR-0013): one fixed-size snapshot of what the firmware can measure about
//! itself, for the desktop's developer-mode card.
//!
//! Every field is optional. `None` means the device cannot measure that value (never "zero"), and
//! the desktop shows it as unavailable. Counters are `u32`, wrap, and reset at boot. Nothing here
//! identifies the device or carries payload bytes (ADR-0005). The struct and its enums are
//! wire-significant: fields are never reordered or removed, new ones go at the end.

use serde::{Deserialize, Serialize};

/// The chip family the firmware runs on. Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChipModel {
    /// Espressif ESP32-C3.
    Esp32C3,
}

/// Why the chip last reset, reduced to what a person can act on. Wire-significant, append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResetReason {
    /// Power was applied.
    PowerOn,
    /// The firmware asked for a reset (the recovery hold, or a flash tool).
    Software,
    /// A watchdog fired.
    Watchdog,
    /// The supply voltage dropped.
    Brownout,
    /// Woke from deep sleep.
    DeepSleep,
    /// Any other reason the chip reports.
    Other,
}

/// The snapshot. See the module docs for the rules every field follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeviceInternals {
    /// Size of the running app image in flash.
    pub app_image_bytes: Option<u32>,
    /// Size of the app partition that image lives in.
    pub app_partition_bytes: Option<u32>,
    /// Size of the compiled asset blob.
    pub asset_blob_bytes: Option<u32>,
    /// The most the asset blob may be (its build-time cap).
    pub asset_blob_cap_bytes: Option<u32>,
    /// Total flash, as the bootloader image header declares it.
    pub flash_total_bytes: Option<u32>,
    /// Data RAM the firmware can use: static data plus the stack region.
    pub ram_total_bytes: Option<u32>,
    /// RAM taken by static data (`.data`, `.bss`, the frame buffer).
    pub ram_static_bytes: Option<u32>,
    /// RAM not in use right now. The firmware has no heap, so this is the stack headroom at the
    /// moment of the snapshot.
    pub ram_free_bytes: Option<u32>,
    /// Least stack that has been free since boot.
    pub stack_free_min_bytes: Option<u32>,
    /// Share of the last second the run loop spent doing work, 0 to 100.
    pub busy_percent: Option<u8>,
    /// Average time per frame spent drawing it, ms, over the last completed second (the window
    /// `fps` covers). Input and transport servicing between tile rows is not counted. After an
    /// idle second it keeps the last second that drew a frame.
    pub compose_ms: Option<u16>,
    /// Average time per frame spent transferring it to the panel, ms, over the same second as
    /// `compose_ms`. Input and transport servicing between display windows is not counted.
    pub flush_ms: Option<u16>,
    /// Frames rendered per second over the last second.
    pub fps: Option<u8>,
    /// The chip family.
    pub chip_model: Option<ChipModel>,
    /// Silicon revision as `(major, minor)`.
    pub chip_revision: Option<(u8, u8)>,
    /// Internal chip temperature, in tenths of a degree Celsius.
    pub temperature_decicelsius: Option<i16>,
    /// Time since boot, ms (wraps).
    pub uptime_ms: Option<u32>,
    /// Why the chip last reset.
    pub reset_reason: Option<ResetReason>,
    /// The most rotary edges ever waiting in the capture queue at once.
    pub edge_queue_high_water: Option<u16>,
    /// Rotary edges dropped because the capture queue was full.
    pub dropped_edges: Option<u32>,
    /// Electrically impossible encoder transitions seen.
    pub invalid_transitions: Option<u32>,
    /// Valid frames received from the host.
    pub frames_in: Option<u32>,
    /// Frames sent to the host.
    pub frames_out: Option<u32>,
    /// Frames received and rejected (framing, checksum, version, payload).
    pub frames_rejected: Option<u32>,
}
