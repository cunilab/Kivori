//! Windows Core Audio master volume.
//!
//! THREADING (research R-50, R-76): there is no tokio in this crate. A dedicated
//! `kivori-audio` OS thread performs `CoInitializeEx`, owns the `IMMDeviceEnumerator` and the
//! endpoint, and holds both callbacks. Callbacks are INGRESS ONLY: they push onto a channel and
//! never perform COM work inline. All rebinding happens on the owning thread.
//!
//! EVENT CONTEXT: every write passes `KIVORI_EVENT_CONTEXT` so the volume callback can tell
//! Kivori's own echo from a genuinely external change.
//!
//! ENDPOINT: the default render endpoint is a MOVING TARGET. An `IMMNotificationClient` rebinds
//! on `OnDefaultDeviceChanged(eRender, eConsole)` and publishes the NEW endpoint's confirmed
//! value. `eCommunications` is deliberately not followed; whether `eMultimedia` resolves
//! identically to `eConsole` is a physical-validation item (Task 14), not an assumption baked in
//! here.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::core::{implement, Result as WinResult, GUID, PCWSTR};
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    eConsole, eRender, EDataFlow, ERole, IMMDevice, IMMDeviceEnumerator, IMMNotificationClient,
    IMMNotificationClient_Impl, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

use crate::platform::{ActionAvailability, BackendError, ConfirmationClass, VolumeBackend};

/// Identifies volume changes Kivori itself originated. `SetMasterVolumeLevelScalar` takes this
/// GUID as its event context, and `OnNotify` reports it back in
/// `AUDIO_VOLUME_NOTIFICATION_DATA.guidEventContext`, which is how the callback tells the echo of
/// our own write apart from a genuinely external change (the Windows flyout, a media key,
/// another app).
const KIVORI_EVENT_CONTEXT: GUID = GUID::from_u128(0x4b49_564f_5249_0002_0000_0000_0000_0001);

/// Documented device role (research R-50/R-76): `eRender` + `eConsole`. `eCommunications` is
/// deliberately not followed.
const ROLE: ERole = eConsole;
const FLOW: EDataFlow = eRender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOrigin {
    /// The echo of a write Kivori itself just performed. `set()` already confirmed it
    /// synchronously via its own read-back, so this is informational only.
    Kivori,
    /// A change Kivori did not originate: the Windows flyout, a media key, another app.
    External,
    /// The default render endpoint changed; this is the new endpoint's freshly read volume.
    EndpointRebind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeChange {
    pub percent: u8,
    pub origin: ChangeOrigin,
}

/// Requests handled on the owning `kivori-audio` thread. `Read`/`Set` carry a one-shot reply
/// channel so the public, `Sync` API can make a synchronous call into the thread that actually
/// owns the non-`Send` COM objects.
enum Command {
    Read(Sender<Result<u8, BackendError>>),
    Set(u8, Sender<Result<u8, BackendError>>),
    /// Raised by the `IMMNotificationClient` callback when the default render endpoint changes.
    /// All COM work for the rebind happens here, on the owning thread — never in the callback.
    Rebind,
    Shutdown,
}

pub struct WindowsVolumeBackend {
    commands: Sender<Command>,
    changes: Mutex<Receiver<VolumeChange>>,
    available: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WindowsVolumeBackend {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<Command>();
        let (change_tx, change_rx) = mpsc::channel::<VolumeChange>();
        let available = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let thread_available = Arc::clone(&available);
        let thread_command_tx = command_tx.clone();
        let thread = std::thread::Builder::new()
            .name("kivori-audio".to_string())
            .spawn(move || {
                audio_thread(
                    command_rx,
                    change_tx,
                    thread_available,
                    thread_command_tx,
                    ready_tx,
                );
            })
            .expect("spawn kivori-audio thread");

        // Block until the owning thread has attempted its initial bind, so `availability()` is
        // correct the instant `new()` returns rather than racing thread startup. If the thread
        // panics before signalling, the channel closes, `recv` returns `Err`, and `available`
        // is left at its initial `false` — RuntimeUnavailable, never a panic here.
        let _ = ready_rx.recv();

        Self {
            commands: command_tx,
            changes: Mutex::new(change_rx),
            available,
            thread: Some(thread),
        }
    }

    /// Drains one pending backend-originated volume change, if any. Called each device-thread
    /// tick from `runtime::device_task`. Every value it yields was read from the OS by the
    /// owning `kivori-audio` thread — never assumed, requested, or cached.
    pub fn try_recv_change(&self) -> Option<VolumeChange> {
        self.changes
            .lock()
            .expect("changes channel mutex")
            .try_recv()
            .ok()
    }
}

impl Default for WindowsVolumeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WindowsVolumeBackend {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl VolumeBackend for WindowsVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        if self.available.load(Ordering::SeqCst) {
            ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            }
        } else {
            // Windows IS implemented; a missing endpoint is a runtime fact, never
            // `NotImplementedYet`.
            ActionAvailability::RuntimeUnavailable {
                reason: "no default render endpoint".to_string(),
            }
        }
    }

    fn read(&self) -> Result<u8, BackendError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(Command::Read(reply_tx))
            .map_err(|_| BackendError::Os("kivori-audio thread is gone".to_string()))?;
        reply_rx
            .recv()
            .map_err(|_| BackendError::Os("kivori-audio thread is gone".to_string()))?
    }

    fn set(&self, percent: u8) -> Result<u8, BackendError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(Command::Set(percent, reply_tx))
            .map_err(|_| BackendError::Os("kivori-audio thread is gone".to_string()))?;
        reply_rx
            .recv()
            .map_err(|_| BackendError::Os("kivori-audio thread is gone".to_string()))?
    }
}

