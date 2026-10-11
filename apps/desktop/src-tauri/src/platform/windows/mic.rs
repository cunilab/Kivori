//! Windows Core Audio microphone mute on EVERY active capture endpoint (ADR-0011).
//!
//! WHICH DEVICES: `IMMDeviceEnumerator::EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)`. Each
//! endpoint's `IAudioEndpointVolume::SetMute` is called, then `GetMute` reads it back, so the
//! result is always what the OS reports for that device, never the request. Muting only the
//! default device is rejected: another app may record from a different one, so "muted" would be
//! false.
//!
//! WHILE MUTED: an `IMMNotificationClient` on this thread's own enumerator reports a device that
//! arrives (or becomes active); the thread rescans, mutes it too and reads it back. If that fails
//! the device reads live, so the state says live, not muted.
//!
//! OUTSIDE CHANGES: one `IAudioEndpointVolumeCallback` per endpoint. Every own write carries
//! [`KIVORI_MIC_EVENT_CONTEXT`], so the callback can tell Kivori's echo from the user toggling
//! mute in Sound settings. Callbacks are INGRESS ONLY: they post a command and return; all COM
//! work happens on the owning `kivori-mic` thread (same rules as the output-volume backend).
//!
//! IN USE: the privacy consent store,
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone`.
//! An app entry (a packaged app directly under it, a desktop app under `NonPackaged`) with a
//! `LastUsedTimeStart` and a `LastUsedTimeStop` of 0 is capturing now. It is undocumented, so any
//! failure to read it is unknown (`None`), never "not in use".

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::core::{implement, w, Result as WinResult, GUID, PCWSTR, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    eCapture, EDataFlow, ERole, IMMDevice, IMMDeviceEnumerator, IMMNotificationClient,
    IMMNotificationClient_Impl, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    REG_VALUE_TYPE,
};
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

use crate::platform::{
    ActionAvailability, BackendError, ChangeOrigin, ConfirmationClass, MicBackend, MicChange,
    MicDevice, MicMute,
};

/// Identifies mute changes Kivori itself originated (see the output-volume backend's context).
const KIVORI_MIC_EVENT_CONTEXT: GUID = GUID::from_u128(0x4b49_564f_5249_0003_0000_0000_0000_0001);

enum Command {
    Devices(Sender<Result<Vec<MicDevice>, BackendError>>),
    SetMute(bool, Sender<Result<Vec<MicDevice>, BackendError>>),
    /// A device came, went or changed state: rescan on the owning thread.
    Rescan,
    /// A mute callback fired; the origin was classified by its event context.
    Changed(ChangeOrigin),
    Shutdown,
}

pub struct WindowsMicBackend {
    commands: Sender<Command>,
    changes: Mutex<Receiver<MicChange>>,
    /// At least one active capture endpoint exists.
    available: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WindowsMicBackend {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<Command>();
        let (change_tx, change_rx) = mpsc::channel::<MicChange>();
        let available = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let thread_available = Arc::clone(&available);
        let thread_commands = command_tx.clone();
        let thread = std::thread::Builder::new()
            .name("kivori-mic".to_string())
            .spawn(move || {
                mic_thread(
                    command_rx,
                    change_tx,
                    thread_available,
                    thread_commands,
                    ready_tx,
                );
            })
            .expect("spawn kivori-mic thread");
        // Block until the first scan, so `availability()` is right as soon as `new()` returns. A
        // thread that dies first closes the channel and `available` stays false.
        let _ = ready_rx.recv();

        Self {
            commands: command_tx,
            changes: Mutex::new(change_rx),
            available,
            thread: Some(thread),
        }
    }

    fn call<T>(&self, make: impl FnOnce(Sender<T>) -> Command) -> Option<T> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands.send(make(reply_tx)).ok()?;
        reply_rx.recv().ok()
    }
}

impl Default for WindowsMicBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WindowsMicBackend {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn gone() -> BackendError {
    BackendError::Os("kivori-mic thread is gone".to_string())
}

impl MicBackend for WindowsMicBackend {
    fn availability(&self) -> ActionAvailability {
        if self.available.load(Ordering::SeqCst) {
            ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            }
        } else {
            ActionAvailability::RuntimeUnavailable {
                reason: "no microphone".to_string(),
            }
        }
    }

    fn devices(&self) -> Result<Vec<MicDevice>, BackendError> {
        self.call(Command::Devices).ok_or_else(gone)?
    }

    fn set_mute(&self, muted: bool) -> Result<Vec<MicDevice>, BackendError> {
        self.call(|reply| Command::SetMute(muted, reply))
            .ok_or_else(gone)?
    }

    fn in_use(&self) -> Option<bool> {
        consent_store_in_use()
    }

    fn try_recv_change(&self) -> Option<MicChange> {
        self.changes
            .lock()
            .expect("changes channel mutex")
            .try_recv()
            .ok()
    }
}

/// One bound capture endpoint, owned by the `kivori-mic` thread.
struct Endpoint {
    id: String,
    name: String,
    volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

impl Endpoint {
    fn read(&self) -> MicMute {
        // SAFETY: `volume` is a live, activated `IAudioEndpointVolume`.
        match unsafe { self.volume.GetMute() } {
            Ok(muted) if muted.as_bool() => MicMute::Muted,
            Ok(_) => MicMute::Live,
            Err(_) => MicMute::Unreadable,
        }
    }

    /// Write, then read back. A failed write is left to the read-back to report (the device then
    /// still reads live), so the result is always what the OS says.
    fn write(&self, muted: bool) -> MicMute {
        // SAFETY: as in `read`; `KIVORI_MIC_EVENT_CONTEXT` is a valid `'static` GUID.
        let _ = unsafe { self.volume.SetMute(muted, &KIVORI_MIC_EVENT_CONTEXT) };
        self.read()
    }

    fn device(&self, mute: MicMute) -> MicDevice {
        MicDevice {
            name: self.name.clone(),
            mute,
            capturing: None,
        }
    }

    fn release(self) {
        // SAFETY: the callback was registered on this thread, on this object.
        let _ = unsafe { self.volume.UnregisterControlChangeNotify(&self.callback) };
    }
}

