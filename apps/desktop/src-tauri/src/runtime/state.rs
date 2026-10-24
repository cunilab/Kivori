//! Tauri-managed application state.
//!
//! `AppState` owns the shared snapshot, the session activity ring, the window-lifecycle policy, and the
//! handles to the background device thread (command channel + cancellation + join handle). The device
//! thread — not React, not the webview — owns the mutable connection state and the serial link; this
//! type only exposes a read snapshot + a command channel to it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use kivori_model::{MascotAction, SendableState};

use crate::activity::{ActivityEvent, ActivityEventKind, ActivityLog};
use crate::config::{ConfigStore, ResolvedConfig};
use crate::desk::catalog::ServicesCell;
use crate::firmware::{self, FirmwarePhase, FirmwareStatus};
use crate::firmware_offer::OfferTracker;
use crate::ipc::dto::{
    self, ConfigDto, ConnectionStatusDto, DeskStatusDto, DiagnosticsDto, DiagnosticsSnapshot,
    FirmwareOfferDto,
};
use crate::platform;
use crate::platform::host_events::{HostEventsGuard, HostSignal};
use crate::window_lifecycle::WindowLifecycle;

/// A message from a Tauri command (UI thread) to the background device thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceCommand {
    /// Set the desired sendable state; transmitted to the device when connected (FR-012).
    SetDesired(SendableState),
    /// Dev-only state request, kept distinct in native activity even though its wire state is identical.
    MirrorDesired(SendableState),
    /// Adopt a saved config: the buddy's intensity and reactions, and the display views.
    ApplyConfig(Arc<ResolvedConfig>),
    /// Play one immediate social reaction using current companion settings.
    PlayMascotAction(MascotAction),
    /// Flash the fixed firmware image embedded in this desktop build.
    FlashFirmware,
    /// Restore the bundled firmware on the one Kivori device present, even one that cannot
    /// handshake (wrong-major firmware, or a unit held in ROM download mode).
    RestoreFirmware,
    /// The same restore, for an explicit product and optional board chosen by the user.
    RestoreFirmwareFor(String, Option<String>),
    /// Re-publish the current status (used by an explicit UI resync).
    Refresh,
    /// Ask the device for one internals snapshot (developer mode, while the card is visible).
    RequestInternals,
    /// Show another full-screen view on the device.
    SetDisplayMode(kivori_model::desk::DisplayMode),
    /// Try one desk action now, exactly as if its control fired (the Test button). A protected
    /// foreground still suspends everything but system actions.
    TestAction(crate::desk::Action),
}

/// Keeps the OS sleep/wake and lock source running for the life of the app (managed by Tauri, never
/// read). If the OS refuses the registration the app still works; the device just never hears
/// "goodnight" and falls back to its host-silence timeout.
pub struct HostEventsHandle {
    _guard: Mutex<Option<HostEventsGuard>>,
}

impl HostEventsHandle {
    /// Starts the source for this OS, sending signals to the device thread over `tx`.
    #[must_use]
    pub fn start(tx: Sender<HostSignal>) -> Self {
        let guard = match platform::start_host_events(tx) {
            Ok(guard) => Some(guard),
            Err(error) => {
                tracing::warn!(%error, "host presence events are off");
                None
            }
        };
        Self {
            _guard: Mutex::new(guard),
        }
    }
}

/// Tauri-managed application state (`Send + Sync`, accessed via `State<'_, AppState>`).
pub struct AppState {
    /// Whether the dev-only Device Studio commands are compiled in (false in release; FR-028).
    pub device_studio_enabled: bool,
    /// Latest projected connection snapshot — written by the device thread, read by commands/events.
    pub status: Arc<Mutex<ConnectionStatusDto>>,
    /// Latest desk projection (mode, monitoring, last action), written by the device thread.
    pub desk_status: Arc<Mutex<DeskStatusDto>>,
    /// Session-only typed activity ring.
    pub activity_log: Arc<ActivityLog>,
    /// The persisted user config. Commands save through it, then tell the device thread.
    pub config: Arc<Mutex<ConfigStore>>,
    /// Latest safe firmware-update status, written only by the device thread.
    pub firmware_status: Arc<Mutex<FirmwareStatus>>,
    /// The firmware update offer: decided by the device thread each tick, answered by commands.
    pub firmware_offer: Arc<Mutex<OfferTracker>>,
    /// Latest raw diagnostics figures, written by the device thread about once a second.
    pub diagnostics: Arc<Mutex<DiagnosticsSnapshot>>,
    /// Latest host-service availability for the action catalog, written by the device thread.
    pub services: ServicesCell,
    /// Window-lifecycle policy (hide-vs-quit / show-on-reactivate), shared with the window+tray handlers.
    pub lifecycle: Mutex<WindowLifecycle>,
    /// Live Device Studio preview streams (dev-only), cancelled on shutdown/teardown.
    #[cfg(feature = "device-studio")]
    pub previews: Arc<crate::ipc::channels::PreviewStreams>,
    commands: Mutex<Sender<DeviceCommand>>,
    // Lock order: installation admission, then firmware status. Never held across device shutdown.
    app_installing: Mutex<bool>,
    cancel: Arc<AtomicBool>,
    device_thread: Mutex<Option<JoinHandle<()>>>,
}

