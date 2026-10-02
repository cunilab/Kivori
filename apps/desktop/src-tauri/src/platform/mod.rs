//! Platform capability boundary.
//!
//! Three orthogonal concepts, deliberately never collapsed (design spec section 6):
//!   * BackendAvailability — a fact about THIS Kivori build and its runtime;
//!   * ConfirmationClass   — how well an executed action's outcome can be observed;
//!   * ActionAvailability  — the derived value the UI consumes.
//!
//! A `PlatformCapability` type (a fact about the MACHINE) is intentionally absent:
//! every action in Slice 002 is one the OS can perform, so such a type would have
//! no producer. It arrives with the first genuinely OS-restricted action.

pub mod launch;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod shortcut;
#[cfg(any(windows, target_os = "macos"))]
pub mod synth;
pub mod system;
pub mod unimplemented;
#[cfg(windows)]
pub mod windows;

use std::collections::VecDeque;
use std::sync::Mutex;

use kivori_model::desk::{ClockTime, MediaStatus};

pub use shortcut::{Shortcut, ShortcutKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendAvailability {
    Available,
    /// Kivori has no backend compiled for this OS. NOT the same as unsupported.
    NotImplemented {
        target: &'static str,
    },
    /// Implemented, but a runtime dependency is missing right now.
    RuntimeUnavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationClass {
    StateConfirmed,
    ExecutionConfirmed,
    TriggeredUnverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionAvailability {
    Available { confirmation: ConfirmationClass },
    NotImplementedYet { target: &'static str },
    RuntimeUnavailable { reason: String },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// No default render endpoint exists.
    NoEndpoint,
    /// The write was accepted but the resulting state could not be read back.
    ReadBackUnavailable,
    Os(String),
}

/// Where an observed audio change came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOrigin {
    /// The echo of a write Kivori itself just performed. `set()`/`set_mute()` already confirmed
    /// it synchronously via their own read-back, so this is informational only.
    Kivori,
    /// A change Kivori did not originate: the OS flyout, a media key, another app.
    External,
    /// The default output device changed; these are the new device's freshly read values.
    EndpointRebind,
}

/// One audio change the backend observed. Every value in it was read from the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeChange {
    pub percent: u8,
    pub muted: bool,
    pub origin: ChangeOrigin,
}

/// Master output volume, 0..=100, and master mute, on the OS default output device.
pub trait VolumeBackend: Send + Sync {
    fn availability(&self) -> ActionAvailability;
    fn read(&self) -> Result<u8, BackendError>;
    /// Apply a value and return the value actually observed afterwards.
    ///
    /// Returning the read-back rather than `()` is what makes StateConfirmed
    /// structurally honest: the confirmed value comes from the OS, never the request.
    fn set(&self, percent: u8) -> Result<u8, BackendError>;
    /// Master mute as the OS reports it.
    fn read_mute(&self) -> Result<bool, BackendError>;
    /// Apply mute and return the mute state actually observed afterwards (same honesty rule as
    /// [`VolumeBackend::set`]).
    fn set_mute(&self, muted: bool) -> Result<bool, BackendError>;
    /// One pending change observed by the OS since the last call, if any. Never blocks. Backends
    /// that cannot observe changes return `None`.
    fn try_recv_change(&self) -> Option<VolumeChange> {
        None
    }
}

/// Why a desktop action could not run. Never silently replaced by another mechanism
/// (invariant 19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    /// The OS needs a permission Kivori does not have yet (macOS Accessibility). Not an error
    /// state of Kivori, and not retried in a loop.
    PermissionRequired,
    /// Kivori has no implementation of this action for this OS.
    NotImplemented { target: &'static str },
    /// The OS refused or failed the request.
    Failed(String),
}

/// Synthesized key input: media keys and keyboard shortcuts. Ok means the input was dispatched;
/// its effect is never observable, so callers report it as Unverified.
pub trait InputSynth: Send + Sync {
    fn send_media_play_pause(&self) -> Result<(), ActionError>;
    fn send_shortcut(&self, shortcut: &Shortcut) -> Result<(), ActionError>;
}

/// Media playback observation. `None` means this OS gives Kivori no way to observe playback;
/// it is shown as unknown, never guessed.
pub trait MediaObserver: Send + Sync {
    fn status(&self) -> Option<MediaStatus>;
}

/// The local wall-clock time of day, or `None` if it cannot be read.
pub type LocalClock = fn() -> Option<ClockTime>;

#[derive(Debug)]
pub struct FakeVolumeBackend {
    state: Mutex<u8>,
    muted: Mutex<bool>,
    changes: Mutex<VecDeque<VolumeChange>>,
    availability: ActionAvailability,
    quantise_step: Option<u8>,
    readable_after_write: bool,
}

impl FakeVolumeBackend {
    pub fn new(initial: u8) -> Self {
        Self {
            state: Mutex::new(initial),
            muted: Mutex::new(false),
            changes: Mutex::new(VecDeque::new()),
            availability: ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            quantise_step: None,
            readable_after_write: true,
        }
    }

    /// Models a device whose driver snaps to coarse steps.
    pub fn quantised(initial: u8, step: u8) -> Self {
        Self {
            quantise_step: Some(step),
            ..Self::new(initial)
        }
    }

    pub fn with_availability(availability: ActionAvailability) -> Self {
        Self {
            availability,
            ..Self::new(0)
        }
    }

    pub fn unreadable_after_write(initial: u8) -> Self {
        Self {
            readable_after_write: false,
            ..Self::new(initial)
        }
    }

    /// Simulates a change made outside Kivori (the Windows flyout, a media key).
    pub fn external_change(&self, percent: u8) {
        *self.state.lock().expect("fake backend mutex") = percent;
    }

    /// Simulates an observed external change, delivered through `try_recv_change`.
    pub fn push_change(&self, change: VolumeChange) {
        *self.state.lock().expect("fake backend mutex") = change.percent;
        *self.muted.lock().expect("fake backend mutex") = change.muted;
        self.changes
            .lock()
            .expect("fake backend mutex")
            .push_back(change);
    }
}

impl VolumeBackend for FakeVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        self.availability.clone()
    }

    fn read(&self) -> Result<u8, BackendError> {
        if !matches!(self.availability, ActionAvailability::Available { .. }) {
            return Err(BackendError::NoEndpoint);
        }
        Ok(*self.state.lock().expect("fake backend mutex"))
    }

    fn set(&self, percent: u8) -> Result<u8, BackendError> {
        if !matches!(self.availability, ActionAvailability::Available { .. }) {
            return Err(BackendError::NoEndpoint);
        }
        let applied = match self.quantise_step {
            Some(step) if step > 0 => (percent / step) * step,
            _ => percent,
        };
        *self.state.lock().expect("fake backend mutex") = applied;
        if !self.readable_after_write {
            return Err(BackendError::ReadBackUnavailable);
        }
        Ok(applied)
    }

    fn read_mute(&self) -> Result<bool, BackendError> {
        if !matches!(self.availability, ActionAvailability::Available { .. }) {
            return Err(BackendError::NoEndpoint);
        }
        Ok(*self.muted.lock().expect("fake backend mutex"))
    }

    fn set_mute(&self, muted: bool) -> Result<bool, BackendError> {
        if !matches!(self.availability, ActionAvailability::Available { .. }) {
            return Err(BackendError::NoEndpoint);
        }
        *self.muted.lock().expect("fake backend mutex") = muted;
        if !self.readable_after_write {
            return Err(BackendError::ReadBackUnavailable);
        }
        Ok(muted)
    }

    fn try_recv_change(&self) -> Option<VolumeChange> {
        self.changes.lock().expect("fake backend mutex").pop_front()
    }
}

