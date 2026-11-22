//! Linux backends: PulseAudio-protocol audio (PipeWire serves it through `pipewire-pulse`), MPRIS
//! media, X11 key synthesis and focus, `/proc` CPU/RAM, XDG desktop entries and systemd-logind
//! sleep/lock. Target: Ubuntu / GNOME on X11 and Wayland.
//!
//! Wayland keyboard access uses an explicit RemoteDesktop portal session. GNOME focus observation
//! uses the optional Kivori extension; media uses MPRIS on both session types.

mod apps;
mod audio;
mod foreground;
pub mod gnome;
mod gnome_focus;
pub mod host_events;
mod keys;
mod media;
mod mic;
pub mod portal_keys;
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
