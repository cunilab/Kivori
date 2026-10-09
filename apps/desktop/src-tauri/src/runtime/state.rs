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
use crate::firmware::{self, FirmwarePhase, FirmwareStatus};
use crate::ipc::dto::{
    self, ConfigDto, ConnectionStatusDto, DeskStatusDto, DiagnosticsDto, DiagnosticsSnapshot,
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
    /// Re-publish the current status (used by an explicit UI resync).
    Refresh,
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
    /// Latest raw diagnostics figures, written by the device thread about once a second.
    pub diagnostics: Arc<Mutex<DiagnosticsSnapshot>>,
    /// Window-lifecycle policy (hide-vs-quit / show-on-reactivate), shared with the window+tray handlers.
    pub lifecycle: Mutex<WindowLifecycle>,
    /// Live Device Studio preview streams (dev-only), cancelled on shutdown/teardown.
    #[cfg(feature = "device-studio")]
    pub previews: Arc<crate::ipc::channels::PreviewStreams>,
    commands: Mutex<Sender<DeviceCommand>>,
    cancel: Arc<AtomicBool>,
    device_thread: Mutex<Option<JoinHandle<()>>>,
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
            diagnostics: Arc::new(Mutex::new(DiagnosticsSnapshot::initial())),
            lifecycle: Mutex::new(WindowLifecycle::new()),
            #[cfg(feature = "device-studio")]
            previews: Arc::new(crate::ipc::channels::PreviewStreams::new()),
            commands: Mutex::new(commands),
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

    /// The diagnostics page as of now: the device thread's figures plus the stored config.
    #[must_use]
    pub fn diagnostics_snapshot(&self) -> DiagnosticsDto {
        let snapshot = self.diagnostics.lock().expect("diagnostics lock").clone();
        let config = self.config.lock().expect("config lock");
        dto::diagnostics_dto(&snapshot, &config, std::time::Instant::now())
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
    pub fn queue_firmware_flash(&self) -> Result<(), String> {
        if self.status_snapshot().connection != "connected" {
            return Err("Connect a compatible device before flashing firmware.".to_string());
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
            status.message = "Preparing firmware update.".to_string();
            previous
        };
        if let Err(error) = self.send_command(DeviceCommand::FlashFirmware) {
            *self.firmware_status.lock().expect("firmware status lock") = previous;
            return Err(error);
        }
        Ok(())
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