/// Keeps firmware admission closed until the installer returns or exits the process.
#[must_use]
pub struct AppInstallGuard<'a> {
    state: &'a AppState,
}

impl Drop for AppInstallGuard<'_> {
    fn drop(&mut self) {
        *self.state.app_installing.lock().expect("installation lock") = false;
    }
}

impl AppState {
    /// Wires managed state around an already-spawned device thread's handles.
    #[must_use]
    pub fn new(
        device_studio_enabled: bool,
        status: Arc<Mutex<ConnectionStatusDto>>,
        activity_log: Arc<ActivityLog>,
        commands: Sender<DeviceCommand>,
        cancel: Arc<AtomicBool>,
        device_thread: JoinHandle<()>,
    ) -> Self {
        Self::new_with_firmware(
            device_studio_enabled,
            status,
            activity_log,
            Arc::new(Mutex::new(firmware::initial_status())),
            commands,
            cancel,
            device_thread,
        )
    }

    /// Wires managed state with the firmware-status cell shared with the device thread.
    #[must_use]
    pub fn new_with_firmware(
        device_studio_enabled: bool,
        status: Arc<Mutex<ConnectionStatusDto>>,
        activity_log: Arc<ActivityLog>,
        firmware_status: Arc<Mutex<FirmwareStatus>>,
        commands: Sender<DeviceCommand>,
        cancel: Arc<AtomicBool>,
        device_thread: JoinHandle<()>,
    ) -> Self {
        Self {
            device_studio_enabled,
            status,
            desk_status: Arc::new(Mutex::new(crate::ipc::dto::initial_desk_status(
                &ResolvedConfig::default(),
            ))),
            activity_log,
            config: Arc::new(Mutex::new(ConfigStore::detached())),
            firmware_status,
            firmware_offer: Arc::new(Mutex::new(OfferTracker::new())),
            diagnostics: Arc::new(Mutex::new(DiagnosticsSnapshot::initial())),
            services: ServicesCell::default(),
            lifecycle: Mutex::new(WindowLifecycle::new()),
            #[cfg(feature = "device-studio")]
            previews: Arc::new(crate::ipc::channels::PreviewStreams::new()),
            commands: Mutex::new(commands),
            app_installing: Mutex::new(false),
            cancel,
            device_thread: Mutex::new(Some(device_thread)),
        }
    }

    /// Shares the desk projection cell the device thread writes.
    #[must_use]
    pub fn with_desk_status(mut self, desk_status: Arc<Mutex<DeskStatusDto>>) -> Self {
        self.desk_status = desk_status;
        self
    }

    /// Shares the diagnostics cell the device thread writes.
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: Arc<Mutex<DiagnosticsSnapshot>>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Shares the firmware offer tracker the device thread evaluates.
    #[must_use]
    pub fn with_firmware_offer(mut self, firmware_offer: Arc<Mutex<OfferTracker>>) -> Self {
        self.firmware_offer = firmware_offer;
        self
    }

    /// Shares the services cell the device thread writes.
    #[must_use]
    pub fn with_services(mut self, services: ServicesCell) -> Self {
        self.services = services;
        self
    }

    /// Shares the config store the setup code opened.
    #[must_use]
    pub fn with_config(mut self, config: Arc<Mutex<ConfigStore>>) -> Self {
        self.config = config;
        self
    }