/// State owned by the `kivori-mic` thread.
struct Mic {
    enumerator: IMMDeviceEnumerator,
    commands: Sender<Command>,
    endpoints: Vec<Endpoint>,
    /// Mic mute is on: an endpoint that arrives is muted too.
    hold_muted: bool,
    /// Last device state sent on `changes`; notifications that do not change it are dropped.
    reported: Option<Vec<MicDevice>>,
    changes: Sender<MicChange>,
}

impl Mic {
    /// Aligns the bound endpoints with the active capture devices: binds new ones (muting them if
    /// mic mute is on) and releases those that went away.
    fn rescan(&mut self) -> Result<(), BackendError> {
        // SAFETY: standard endpoint enumeration; failure is a handled `Err`.
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
        }
        .map_err(|err| BackendError::Os(err.to_string()))?;
        // SAFETY: `collection` is live.
        let count =
            unsafe { collection.GetCount() }.map_err(|err| BackendError::Os(err.to_string()))?;

        let mut present = Vec::new();
        for index in 0..count {
            // SAFETY: `index` is below the count just read.
            let Ok(device) = (unsafe { collection.Item(index) }) else {
                continue;
            };
            let Some(id) = endpoint_id(&device) else {
                continue;
            };
            if !self.endpoints.iter().any(|e| e.id == id) {
                if let Some(endpoint) = self.bind(&device, id.clone()) {
                    if self.hold_muted {
                        let _ = endpoint.write(true);
                    }
                    self.endpoints.push(endpoint);
                }
            }
            present.push(id);
        }

