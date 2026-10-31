//! Platform capability boundary.
//!
//! Three orthogonal concepts, deliberately never collapsed (design spec section 6):
//!   * BackendAvailability — a fact about THIS Kivori build and its runtime;
//!   * ConfirmationClass   — how well an executed action's outcome can be observed;
//!   * ActionAvailability  — the derived value the UI consumes.
//!
//! A `PlatformCapability` type (a fact about the MACHINE) is intentionally absent: the one
//! machine fact the UI needs, "this OS cannot do it", is `ActionAvailability::Unsupported`,
//! decided by the action catalog (`desk::catalog`), not by a probe.

pub mod apps;
pub mod foreground;
pub mod host_events;
pub mod launch;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(any(windows, test))]
mod mic_coverage;
pub mod permissions;
pub mod png;
pub mod shortcut;
#[cfg(any(windows, target_os = "macos"))]
pub mod synth;
pub mod system;
pub mod unimplemented;
#[cfg(windows)]
pub mod windows;

use std::collections::VecDeque;
use std::sync::Mutex;

use kivori_model::desk::{ClockTime, MediaStatus, MicState};

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
    Available {
        confirmation: ConfirmationClass,
    },
    NotImplementedYet {
        target: &'static str,
    },
    /// This OS cannot do it (a machine fact, not a gap in Kivori): shown disabled with the reason.
    Unsupported {
        reason: String,
    },
    RuntimeUnavailable {
        reason: String,
    },
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
    /// `None` when the device's mute cannot be read (some devices have none): unknown, not false.
    pub muted: Option<bool>,
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

/// What the OS says about one capture device's mute, after a read (or a write and its read-back).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicMute {
    /// Read back muted.
    Muted,
    /// Read back unmuted.
    Live,
    /// The mute could not be read (the write may or may not have landed).
    Unreadable,
    /// The device has no settable mute at all (macOS: no `kAudioDevicePropertyMute` on the input
    /// scope, e.g. a phone's Continuity microphone). Kivori never substitutes input volume 0
    /// (invariant 19), so it counts as muted only while it is observed not capturing.
    Unsettable,
}

/// One active capture device, or an Unreadable entry for unresolved discovery coverage.
/// The name is user data: shown in the app, never logged (ADR-0005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicDevice {
    pub name: String,
    pub mute: MicMute,
    /// Whether some process is recording from this device: `Some(true)` capturing, `Some(false)`
    /// observed idle, `None` unknown (never read as idle). Only an `Unsettable` device's value
    /// matters for the muted state.
    pub capturing: Option<bool>,
}

/// Whether the microphones are muted: `Some(true)` only when at least one device reads muted and
/// every other one is muted or has no mute control while observed idle, `Some(false)` when any is
/// (or must be treated as) live, `None` when that cannot be told. A device with no mute control
/// that is capturing, or whose capture is unknown, is live. Never claims muted while any
/// microphone may be live.
#[must_use]
pub fn mics_muted(devices: &[MicDevice]) -> Option<bool> {
    let mut any_muted = false;
    let mut any_unreadable = false;
    for d in devices {
        match d.mute {
            MicMute::Muted => any_muted = true,
            MicMute::Live => return Some(false),
            MicMute::Unsettable if d.capturing != Some(false) => return Some(false),
            MicMute::Unsettable => {}
            MicMute::Unreadable => any_unreadable = true,
        }
    }
    if devices.is_empty() || any_unreadable {
        None
    } else {
        Some(any_muted)
    }
}

/// One change to the capture devices the backend observed: every value in it was read from the OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicChange {
    pub devices: Vec<MicDevice>,
    /// `External` for a user toggling mute in the OS, `EndpointRebind` when the set of devices
    /// changed (a device arrived or left), `Kivori` for the echo of its own write.
    pub origin: ChangeOrigin,
}

/// Every active capture device (the OS microphones), muted and read back one by one (ADR-0011).
pub trait MicBackend: Send + Sync {
    fn availability(&self) -> ActionAvailability;
    /// Every active capture device with its mute as the OS reports it now. An empty list is "no
    /// microphone", not "muted".
    fn devices(&self) -> Result<Vec<MicDevice>, BackendError>;
    /// Mutes (or unmutes) every active capture device, reads each back and returns what it read.
    /// While muted, a device that arrives later is muted and read back too; unmuting ends that.
    fn set_mute(&self, muted: bool) -> Result<Vec<MicDevice>, BackendError>;
    /// Whether an app is capturing audio right now, where the OS reports it; `None` = unknown
    /// (never "not in use"). A device that is only playing audio does not count.
    fn in_use(&self) -> Option<bool>;
    /// One pending change observed by the OS since the last call, if any. Never blocks.
    fn try_recv_change(&self) -> Option<MicChange> {
        None
    }
}

