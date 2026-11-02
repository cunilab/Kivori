//! Kivori desktop binary — a thin launcher that runs the library's Tauri runtime.
//!
//! All wiring (AppState, commands, events, window lifecycle, the background device task) lives in the
//! library (`lib.rs` → `run()`), so it stays host-testable; this binary only starts it.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    kivori_desktop::run();
}
