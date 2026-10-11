//! macOS backends (M1): CoreAudio master volume/mute and per-device microphone mute, Mach CPU/RAM counters, local time, and
//! now-playing observation (layered, see `media` and ADR-0009). Key synthesis lives in
//! `platform::synth`.

mod app_volume;
pub mod apps;
mod audio;
mod foreground;
pub mod host_events;
mod media;
mod mic;
mod system;

pub use app_volume::MacAppVolumeBackend;
pub use audio::MacVolumeBackend;
pub use foreground::MacForeground;
pub use media::MacMediaObserver;
pub use mic::MacMicBackend;
pub use system::{local_time, MacSystemProbe};
