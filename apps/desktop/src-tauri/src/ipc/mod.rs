//! The desktop ↔ webview IPC boundary (contracts/ipc.md): typed DTOs, least-privilege commands, and
//! redacted events. This is the entire privileged surface exposed to the React webview — no raw
//! serial, filesystem, shell, or credential access is ever offered (Constitution VIII).

/// Preview-frame streaming (dev-only; compiled out of release with Device Studio).
#[cfg(feature = "device-studio")]
pub mod channels;
pub mod commands;
pub mod dto;
pub mod events;
pub mod gnome;