/// The wire state for what the backend reported: a mute that is `None` stays unknown.
#[must_use]
pub fn mic_state(devices: &[MicDevice], in_use: Option<bool>) -> MicState {
    MicState {
        muted: mics_muted(devices),
        in_use,
    }
}

/// Why App Volume and App Mute are disabled on macOS (shown by the catalog and the macOS stub).
pub const APP_VOLUME_UNSUPPORTED_ON_MAC: &str = "Per-app volume isn't available on macOS";

/// Why a per-app volume request could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppVolumeError {
    /// The app has no audio session on the default output right now (not running, or silent
    /// and never opened one). Not the same as unsupported.
    NoSession,
    /// The write landed but not every session's read-back could be observed or agreed.
    ReadBackUnavailable,
    Os(String),
}

/// Per-app volume (0..=100) and mute, on the sessions an app has on the default output device.
/// `app` is the lowercase foreground-style id (Windows: the executable file name, `spotify.exe`).
pub trait AppVolumeBackend: Send + Sync {
    fn availability(&self) -> ActionAvailability;
    fn read(&self, app: &str) -> Result<u8, AppVolumeError>;
    /// Apply to every session of the app and return the value observed afterwards. Same honesty
    /// rule as [`VolumeBackend::set`]: the sessions must agree, or it is `ReadBackUnavailable`.
    fn set(&self, app: &str, percent: u8) -> Result<u8, AppVolumeError>;
    fn read_mute(&self, app: &str) -> Result<bool, AppVolumeError>;
    fn set_mute(&self, app: &str, muted: bool) -> Result<bool, AppVolumeError>;
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
    /// Input began, but its completion could not be observed before the caller stopped waiting.
    Uncertain,
}

/// Synthesized key input: media keys and keyboard shortcuts. Ok means the input was dispatched;
/// its effect is never observable, so callers report it as Unverified.
/// A system media key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    PlayPause,
    Previous,
    Next,
}

/// Owned checks carried all the way to actual input dispatch, including main-thread queueing.
#[derive(Clone)]
pub struct InputGuard {
    pub deadline: Option<std::time::Instant>,
    pub current: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
}

impl InputGuard {
    pub fn check(&self) -> Result<(), ActionError> {
        if !(self.current)()
            || self
                .deadline
                .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            Err(ActionError::Failed("input is no longer current".into()))
        } else {
            Ok(())
        }
    }
}

pub trait InputSynth: Send + Sync {
    fn send_media_key(&self, key: MediaKey, guard: InputGuard) -> Result<(), ActionError>;
    fn send_shortcut(&self, shortcut: &Shortcut, guard: InputGuard) -> Result<(), ActionError>;
}

/// What is playing, as the OS (or player) reports it. Either part may be empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
}

/// Media playback observation. `None` means this OS gives Kivori no way to observe playback;
/// it is shown as unknown, never guessed.
pub trait MediaObserver: Send + Sync {
    fn status(&self) -> Option<MediaStatus>;
    /// Title and artist of what is playing, when observable. Never blocks (observers cache).
    fn now_playing(&self) -> Option<NowPlaying> {
        None
    }
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
        if let Some(muted) = change.muted {
            *self.muted.lock().expect("fake backend mutex") = muted;
        }
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

/// A scripted [`MicBackend`]: devices are registered up front, and each can be made to refuse
/// the write, to stay live after it, or to be unreadable.
#[derive(Debug)]
pub struct FakeMicBackend {
    devices: Mutex<Vec<(MicDevice, FakeMicQuirk)>>,
    availability: ActionAvailability,
    in_use: Mutex<Option<bool>>,
    hold_muted: Mutex<bool>,
    changes: Mutex<VecDeque<MicChange>>,
}

/// How a fake capture device answers a mute request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeMicQuirk {
    /// Mute works and reads back.
    Works,
    /// The write is accepted but the device keeps reading live.
    StaysLive,
    /// The write lands but the read-back fails.
    Unreadable,
    /// No mute property at all, capture state unknown.
    Unsettable,
    /// No mute property, observed not capturing.
    UnsettableIdle,
    /// No mute property, and some process is recording from it.
    UnsettableCapturing,
}

impl FakeMicQuirk {
    fn unsettable(self) -> bool {
        matches!(
            self,
            Self::Unsettable | Self::UnsettableIdle | Self::UnsettableCapturing
        )
    }

