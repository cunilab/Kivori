//! Linux backends: PulseAudio-protocol audio (PipeWire serves it through `pipewire-pulse`), MPRIS
//! media, X11 key synthesis and focus, `/proc` CPU/RAM, XDG desktop entries and systemd-logind
//! sleep/lock. Target: Ubuntu / GNOME on X11 and Wayland.
//!
//! Wayland is the honest gap. No portal-free way exists to synthesize keys or to read the focused
//! window there, so on a Wayland session shortcuts report `RuntimeUnavailable` and the foreground
//! is `Unknown` (only the General profile applies). Media keys and observation use MPRIS over
//! D-Bus and work on both session types. See each module for the contract it honours.

mod apps;
mod audio;
mod foreground;
pub mod gnome;
mod gnome_focus;
pub mod host_events;
mod keys;
mod media;
mod mic;
mod proc;
mod pulse;
mod session;
mod system;
mod x11;

pub use apps::{icon_png, launch_target, running_apps};
pub use audio::LinuxVolumeBackend;
pub use foreground::LinuxForeground;
pub use keys::LinuxInputSynth;
pub use media::{LinuxMediaObserver, Mpris};
pub use mic::LinuxMicBackend;
pub use session::{session_kind, SessionKind};
pub use system::{local_time, LinuxSystemProbe};