/// State the owning thread holds for the currently-bound endpoint. `None` when there is no
/// default render endpoint right now.
#[derive(Default)]
struct EndpointState {
    volume: Option<IAudioEndpointVolume>,
    callback: Option<IAudioEndpointVolumeCallback>,
}

/// Body of the dedicated `kivori-audio` OS thread. Owns every COM object for its whole lifetime;
/// nothing here is ever touched from another thread.
fn audio_thread(
    commands: Receiver<Command>,
    changes: Sender<VolumeChange>,
    available: Arc<AtomicBool>,
    self_commands: Sender<Command>,
    ready: Sender<()>,
) {
    // SAFETY: called once, at the start of this dedicated thread's life, before any other COM
    // call on it.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    // SAFETY: `MMDeviceEnumerator` is a well-known CLSID; `CoCreateInstance` is the standard way
    // to create it. Failure (no audio subsystem) is handled below, not unwrapped.
    let enumerator: Option<IMMDeviceEnumerator> =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok() };

    // Register the endpoint-notification client on the enumerator itself (not on any particular
    // endpoint) so a default-device switch is observed even while no endpoint is currently bound.
    let notification_client: Option<IMMNotificationClient> =
        enumerator.as_ref().map(|enumerator| {
            let client: IMMNotificationClient = NotificationClient {
                commands: self_commands.clone(),
            }
            .into();
            // SAFETY: `client` is a valid, freshly constructed COM object of the right interface.
            let _ = unsafe { enumerator.RegisterEndpointNotificationCallback(&client) };
            client
        });

    let mut state = EndpointState::default();
    if let Some(enumerator) = &enumerator {
        rebind(enumerator, &mut state, &changes);
    }
    available.store(state.volume.is_some(), Ordering::SeqCst);
    let _ = ready.send(());

    loop {
        match commands.recv() {
            Ok(Command::Read(reply)) => {
                let result = match &state.volume {
                    Some(volume) => read_scalar(volume),
                    None => Err(BackendError::NoEndpoint),
                };
                let _ = reply.send(result);
            }
            Ok(Command::Set(percent, reply)) => {
                let result = match &state.volume {
                    Some(volume) => write_scalar(volume, percent),
                    None => Err(BackendError::NoEndpoint),
                };
                let _ = reply.send(result);
            }
            Ok(Command::Rebind) => {
                if let Some(enumerator) = &enumerator {
                    rebind(enumerator, &mut state, &changes);
                    available.store(state.volume.is_some(), Ordering::SeqCst);
                }
            }
            Ok(Command::Shutdown) | Err(_) => break,
        }
    }

    // Drop (contract item 8): unregister both callbacks and CoUninitialize, all still on this
    // owning thread.
    //
    // ORDERING IS LOAD-BEARING, and invisible to the type system: every live COM interface must
    // be released before `CoUninitialize` runs, or its `Release()` call lands on an apartment
    // that no longer exists (undefined behaviour — this is what previously crashed CI with
    // STATUS_ACCESS_VIOLATION). Release order below: endpoint volume + its callback, then the
    // notification client, then the enumerator, and only then `CoUninitialize`. The unregister
    // calls just below borrow `enumerator`/`notification_client` (they don't consume), so the
    // explicit `drop()`s afterward are what actually releases them on this side of the
    // `CoUninitialize` line — do NOT delete those `drop()` calls or go back to letting these two
    // bindings fall off the end of the function, which reintroduces the crash.
    if let (Some(volume), Some(callback)) = (state.volume.take(), state.callback.take()) {
        // SAFETY: both were registered on this same thread above.
        let _ = unsafe { volume.UnregisterControlChangeNotify(&callback) };
    }
    if let (Some(enumerator), Some(client)) = (enumerator.as_ref(), notification_client.as_ref()) {
        // SAFETY: registered on this same thread above.
        let _ = unsafe { enumerator.UnregisterEndpointNotificationCallback(client) };
    }
    drop(notification_client);
    drop(enumerator);
    // SAFETY: matches the `CoInitializeEx` call above, on the same thread; every COM interface
    // above has been explicitly released by this point.
    unsafe { CoUninitialize() };
}