    /// The stored config as the UI shows it.
    #[must_use]
    pub fn config_snapshot(&self) -> ConfigDto {
        dto::config_dto(&self.config.lock().expect("config lock"))
    }

    /// Saves a config change and, only if that worked, tells the device thread to apply it.
    ///
    /// `change` edits a copy of the file and stores it (or resets it). A failure leaves both the
    /// stored config and the running device untouched. Returns the outcome and the activity record
    /// of it (`saved` on success, `ConfigSaveFailed` otherwise) for the caller to emit.
    pub fn update_config(
        &self,
        saved: ActivityEventKind,
        change: impl FnOnce(&mut ConfigStore) -> Result<(), crate::config::ConfigError>,
    ) -> (Result<ConfigDto, String>, ActivityEvent) {
        let mut store = self.config.lock().expect("config lock");
        match change(&mut store) {
            Ok(()) => {
                let config = dto::config_dto(&store);
                // The device thread stops only at shutdown, when there is nothing left to apply to.
                let _ = self.send_command(DeviceCommand::ApplyConfig(store.resolved()));
                (Ok(config), self.activity_log.record(saved, None))
            }
            Err(error) => (
                Err(error.to_string()),
                self.activity_log
                    .record(ActivityEventKind::ConfigSaveFailed, None),
            ),
        }
    }

    /// Whether first-run setup is finished.
    #[must_use]
    pub fn onboarding_snapshot(&self) -> dto::OnboardingDto {
        dto::OnboardingDto {
            completed: self
                .config
                .lock()
                .expect("config lock")
                .file()
                .onboarding
                .completed,
        }
    }

    /// The diagnostics page as of now: the device thread's figures plus the stored config.
    #[must_use]
    pub fn diagnostics_snapshot(&self) -> DiagnosticsDto {
        let snapshot = self.diagnostics.lock().expect("diagnostics lock").clone();
        let config = self.config.lock().expect("config lock");
        dto::diagnostics_dto(&snapshot, &config, std::time::Instant::now())
    }

    /// The Device internals card as of now: the latest snapshot the device thread cached.
    #[must_use]
    pub fn internals_snapshot(&self) -> dto::DeviceInternalsDto {
        let snapshot = self.diagnostics.lock().expect("diagnostics lock").clone();
        dto::device_internals_dto(&snapshot, std::time::Instant::now())
    }

    /// The current desk projection.
    #[must_use]
    pub fn desk_snapshot(&self) -> DeskStatusDto {
        self.desk_status.lock().expect("desk status lock").clone()
    }

    /// The current connection snapshot (the initial-sync command reads this so UI correctness does not
    /// depend on event-subscription timing).
    #[must_use]
    pub fn status_snapshot(&self) -> ConnectionStatusDto {
        self.status.lock().expect("status lock").clone()
    }

    /// The current safe firmware-update projection for the Overview UI.
    #[must_use]
    pub fn firmware_status_snapshot(&self) -> FirmwareStatus {
        self.firmware_status
            .lock()
            .expect("firmware status lock")
            .clone()
    }

    /// The firmware update offer as the dialog, the Device page and the tray show it.
    #[must_use]
    pub fn firmware_offer_snapshot(&self) -> FirmwareOfferDto {
        let product = self
            .status_snapshot()
            .device
            .and_then(|device| device.identity)
            .map(|identity| identity.product);
        let tracker = self.firmware_offer.lock().expect("firmware offer lock");
        dto::firmware_offer_dto(tracker.current(), product)
    }

    /// "Update now" on the firmware offer: the ordinary flash of the bundled image (an
    /// incompatible device takes the restore path, as with "Flash firmware").
    ///
    /// # Errors
    /// When nothing is offered, or the flash cannot be queued.
    pub fn accept_firmware_offer(&self) -> Result<ActivityEvent, String> {
        self.firmware_offer
            .lock()
            .expect("firmware offer lock")
            .accept()
            .map_err(|refused| refused.message().to_string())?;
        self.queue_firmware_flash()?;
        Ok(self
            .activity_log
            .record(ActivityEventKind::FirmwareOfferAccepted, None))
    }

    /// "Later" on an optional firmware offer: asked again when the device next connects.
    ///
    /// # Errors
    /// When no optional offer is shown.
    pub fn postpone_firmware_offer(&self) -> Result<ActivityEvent, String> {
        self.firmware_offer
            .lock()
            .expect("firmware offer lock")
            .later()
            .map_err(|refused| refused.message().to_string())?;
        Ok(self
            .activity_log
            .record(ActivityEventKind::FirmwareOfferLater, None))
    }

