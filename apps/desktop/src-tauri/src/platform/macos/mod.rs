//! macOS backends (M1): CoreAudio master volume/mute, Mach CPU/RAM counters, local time.
//!
//! Media observation is deliberately absent: macOS has no public API that lets an ordinary app
//! read system-wide now-playing state (`MPNowPlayingInfoCenter` only publishes the caller's own
//! playback; MediaRemote is private and entitlement-gated since macOS 15.4), so macOS uses
//! [`crate::platform::unimplemented::NoMediaObserver`]. Key synthesis lives in `platform::synth`.

mod audio;
mod system;

pub use audio::MacVolumeBackend;
pub use system::{local_time, MacSystemProbe};