        let (keep, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut self.endpoints)
            .into_iter()
            .partition(|e| present.contains(&e.id));
        self.endpoints = keep;
        for endpoint in gone {
            endpoint.release();
        }
        Ok(())
    }

    fn bind(&self, device: &IMMDevice, id: String) -> Option<Endpoint> {
        // SAFETY: `device` is a live endpoint; activation failure is a handled `Err`.
        let volume: IAudioEndpointVolume =
            unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) }.ok()?;
        let callback: IAudioEndpointVolumeCallback = MicCallback {
            commands: self.commands.clone(),
        }
        .into();
        // SAFETY: `callback` is a valid, freshly constructed COM object of the right interface.
        unsafe { volume.RegisterControlChangeNotify(&callback) }.ok()?;
        Some(Endpoint {
            id,
            name: friendly_name(device).unwrap_or_else(|| "Microphone".to_string()),
            volume,
            callback,
        })
    }

    fn snapshot(&self) -> Vec<MicDevice> {
        self.endpoints.iter().map(|e| e.device(e.read())).collect()
    }

    fn devices(&mut self) -> Result<Vec<MicDevice>, BackendError> {
        self.rescan()?;
        Ok(self.snapshot())
    }

    fn set_mute(&mut self, muted: bool) -> Result<Vec<MicDevice>, BackendError> {
        self.rescan()?;
        if self.endpoints.is_empty() {
            return Err(BackendError::NoEndpoint);
        }
        self.hold_muted = muted;
        let devices: Vec<_> = self
            .endpoints
            .iter()
            .map(|e| e.device(e.write(muted)))
            .collect();
        self.reported = Some(devices.clone());
        Ok(devices)
    }

    fn publish(&mut self, origin: ChangeOrigin) {
        if self.rescan().is_err() {
            return;
        }
        let devices = self.snapshot();
        if self.reported.as_ref() == Some(&devices) {
            return;
        }
        self.reported = Some(devices.clone());
        let _ = self.changes.send(MicChange { devices, origin });
    }
}

fn endpoint_id(device: &IMMDevice) -> Option<String> {
    // SAFETY: `device` is live; the returned string is CoTaskMem-allocated and freed below.
    let id: PWSTR = unsafe { device.GetId() }.ok()?;
    // SAFETY: `id` is a valid NUL-terminated wide string until freed.
    let text = unsafe { id.to_string() }.ok();
    // SAFETY: allocated by `GetId` with `CoTaskMemAlloc`.
    unsafe { CoTaskMemFree(Some(id.0 as *const _)) };
    text
}

/// The endpoint's user-visible name (user data: shown, never logged).
fn friendly_name(device: &IMMDevice) -> Option<String> {
    // SAFETY: `device` is live; every call below checks its result.
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.ok()?;
    let value = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }.ok()?;
    let text: PWSTR = unsafe { PropVariantToStringAlloc(&value) }.ok()?;
    // SAFETY: a valid NUL-terminated wide string until freed.
    let name = unsafe { text.to_string() }.ok();
    // SAFETY: allocated by `PropVariantToStringAlloc` with `CoTaskMemAlloc`.
    unsafe { CoTaskMemFree(Some(text.0 as *const _)) };
    name.filter(|n| !n.is_empty())
}