    fn capturing(self) -> Option<bool> {
        match self {
            Self::UnsettableIdle => Some(false),
            Self::UnsettableCapturing => Some(true),
            _ => None,
        }
    }
}

impl FakeMicBackend {
    pub fn new() -> Self {
        Self {
            devices: Mutex::new(Vec::new()),
            availability: ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            in_use: Mutex::new(None),
            hold_muted: Mutex::new(false),
            changes: Mutex::new(VecDeque::new()),
        }
    }

    pub fn with_availability(availability: ActionAvailability) -> Self {
        Self {
            availability,
            ..Self::new()
        }
    }

    /// A live device that behaves as `quirk` says.
    #[must_use]
    pub fn with_device(self, name: &str, quirk: FakeMicQuirk) -> Self {
        self.add_device(name, quirk);
        self
    }

    /// A device arrives: it is muted too while mic mute is on (and may fail to be).
    pub fn add_device(&self, name: &str, quirk: FakeMicQuirk) {
        let hold = *self.hold_muted.lock().expect("fake mic mutex");
        let mut device = MicDevice {
            name: name.to_string(),
            mute: if quirk.unsettable() {
                MicMute::Unsettable
            } else {
                MicMute::Live
            },
            capturing: quirk.capturing(),
        };
        if hold {
            device.mute = Self::applied(quirk, true, device.mute);
        }
        let mut devices = self.devices.lock().expect("fake mic mutex");
        devices.push((device, quirk));
        let snapshot = devices.iter().map(|(d, _)| d.clone()).collect();
        drop(devices);
        self.changes
            .lock()
            .expect("fake mic mutex")
            .push_back(MicChange {
                devices: snapshot,
                origin: ChangeOrigin::EndpointRebind,
            });
    }

    pub fn set_in_use(&self, in_use: Option<bool>) {
        *self.in_use.lock().expect("fake mic mutex") = in_use;
    }

    /// An app starts or stops recording from `name` (`None` = unknown); the change is observed.
    pub fn set_capturing(&self, name: &str, capturing: Option<bool>) {
        let mut devices = self.devices.lock().expect("fake mic mutex");
        for (device, _) in devices.iter_mut().filter(|(d, _)| d.name == name) {
            device.capturing = capturing;
        }
        let snapshot = devices.iter().map(|(d, _)| d.clone()).collect();
        drop(devices);
        self.changes
            .lock()
            .expect("fake mic mutex")
            .push_back(MicChange {
                devices: snapshot,
                origin: ChangeOrigin::External,
            });
    }

    /// The user mutes or unmutes `name` in the OS settings.
    pub fn external_mute(&self, name: &str, muted: bool) {
        let mut devices = self.devices.lock().expect("fake mic mutex");
        for (device, _) in devices.iter_mut().filter(|(d, _)| d.name == name) {
            device.mute = if muted { MicMute::Muted } else { MicMute::Live };
        }
        let snapshot = devices.iter().map(|(d, _)| d.clone()).collect();
        drop(devices);
        self.changes
            .lock()
            .expect("fake mic mutex")
            .push_back(MicChange {
                devices: snapshot,
                origin: ChangeOrigin::External,
            });
    }

    fn applied(quirk: FakeMicQuirk, muted: bool, before: MicMute) -> MicMute {
        match (quirk, muted) {
            (FakeMicQuirk::Works, true) => MicMute::Muted,
            (FakeMicQuirk::Works, false) => MicMute::Live,
            (FakeMicQuirk::StaysLive, _) => before,
            (FakeMicQuirk::Unreadable, _) => MicMute::Unreadable,
            (_, _) => MicMute::Unsettable,
        }
    }

    fn available(&self) -> Result<(), BackendError> {
        if matches!(self.availability, ActionAvailability::Available { .. }) {
            Ok(())
        } else {
            Err(BackendError::NoEndpoint)
        }
    }
}

impl Default for FakeMicBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MicBackend for FakeMicBackend {
    fn availability(&self) -> ActionAvailability {
        self.availability.clone()
    }

    fn devices(&self) -> Result<Vec<MicDevice>, BackendError> {
        self.available()?;
        Ok(self
            .devices
            .lock()
            .expect("fake mic mutex")
            .iter()
            .map(|(d, _)| d.clone())
            .collect())
    }

    fn set_mute(&self, muted: bool) -> Result<Vec<MicDevice>, BackendError> {
        self.available()?;
        *self.hold_muted.lock().expect("fake mic mutex") = muted;
        let mut devices = self.devices.lock().expect("fake mic mutex");
        if devices.is_empty() {
            return Err(BackendError::NoEndpoint);
        }
        for (device, quirk) in devices.iter_mut() {
            device.mute = Self::applied(*quirk, muted, device.mute);
        }
        Ok(devices.iter().map(|(d, _)| d.clone()).collect())
    }