/// Unregisters/releases the old endpoint (if any), binds the current default render endpoint,
/// registers the volume-change callback, and — on success — reads the new endpoint's current
/// volume and publishes it as `ChangeOrigin::EndpointRebind`. Runs entirely on the owning thread.
fn rebind(
    enumerator: &IMMDeviceEnumerator,
    state: &mut EndpointState,
    changes: &Sender<VolumeChange>,
) {
    if let (Some(volume), Some(callback)) = (state.volume.take(), state.callback.take()) {
        // SAFETY: both were registered on this same thread, on this same object.
        let _ = unsafe { volume.UnregisterControlChangeNotify(&callback) };
    }

    // SAFETY: standard WASAPI endpoint acquisition; absence of a default endpoint is a normal,
    // handled `Err`, not a precondition violation.
    let bound: WinResult<IAudioEndpointVolume> = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(FLOW, ROLE)
            .and_then(|device: IMMDevice| device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None))
    };

    let Ok(volume) = bound else {
        return;
    };

    let callback: IAudioEndpointVolumeCallback = VolumeCallback {
        changes: changes.clone(),
    }
    .into();
    // SAFETY: `callback` is a valid, freshly constructed COM object of the right interface.
    if unsafe { volume.RegisterControlChangeNotify(&callback) }.is_err() {
        return;
    }

    if let Ok(percent) = read_scalar(&volume) {
        let _ = changes.send(VolumeChange {
            percent,
            origin: ChangeOrigin::EndpointRebind,
        });
    }

    state.volume = Some(volume);
    state.callback = Some(callback);
}

/// `f32 0.0..=1.0` -> `u8 0..=100` with rounding. The only place a scalar volume becomes an
/// integer percent; no float escapes past this boundary.
fn scalar_to_percent(level: f32) -> u8 {
    (level.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// `u8 0..=100` -> `f32 0.0..=1.0`. The only place a percent becomes a scalar volume.
fn percent_to_scalar(percent: u8) -> f32 {
    f32::from(percent.min(100)) / 100.0
}

fn read_scalar(volume: &IAudioEndpointVolume) -> Result<u8, BackendError> {
    // SAFETY: `volume` is a live, activated `IAudioEndpointVolume`.
    let level = unsafe { volume.GetMasterVolumeLevelScalar() }
        .map_err(|err| BackendError::Os(err.to_string()))?;
    Ok(scalar_to_percent(level))
}

/// Product rule: `set` performs the write then re-reads for synchronous confirmation. If the
/// write succeeds but the read-back fails, that is reported distinctly as
/// `BackendError::ReadBackUnavailable` — never silently treated as success.
fn write_scalar(volume: &IAudioEndpointVolume, percent: u8) -> Result<u8, BackendError> {
    let level = percent_to_scalar(percent);
    // SAFETY: `volume` is a live, activated `IAudioEndpointVolume`; `KIVORI_EVENT_CONTEXT` is a
    // valid `'static` GUID.
    unsafe { volume.SetMasterVolumeLevelScalar(level, &KIVORI_EVENT_CONTEXT) }
        .map_err(|err| BackendError::Os(err.to_string()))?;
    read_scalar(volume).map_err(|_| BackendError::ReadBackUnavailable)
}

/// `IAudioEndpointVolumeCallback`: INGRESS ONLY. `OnNotify` performs no COM calls — it classifies
/// the write's origin by comparing `guidEventContext` to `KIVORI_EVENT_CONTEXT` and pushes onto
/// the channel. All further handling happens on the owning `kivori-audio` thread.
#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    changes: Sender<VolumeChange>,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, notify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> WinResult<()> {
        if notify.is_null() {
            return Ok(());
        }
        // SAFETY: the audio engine guarantees `notify` is valid for the duration of this call.
        let data = unsafe { &*notify };
        let origin = if data.guidEventContext == KIVORI_EVENT_CONTEXT {
            ChangeOrigin::Kivori
        } else {
            ChangeOrigin::External
        };
        let percent = scalar_to_percent(data.fMasterVolume);
        let _ = self.changes.send(VolumeChange { percent, origin });
        Ok(())
    }
}

/// `IMMNotificationClient`: INGRESS ONLY. `OnDefaultDeviceChanged` performs no COM calls — for
/// the render/console role it pushes `Command::Rebind`; every other notification is a documented
/// no-op. All rebinding happens on the owning `kivori-audio` thread that drains the channel.
#[implement(IMMNotificationClient)]
struct NotificationClient {
    commands: Sender<Command>,
}

impl IMMNotificationClient_Impl for NotificationClient_Impl {
    fn OnDeviceStateChanged(&self, _device_id: &PCWSTR, _new_state: DEVICE_STATE) -> WinResult<()> {
        Ok(())
    }

    fn OnDeviceAdded(&self, _device_id: &PCWSTR) -> WinResult<()> {
        Ok(())
    }

    fn OnDeviceRemoved(&self, _device_id: &PCWSTR) -> WinResult<()> {
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _default_device_id: &PCWSTR,
    ) -> WinResult<()> {
        // Only the render/console role Kivori controls. `eCommunications` is deliberately not
        // followed; comparing enum values inline is not COM work.
        if flow == FLOW && role == ROLE {
            let _ = self.commands.send(Command::Rebind);
        }
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _device_id: &PCWSTR, _key: &PROPERTYKEY) -> WinResult<()> {
        Ok(())
    }
}