/// Body of the dedicated `kivori-mic` OS thread. Owns every COM object for its whole lifetime.
fn mic_thread(
    commands: Receiver<Command>,
    changes: Sender<MicChange>,
    available: Arc<AtomicBool>,
    self_commands: Sender<Command>,
    ready: Sender<()>,
) {
    // SAFETY: called once, at the start of this dedicated thread's life, before any other COM
    // call on it.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    // SAFETY: `MMDeviceEnumerator` is a well-known CLSID; failure (no audio subsystem) is handled.
    let enumerator: Option<IMMDeviceEnumerator> =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok() };

    let Some(enumerator) = enumerator else {
        let _ = ready.send(());
        // SAFETY: matches the `CoInitializeEx` above; nothing COM is alive.
        unsafe { CoUninitialize() };
        return;
    };

    let client: IMMNotificationClient = MicNotificationClient {
        commands: self_commands.clone(),
    }
    .into();
    // SAFETY: `client` is a valid, freshly constructed COM object of the right interface.
    let registered = unsafe { enumerator.RegisterEndpointNotificationCallback(&client) }.is_ok();

    let mut mic = Mic {
        enumerator,
        commands: self_commands,
        endpoints: Vec::new(),
        hold_muted: false,
        reported: None,
        changes,
    };
    let _ = mic.rescan();
    available.store(!mic.endpoints.is_empty(), Ordering::SeqCst);
    let _ = ready.send(());

    loop {
        match commands.recv() {
            Ok(Command::Devices(reply)) => {
                let result = mic.devices();
                available.store(!mic.endpoints.is_empty(), Ordering::SeqCst);
                let _ = reply.send(result);
            }
            Ok(Command::SetMute(muted, reply)) => {
                let _ = reply.send(mic.set_mute(muted));
            }
            Ok(Command::Rescan) => {
                mic.publish(ChangeOrigin::EndpointRebind);
                available.store(!mic.endpoints.is_empty(), Ordering::SeqCst);
            }
            Ok(Command::Changed(origin)) => mic.publish(origin),
            Ok(Command::Shutdown) | Err(_) => break,
        }
    }

    // ORDERING IS LOAD-BEARING (see the output-volume backend): every COM interface must be
    // released before `CoUninitialize`. Endpoint callbacks first, then the notification client,
    // then the enumerator, and only then `CoUninitialize`. The explicit `drop()`s are what
    // release them on this side of that line.
    for endpoint in std::mem::take(&mut mic.endpoints) {
        endpoint.release();
    }
    if registered {
        // SAFETY: registered on this same thread above.
        let _ = unsafe {
            mic.enumerator
                .UnregisterEndpointNotificationCallback(&client)
        };
    }
    drop(client);
    let Mic { enumerator, .. } = mic;
    drop(enumerator);
    // SAFETY: matches the `CoInitializeEx` call above, on the same thread.
    unsafe { CoUninitialize() };
}

/// `IAudioEndpointVolumeCallback`: INGRESS ONLY. `OnNotify` performs no COM calls: it classifies
/// the write's origin by its event context and posts `Command::Changed`.
#[implement(IAudioEndpointVolumeCallback)]
struct MicCallback {
    commands: Sender<Command>,
}

impl IAudioEndpointVolumeCallback_Impl for MicCallback_Impl {
    fn OnNotify(&self, notify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> WinResult<()> {
        if notify.is_null() {
            return Ok(());
        }
        // SAFETY: the audio engine guarantees `notify` is valid for the duration of this call.
        let data = unsafe { &*notify };
        let origin = if data.guidEventContext == KIVORI_MIC_EVENT_CONTEXT {
            ChangeOrigin::Kivori
        } else {
            ChangeOrigin::External
        };
        let _ = self.commands.send(Command::Changed(origin));
        Ok(())
    }
}

/// `IMMNotificationClient`: INGRESS ONLY. A capture device that arrives, leaves or changes state
/// posts `Command::Rescan`; everything else is a documented no-op.
#[implement(IMMNotificationClient)]
struct MicNotificationClient {
    commands: Sender<Command>,
}

impl IMMNotificationClient_Impl for MicNotificationClient_Impl {
    fn OnDeviceStateChanged(&self, _device_id: &PCWSTR, _new_state: DEVICE_STATE) -> WinResult<()> {
        let _ = self.commands.send(Command::Rescan);
        Ok(())
    }

    fn OnDeviceAdded(&self, _device_id: &PCWSTR) -> WinResult<()> {
        let _ = self.commands.send(Command::Rescan);
        Ok(())
    }