    fn in_use(&self) -> Option<bool> {
        *self.in_use.lock().expect("fake mic mutex")
    }

    fn try_recv_change(&self) -> Option<MicChange> {
        self.changes.lock().expect("fake mic mutex").pop_front()
    }
}

/// A scripted [`AppVolumeBackend`]: apps with a session are registered up front.
#[derive(Debug)]
pub struct FakeAppVolumeBackend {
    sessions: Mutex<std::collections::HashMap<String, (u8, bool)>>,
    availability: ActionAvailability,
    readable_after_write: bool,
}

impl FakeAppVolumeBackend {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(std::collections::HashMap::new()),
            availability: ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            readable_after_write: true,
        }
    }

    pub fn with_availability(availability: ActionAvailability) -> Self {
        Self {
            availability,
            ..Self::new()
        }
    }

    pub fn unreadable_after_write() -> Self {
        Self {
            readable_after_write: false,
            ..Self::new()
        }
    }

    /// An app that has a session at `percent`, unmuted.
    #[must_use]
    pub fn with_session(self, app: &str, percent: u8) -> Self {
        self.sessions
            .lock()
            .expect("fake app volume mutex")
            .insert(app.to_string(), (percent, false));
        self
    }

    fn available(&self) -> Result<(), AppVolumeError> {
        if matches!(self.availability, ActionAvailability::Available { .. }) {
            Ok(())
        } else {
            Err(AppVolumeError::Os("app volume unavailable".to_string()))
        }
    }
}

impl Default for FakeAppVolumeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AppVolumeBackend for FakeAppVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        self.availability.clone()
    }

    fn read(&self, app: &str) -> Result<u8, AppVolumeError> {
        self.available()?;
        let sessions = self.sessions.lock().expect("fake app volume mutex");
        sessions
            .get(app)
            .map(|(percent, _)| *percent)
            .ok_or(AppVolumeError::NoSession)
    }

    fn set(&self, app: &str, percent: u8) -> Result<u8, AppVolumeError> {
        self.available()?;
        let mut sessions = self.sessions.lock().expect("fake app volume mutex");
        let session = sessions.get_mut(app).ok_or(AppVolumeError::NoSession)?;
        session.0 = percent;
        if !self.readable_after_write {
            return Err(AppVolumeError::ReadBackUnavailable);
        }
        Ok(percent)
    }

    fn read_mute(&self, app: &str) -> Result<bool, AppVolumeError> {
        self.available()?;
        let sessions = self.sessions.lock().expect("fake app volume mutex");
        sessions
            .get(app)
            .map(|(_, muted)| *muted)
            .ok_or(AppVolumeError::NoSession)
    }

    fn set_mute(&self, app: &str, muted: bool) -> Result<bool, AppVolumeError> {
        self.available()?;
        let mut sessions = self.sessions.lock().expect("fake app volume mutex");
        let session = sessions.get_mut(app).ok_or(AppVolumeError::NoSession)?;
        session.1 = muted;
        if !self.readable_after_write {
            return Err(AppVolumeError::ReadBackUnavailable);
        }
        Ok(muted)
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
    fn send_media_key(&self, key: MediaKey, guard: InputGuard) -> Result<(), ActionError> {
        guard.check()?;
        let name = match key {
            MediaKey::PlayPause => "media-play-pause",
            MediaKey::Previous => "media-previous",
            MediaKey::Next => "media-next",
        };
        self.sent
            .lock()
            .expect("fake synth mutex")
            .push(name.to_string());
        self.result.clone()
    }

    fn send_shortcut(&self, shortcut: &Shortcut, guard: InputGuard) -> Result<(), ActionError> {
        guard.check()?;
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
    pub now_playing: Mutex<Option<NowPlaying>>,
}

impl MediaObserver for FakeMediaObserver {
    fn status(&self) -> Option<MediaStatus> {
        *self.status.lock().expect("fake media mutex")
    }
    fn now_playing(&self) -> Option<NowPlaying> {
        self.now_playing.lock().expect("fake media mutex").clone()
    }
}

/// What has keyboard focus right now (docs/product.md, context and profiles).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Foreground {
    /// An ordinary app. `id` is the stable match key (Windows: lowercase executable file name such
    /// as `code.exe`; macOS: bundle identifier such as `com.microsoft.vscode`); `name` is for
    /// display.
    App { id: String, name: String },
    /// A protected surface (UAC / secure desktop, lock or login screen, an admin surface):
    /// custom actions are suspended and no profile applies, not even General (invariant 9).
    Protected,
    /// Focus can't be observed (no backend, or no focused window right now).
    Unknown,
}

