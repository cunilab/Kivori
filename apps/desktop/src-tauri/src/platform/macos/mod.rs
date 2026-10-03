//! macOS backends (M1): CoreAudio master volume/mute, Mach CPU/RAM counters, local time, and
//! now-playing observation (layered, see `media` and ADR-0009). Key synthesis lives in
//! `platform::synth`.

mod audio;
mod media;
mod system;

pub use audio::MacVolumeBackend;
pub use media::MacMediaObserver;
pub use system::{local_time, MacSystemProbe};