    fn OnDeviceRemoved(&self, _device_id: &PCWSTR) -> WinResult<()> {
        let _ = self.commands.send(Command::Rescan);
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        _flow: EDataFlow,
        _role: ERole,
        _default_device_id: &PCWSTR,
    ) -> WinResult<()> {
        // Every active capture device is covered, so the default does not matter here.
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _device_id: &PCWSTR, _key: &PROPERTYKEY) -> WinResult<()> {
        Ok(())
    }
}

fn is_ok(error: WIN32_ERROR) -> bool {
    error.0 == 0
}

/// Opens `path` under `parent` for reading.
fn open_key(parent: HKEY, path: PCWSTR) -> Option<HKEY> {
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-terminated and outlives the call; `key` is a valid out pointer.
    is_ok(unsafe { RegOpenKeyExW(parent, path, 0, KEY_READ, &mut key) }).then_some(key)
}

/// The names of `key`'s subkeys, each NUL-terminated, or `None` if it cannot be listed.
fn subkeys(key: HKEY) -> Option<Vec<Vec<u16>>> {
    let mut names = Vec::new();
    for index in 0u32.. {
        let mut buffer = [0u16; 512];
        let mut length = buffer.len() as u32;
        // SAFETY: `buffer` is writable for `length` characters; the optional parameters are unused.
        let error = unsafe {
            RegEnumKeyExW(
                key,
                index,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
                None,
                PWSTR::null(),
                None,
                None,
            )
        };
        if error.0 == 259 {
            // ERROR_NO_MORE_ITEMS
            return Some(names);
        }
        if !is_ok(error) {
            // A name too long for the buffer (ERROR_MORE_DATA) is skipped; anything else is
            // unreadable.
            if error.0 == 234 {
                continue;
            }
            return None;
        }
        names.push(buffer[..=length as usize].to_vec());
    }
    Some(names)
}

/// A `REG_QWORD` value, or `None` if it is absent or another type.
fn qword(key: HKEY, name: PCWSTR) -> Option<u64> {
    let mut value = 0u64;
    let mut size = std::mem::size_of::<u64>() as u32;
    let mut kind = REG_VALUE_TYPE::default();
    // SAFETY: `value` is writable for `size` bytes; `name` is NUL-terminated.
    let error = unsafe {
        RegQueryValueExW(
            key,
            name,
            None,
            Some(&mut kind),
            Some(std::ptr::addr_of_mut!(value).cast()),
            Some(&mut size),
        )
    };
    (is_ok(error) && size == 8).then_some(value)
}

/// Whether one app entry is capturing now: it started and has not stopped.
fn entry_capturing(app: HKEY) -> bool {
    let start = qword(app, w!("LastUsedTimeStart")).unwrap_or(0);
    let stop = qword(app, w!("LastUsedTimeStop")).unwrap_or(0);
    start != 0 && stop == 0
}

/// Whether any app entry under `parent` (and, for `NonPackaged`, its children) is capturing.
/// `None` when the store cannot be read.
fn capturing_under(parent: HKEY) -> Option<bool> {
    let mut any = false;
    for name in subkeys(parent)? {
        let Some(child) = open_key(parent, PCWSTR(name.as_ptr())) else {
            continue;
        };
        if String::from_utf16_lossy(&name[..name.len() - 1]) == "NonPackaged" {
            any |= capturing_under(child).unwrap_or(false);
        } else {
            any |= entry_capturing(child);
        }
        // SAFETY: `child` was opened above and is closed exactly once.
        let _ = unsafe { RegCloseKey(child) };
    }
    Some(any)
}

/// "Mic in use" from the privacy consent store (ADR-0011); `None` = unknown.
fn consent_store_in_use() -> Option<bool> {
    let root = open_key(
        HKEY_CURRENT_USER,
        w!(
            r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone"
        ),
    )?;
    let in_use = capturing_under(root);
    // SAFETY: `root` was opened above and is closed exactly once.
    let _ = unsafe { RegCloseKey(root) };
    in_use
}
