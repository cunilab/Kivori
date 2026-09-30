#![no_std]
#![warn(missing_docs)]
//! `kivori-firmware` — the ESP32-C3 device firmware.
//!
//! The device's behaviour (lifecycle FSM, protocol dispatcher, heartbeat, change-driven rendering)
//! is written against hardware-neutral [`ports`] so it runs identically on real esp-hal peripherals
//! (the `embedded` binary, Phase 8) and on the host [`sim`] adapters (the `host-sim` feature). This
//! keeps every device decision host-testable against the shared golden frames (constitution
//! Principle IV, constraint 4).

/// Board bring-up: clocks, USB Serial/JTAG, and the display SPI bus (T070). No physical pin, panel
/// controller, or offset is chosen here — every board fact is a parameter.
#[cfg(feature = "embedded")]
pub mod bsp;
#[cfg(feature = "embedded")]
pub mod clock;
/// Raw-payload tracing, development-only (T102). Compiled out unless `debug-payloads` is enabled.
#[cfg(all(feature = "debug-payloads", feature = "embedded"))]
pub mod debug_payloads;
pub mod display;
pub mod health;
pub mod input;
/// Detent -> on-panel latency readout, development-only (Slice 002 checklist row 14).
#[cfg(feature = "latency-probe")]
pub mod latency_probe;
pub mod ports;
/// Board profiles. The only concrete profile is simulation-only; no physical panel is described anywhere.
pub mod profile;
pub mod proto;
pub mod render;
/// The production device runtime (T074): the single run loop shared by physical firmware and simulation.
pub mod runtime;
pub mod state;

#[cfg(feature = "host-sim")]
pub mod sim;

/// Wokwi pre-hardware simulation: probe adapters + the in-firmware self-test harness. Simulation is an
/// additional gate; it never replaces the host-sim tests or physical validation.
#[cfg(feature = "wokwi")]
pub mod selftest;

/// External serial test mode: the real USB Serial/JTAG receive/transmit loop under Wokwi automation.
#[cfg(feature = "wokwi-serial")]
pub mod external;
#[cfg(feature = "wokwi")]
pub mod sim_probe;
/// Wokwi stage-2 GENERIC SPI/RGB565 tile-transfer probe (T131). Never a controller validation.
#[cfg(feature = "wokwi-spi")]
pub mod spi_probe;
/// Real USB Serial/JTAG transport (also the hardware adapter core, T071).
#[cfg(any(feature = "wokwi-serial", feature = "embedded"))]
pub mod transport;
/// Wokwi PRODUCTION-RUNTIME mode: constructs ports for the real [`runtime::run`] loop (T074).
#[cfg(feature = "wokwi-runtime")]
pub mod wokwi_runtime;

/// Physical HW-040 rotary encoder `InputSource` adapter (Task 13).
#[cfg(feature = "physical-st7789")]
pub mod physical_rotary;
#[cfg(feature = "physical-st7789")]
pub mod physical_st7789;