/// Observes the focused app. Cheap enough to call a few times a second.
pub trait ForegroundObserver: Send + Sync {
    fn foreground(&self) -> Foreground;
}

/// A [`ForegroundObserver`] that reports whatever the test sets.
#[derive(Debug)]
pub struct FakeForeground(pub Mutex<Foreground>);

impl Default for FakeForeground {
    fn default() -> Self {
        Self(Mutex::new(Foreground::Unknown))
    }
}

impl ForegroundObserver for FakeForeground {
    fn foreground(&self) -> Foreground {
        self.0.lock().expect("fake foreground mutex").clone()
    }
}

/// Every OS service the device task uses, built for the current target. Targets without an
/// implementation get the honest "not implemented" / "not observable" variants.
pub struct OsServices {
    pub volume: std::sync::Arc<dyn VolumeBackend>,
    pub app_volume: std::sync::Arc<dyn AppVolumeBackend>,
    pub mic: std::sync::Arc<dyn MicBackend>,
    pub synth: std::sync::Arc<dyn InputSynth>,
    pub media: std::sync::Arc<dyn MediaObserver>,
    pub system: Box<dyn system::SystemProbe>,
    pub clock: LocalClock,
    pub foreground: std::sync::Arc<dyn ForegroundObserver>,
}

/// Runs a closure on the app's main thread (Tauri's `run_on_main_thread`).
pub type MainThread = std::sync::Arc<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;

/// Builds [`OsServices`] for this OS. Spawns the audio (and, on Windows, media) threads.
///
/// `main` is how macOS key synthesis reaches the main thread: since macOS 15 the keyboard-layout
/// lookups it needs assert they run there and kill the process otherwise. `None` runs input on the
/// calling thread (fine on Windows; on macOS only for code that never synthesizes input).
#[must_use]
pub fn os_services(main: Option<MainThread>) -> OsServices {
    use std::sync::Arc;
    #[cfg(windows)]
    {
        // One backend: app sessions are read and written on the same COM-owning audio thread.
        let audio = Arc::new(windows::WindowsVolumeBackend::new());
        OsServices {
            volume: audio.clone(),
            app_volume: audio,
            mic: Arc::new(windows::WindowsMicBackend::new()),
            synth: Arc::new(synth::EnigoInputSynth::new(main)),
            media: Arc::new(windows::WindowsMediaObserver::new()),
            system: Box::new(windows::WindowsSystemProbe),
            clock: windows::local_time,
            foreground: Arc::new(windows::WindowsForeground::new()),
        }
    }
    #[cfg(target_os = "macos")]
    {
        OsServices {
            volume: Arc::new(macos::MacVolumeBackend::new()),
            app_volume: Arc::new(macos::MacAppVolumeBackend),
            mic: Arc::new(macos::MacMicBackend::new()),
            synth: Arc::new(macos::MacInputSynth::new(Arc::new(
                synth::EnigoInputSynth::new(main),
            ))),
            media: Arc::new(macos::MacMediaObserver::new()),
            system: Box::new(macos::MacSystemProbe),
            clock: macos::local_time,
            foreground: Arc::new(macos::MacForeground),
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let target = std::env::consts::OS;
        let _ = main;
        OsServices {
            volume: Arc::new(unimplemented::UnimplementedVolumeBackend::new(target)),
            app_volume: Arc::new(unimplemented::UnimplementedAppVolumeBackend::new(target)),
            mic: Arc::new(unimplemented::UnimplementedMicBackend::new(target)),
            synth: Arc::new(unimplemented::UnimplementedInputSynth { target }),
            media: Arc::new(unimplemented::NoMediaObserver),
            system: Box::new(system::NoSystemProbe),
            clock: || None,
            foreground: Arc::new(unimplemented::NoForeground),
        }
    }
}

/// Starts the sleep/wake and lock/unlock source for this OS. Signals flow to the device thread
/// over `tx`; dropping the guard stops the source.
///
/// # Errors
/// [`host_events::HostEventsError::Unavailable`] when the OS refuses the registration.
pub fn start_host_events(
    tx: std::sync::mpsc::Sender<host_events::HostSignal>,
) -> Result<host_events::HostEventsGuard, host_events::HostEventsError> {
    #[cfg(windows)]
    {
        windows::host_events::start(tx)
    }
    #[cfg(target_os = "macos")]
    {
        macos::host_events::start(tx)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        unimplemented::start_host_events(tx)
    }
}