/// A scripted [`InputSynth`]: records what it was asked to send and returns a fixed result.
#[derive(Debug)]
pub struct FakeInputSynth {
    pub result: Result<(), ActionError>,
    pub sent: Mutex<Vec<String>>,
}

impl FakeInputSynth {
    pub fn new(result: Result<(), ActionError>) -> Self {
        Self {
            result,
            sent: Mutex::new(Vec::new()),
        }
    }
}

impl InputSynth for FakeInputSynth {
    fn send_media_play_pause(&self) -> Result<(), ActionError> {
        self.sent
            .lock()
            .expect("fake synth mutex")
            .push("media-play-pause".to_string());
        self.result.clone()
    }

    fn send_shortcut(&self, shortcut: &Shortcut) -> Result<(), ActionError> {
        self.sent
            .lock()
            .expect("fake synth mutex")
            .push(shortcut.to_string());
        self.result.clone()
    }
}

/// A [`MediaObserver`] that reports whatever the test sets.
#[derive(Debug, Default)]
pub struct FakeMediaObserver {
    pub status: Mutex<Option<MediaStatus>>,
}

impl MediaObserver for FakeMediaObserver {
    fn status(&self) -> Option<MediaStatus> {
        *self.status.lock().expect("fake media mutex")
    }
}

/// Every OS service the device task uses, built for the current target. Targets without an
/// implementation get the honest "not implemented" / "not observable" variants.
pub struct OsServices {
    pub volume: std::sync::Arc<dyn VolumeBackend>,
    pub synth: std::sync::Arc<dyn InputSynth>,
    pub media: std::sync::Arc<dyn MediaObserver>,
    pub system: Box<dyn system::SystemProbe>,
    pub clock: LocalClock,
}

/// Builds [`OsServices`] for this OS. Spawns the audio (and, on Windows, media) threads.
#[must_use]
pub fn os_services() -> OsServices {
    use std::sync::Arc;
    #[cfg(windows)]
    {
        OsServices {
            volume: Arc::new(windows::WindowsVolumeBackend::new()),
            synth: Arc::new(synth::EnigoInputSynth),
            media: Arc::new(windows::WindowsMediaObserver::new()),
            system: Box::new(windows::WindowsSystemProbe),
            clock: windows::local_time,
        }
    }
    #[cfg(target_os = "macos")]
    {
        OsServices {
            volume: Arc::new(macos::MacVolumeBackend::new()),
            synth: Arc::new(synth::EnigoInputSynth),
            media: Arc::new(unimplemented::NoMediaObserver),
            system: Box::new(macos::MacSystemProbe),
            clock: macos::local_time,
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let target = std::env::consts::OS;
        OsServices {
            volume: Arc::new(unimplemented::UnimplementedVolumeBackend::new(target)),
            synth: Arc::new(unimplemented::UnimplementedInputSynth { target }),
            media: Arc::new(unimplemented::NoMediaObserver),
            system: Box::new(system::NoSystemProbe),
            clock: || None,
        }
    }
}