    /// Whether a requested update owns the device session. State changes must not be queued behind it.
    #[must_use]
    pub fn firmware_busy(&self) -> bool {
        matches!(
            self.firmware_status
                .lock()
                .expect("firmware status lock")
                .phase,
            FirmwarePhase::Preparing | FirmwarePhase::Flashing | FirmwarePhase::Reconnecting
        )
    }

    /// Atomically reserves the update workflow before it is queued, preventing concurrent IPC calls
    /// from both accepting an idle device. The device thread revalidates its private port and identity.
    /// A connected device or an incompatible one (which is recovered) may be flashed.
    pub fn queue_firmware_flash(&self) -> Result<(), String> {
        if !matches!(
            self.status_snapshot().connection.as_str(),
            "connected" | "incompatible"
        ) {
            return Err("Connect a compatible device before flashing firmware.".to_string());
        }
        self.reserve_and_queue(DeviceCommand::FlashFirmware)
    }

    /// Queues recovery of the one Kivori device present, whatever its connection state. The device
    /// thread refuses unless exactly one allowlisted port exists.
    pub fn queue_firmware_restore(&self) -> Result<(), String> {
        self.reserve_and_queue(DeviceCommand::RestoreFirmware)
    }

    /// [`Self::queue_firmware_restore`] for the product the user picked (a machine id from the
    /// status's restore choices).
    pub fn queue_firmware_restore_for(
        &self,
        product: String,
        board: Option<String>,
    ) -> Result<(), String> {
        let valid = |id: &str| {
            !id.is_empty()
                && id.len() <= 64
                && id.split('-').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                })
        };
        if !valid(&product) || board.as_deref().is_some_and(|id| !valid(id)) {
            return Err(
                "Choose a valid catalog product and board before restoring firmware.".into(),
            );
        }
        self.reserve_and_queue(DeviceCommand::RestoreFirmwareFor(product, board))
    }

    fn reserve_and_queue(&self, command: DeviceCommand) -> Result<(), String> {
        let installing = self.app_installing.lock().expect("installation lock");
        if *installing {
            return Err("An app update is being installed. Try again after restarting.".into());
        }
        let previous = {
            let mut status = self.firmware_status.lock().expect("firmware status lock");
            if !status.available {
                return Err("Firmware is unavailable in this application build.".to_string());
            }
            if matches!(
                status.phase,
                FirmwarePhase::Preparing | FirmwarePhase::Flashing | FirmwarePhase::Reconnecting
            ) {
                return Err("A firmware update is already in progress.".to_string());
            }
            let previous = status.clone();
            status.phase = FirmwarePhase::Preparing;
            status.image_size = 0;
            status.bundled_version = None;
            status.advice = firmware::UpdateAdvice::Unknown;
            status.message = "Preparing firmware update.".to_string();
            status.failure = None;
            status.wrong_product = None;
            status.restore_choices.clear();
            previous
        };
        if let Err(error) = self.send_command(command) {
            *self.firmware_status.lock().expect("firmware status lock") = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Reserves installer launch against every flash/restore admission, including queued Preparing.
    /// Dropping the guard releases the reservation after an installation failure.
    #[must_use]
    pub fn try_reserve_app_install(&self) -> Option<AppInstallGuard<'_>> {
        let mut installing = self.app_installing.lock().expect("installation lock");
        if *installing || self.firmware_busy() {
            return None;
        }
        *installing = true;
        Some(AppInstallGuard { state: self })
    }

    /// The updater exit hook may stop the runtime only while installation owns admission.
    #[must_use]
    pub fn shutdown_for_app_install(&self) -> bool {
        if !*self.app_installing.lock().expect("installation lock") {
            return false;
        }
        self.shutdown();
        true
    }

    /// Sends a command to the device thread.
    ///
    /// # Errors
    /// Returns an error string if the device thread has stopped.
    pub fn send_command(&self, command: DeviceCommand) -> Result<(), String> {
        self.commands
            .lock()
            .expect("commands lock")
            .send(command)
            .map_err(|_| "device runtime is not available".to_string())
    }

    /// Signals the device thread to stop and waits for it (the explicit-quit shutdown path). Reads are
    /// non-blocking and the loop ticks every ~50 ms, so the join returns promptly.
    pub fn shutdown(&self) {
        // Stop any preview producer threads first so they cannot outlive the runtime.
        #[cfg(feature = "device-studio")]
        self.previews.cancel_all();
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(handle) = self.device_thread.lock().expect("thread lock").take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod installation_tests {
    use super::*;
    use std::sync::mpsc::{self, Receiver};

    fn state() -> (AppState, Receiver<DeviceCommand>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&cancel);
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                std::thread::park_timeout(std::time::Duration::from_millis(1));
            }
        });
        let mut status = dto::initial_status();
        status.connection = "connected".into();
        let state = AppState::new(
            false,
            Arc::new(Mutex::new(status)),
            Arc::new(ActivityLog::new(8)),
            tx,
            Arc::clone(&cancel),
            thread,
        );
        state.firmware_status.lock().unwrap().available = true;
        (state, rx, cancel)
    }

    #[test]
    fn installation_admitted_first_refuses_every_firmware_path() {
        let (state, rx, _) = state();
        let reservation = state.try_reserve_app_install().unwrap();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                assert!(state.queue_firmware_flash().is_err());
                assert!(state.queue_firmware_restore().is_err());
                assert!(state
                    .queue_firmware_restore_for("pala".into(), None)
                    .is_err());
            });
        });
        assert!(rx.try_recv().is_err());
        assert_eq!(state.firmware_status_snapshot().phase, FirmwarePhase::Idle);
        assert!(state.try_reserve_app_install().is_none());
        drop(reservation);
        state.shutdown();
    }

    #[test]
    fn firmware_admitted_first_refuses_installation_and_updater_shutdown() {
        for command in [
            DeviceCommand::FlashFirmware,
            DeviceCommand::RestoreFirmware,
            DeviceCommand::RestoreFirmwareFor("pala".into(), None),
        ] {
            let (state, rx, cancel) = state();
            state.reserve_and_queue(command.clone()).unwrap();
            assert_eq!(rx.recv().unwrap(), command);
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    assert!(state.try_reserve_app_install().is_none());
                    assert!(!state.shutdown_for_app_install());
                });
            });
            assert!(!cancel.load(Ordering::SeqCst));
            assert_eq!(
                state.firmware_status_snapshot().phase,
                FirmwarePhase::Preparing
            );
            state.shutdown();
        }
    }

    #[test]
    fn installation_waits_for_all_busy_firmware_phases() {
        let (state, _rx, _) = state();
        for phase in [
            FirmwarePhase::Preparing,
            FirmwarePhase::Flashing,
            FirmwarePhase::Reconnecting,
        ] {
            state.firmware_status.lock().unwrap().phase = phase;
            assert!(state.try_reserve_app_install().is_none());
        }
        for phase in [FirmwarePhase::Succeeded, FirmwarePhase::Failed] {
            state.firmware_status.lock().unwrap().phase = phase;
            assert!(state.try_reserve_app_install().is_some());
        }
        state.shutdown();
    }

    #[test]
    fn failed_installer_releases_firmware_admission() {
        let (state, rx, _) = state();
        let install = || -> Result<(), ()> {
            let _reservation = state.try_reserve_app_install().unwrap();
            Err(())
        };
        assert!(install().is_err());
        state.queue_firmware_flash().unwrap();
        assert_eq!(rx.recv().unwrap(), DeviceCommand::FlashFirmware);
        state.shutdown();
    }

    #[test]
    fn failed_firmware_queue_releases_installation_admission() {
        let (state, rx, _) = state();
        drop(rx);
        assert!(state.queue_firmware_restore().is_err());
        assert_eq!(state.firmware_status_snapshot().phase, FirmwarePhase::Idle);
        assert!(state.try_reserve_app_install().is_some());
        state.shutdown();
    }

    #[test]
    fn reserved_installer_can_stop_runtime_without_reopening_admission() {
        let (state, rx, cancel) = state();
        let reservation = state.try_reserve_app_install().unwrap();
        assert!(state.shutdown_for_app_install());
        assert!(cancel.load(Ordering::SeqCst));
        assert!(state.queue_firmware_restore().is_err());
        assert!(rx.try_recv().is_err());
        drop(reservation);
    }
}
