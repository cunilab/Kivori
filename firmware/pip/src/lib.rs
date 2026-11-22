#![no_std]
#![warn(missing_docs)]
//! `kivori-firmware` — the RP2040-Zero device firmware.
//!
//! The device's behaviour (lifecycle FSM, protocol dispatcher, heartbeat, change-driven rendering)
//! is written against hardware-neutral [`ports`] so it runs identically on real rp2040-hal peripherals
//! (the `embedded` binary) and on the host [`sim`] adapters (the `host-sim` feature). This
//! keeps every device decision host-testable against the shared golden frames (constitution
//! Principle IV, constraint 4).

/// The 1200-baud touch decision for entering the USB bootloader (hardware-neutral).
pub mod bootloader;
/// Board bring-up shared by every `embedded` mode: clocks and the monotonic timer (T070).
#[cfg(feature = "embedded")]
pub mod bsp;
#[cfg(feature = "embedded")]
pub mod clock;
/// Raw-payload tracing, development-only (T102). Compiled out unless `debug-payloads` is enabled.
#[cfg(all(feature = "debug-payloads", feature = "embedded"))]
pub mod debug_payloads;
/// Per-unit device ID derivation from the flash unique ID (hardware-neutral, issue #24).
pub mod device_id;
pub mod display;
pub mod health;
/// Product identity: the build's product and board, the image record, and the wrong-board check (ADR-0014).
pub mod identity;
pub mod input;
/// Device internals snapshot pieces (ADR-0013): load meter, flash and stack arithmetic.
pub mod internals;
/// Detent -> on-panel latency readout, development-only (Slice 002 checklist row 14).
#[cfg(feature = "latency-probe")]
pub mod latency_probe;
/// The profile picker's device-side state (`PROFILE_PICKER_V1`): turn with the switch held, release to pick.
pub mod picker;
pub mod ports;
/// Board profiles: the verified physical pin map and panel facts.
pub mod profile;
pub mod proto;
pub mod render;
/// The production device runtime (T074): the single run loop the physical firmware runs and the host-sim
/// tests drive.
pub mod runtime;
pub mod state;

#[cfg(feature = "host-sim")]
pub mod sim;

/// Real USB CDC-ACM transport (also the hardware adapter core, T071).
#[cfg(feature = "embedded")]
pub mod transport;
/// Whole-frame outbound queue (hardware-neutral).
pub mod tx_buffer;
/// The USB interrupt's service step (hardware-neutral, host-tested).
pub mod usb_service;

/// Board measurements for the device internals snapshot (flash, RAM, stack, chip, temperature).
#[cfg(feature = "physical-st7789")]
pub mod chip_probe;
/// Physical HW-040 rotary encoder `InputSource` adapter.
#[cfg(feature = "physical-st7789")]
pub mod physical_rotary;
#[cfg(feature = "physical-st7789")]
pub mod physical_st7789;
