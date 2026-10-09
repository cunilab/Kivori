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

pub mod foreground;
pub mod host_events;
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

pub trait InputSynth: Send + Sync {
    fn send_media_key(&self, key: MediaKey) -> Result<(), ActionError>;
    fn send_shortcut(&self, shortcut: &Shortcut) -> Result<(), ActionError>;
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
    fn send_media_key(&self, key: MediaKey) -> Result<(), ActionError> {
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
            synth: Arc::new(synth::EnigoInputSynth::new(main)),
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
