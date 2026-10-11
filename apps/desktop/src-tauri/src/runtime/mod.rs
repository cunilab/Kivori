//! The native runtime: managed [`state::AppState`], the background [`device_task`] (which owns device
//! communication independently of the webview), and the window/tray [`lifecycle`] wiring.

pub mod device_task;
pub mod lifecycle;
pub mod splash;
pub mod state;

pub use state::{AppState, DeviceCommand};
