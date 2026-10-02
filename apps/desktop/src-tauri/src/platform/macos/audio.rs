//! macOS CoreAudio master volume and mute on the DEFAULT OUTPUT device.
//!
//! VOLUME API: `kAudioHardwareServiceDeviceProperty_VirtualMainVolume` ('vmvc', output scope,
//! main element), read and written through the ordinary `AudioObject{Get,Set}PropertyData`. Per
//! AudioToolbox's `AudioHardwareService.h`, it drives the device's true main volume control if it
//! has one, otherwise the channel controls of its preferred stereo pair while keeping their
//! balance, which is exactly the "main element, else per-channel" fallback done for us, and the
//! same slider the menu bar shows. The `AudioHardwareService*` accessor functions are deprecated
//! (10.11) but the HAL serves the selector itself, so no AudioToolbox link is needed; this is the
//! route SimplyCoreAudio (`AudioDevice+VirtualMainOutput.swift`) uses. A device with no software
//! volume at all (HDMI, some USB DACs) lacks 'vmvc' and is reported `RuntimeUnavailable`.
//!
//! MUTE: `kAudioDevicePropertyMute` ('mute', output scope, main element). There is no
//! virtual-main mute selector.
//! ponytail: no per-channel mute fallback; add it when a device with channel-only mute shows up.
//!
//! THREADING mirrors the Windows backend: a dedicated `kivori-audio` thread owns all state and
//! makes every CoreAudio call; the public API sends it commands. HAL listener callbacks are
//! ingress only: they post a command and return.
//!
//! LISTENER LIFETIME: `AudioObjectRemovePropertyListener` does not drain notifications the HAL
//! already dispatched (see acondigital/JUCE#24), so the client-data pointer is never
//! dereferenced. It is an opaque token looked up in [`LISTENERS`]; Drop removes every HAL
//! listener first and only then the token, after which a late callback finds nothing and returns.
//!
//! ECHO: CoreAudio has no event-context GUID. Each own write is recorded with its read-back
//! value and time; a notification whose freshly read state differs from the last reported state
//! only in that field, equal to that value, within [`ECHO_WINDOW`], is `Kivori`. Else `External`.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::platform::{
    ActionAvailability, BackendError, ChangeOrigin, ConfirmationClass, VolumeBackend, VolumeChange,
};

type AudioObjectId = u32;
type OsStatus = i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct PropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

type ListenerProc =
    unsafe extern "C" fn(AudioObjectId, u32, *const PropertyAddress, *mut c_void) -> OsStatus;

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectHasProperty(id: AudioObjectId, address: *const PropertyAddress) -> u8;
    fn AudioObjectIsPropertySettable(
        id: AudioObjectId,
        address: *const PropertyAddress,
        out_settable: *mut u8,
    ) -> OsStatus;
    fn AudioObjectGetPropertyData(
        id: AudioObjectId,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        io_size: *mut u32,
        out_data: *mut c_void,
    ) -> OsStatus;
    fn AudioObjectSetPropertyData(
        id: AudioObjectId,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: u32,
        data: *const c_void,
    ) -> OsStatus;
    fn AudioObjectAddPropertyListener(
        id: AudioObjectId,
        address: *const PropertyAddress,
        listener: ListenerProc,
        client_data: *mut c_void,
    ) -> OsStatus;
    fn AudioObjectRemovePropertyListener(
        id: AudioObjectId,
        address: *const PropertyAddress,
        listener: ListenerProc,
        client_data: *mut c_void,
    ) -> OsStatus;
}

/// A CoreAudio four-character code.
const fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

const SYSTEM_OBJECT: AudioObjectId = 1;
const UNKNOWN_OBJECT: AudioObjectId = 0;
const SCOPE_GLOBAL: u32 = fourcc(b"glob");
const SCOPE_OUTPUT: u32 = fourcc(b"outp");
const ELEMENT_MAIN: u32 = 0;
const ELEMENT_WILDCARD: u32 = u32::MAX;

const DEFAULT_OUTPUT: PropertyAddress = PropertyAddress {
    selector: fourcc(b"dOut"),
    scope: SCOPE_GLOBAL,
    element: ELEMENT_MAIN,
};
/// `kAudioHardwarePropertyRunLoop`; set to NULL so the HAL delivers notifications on its own
/// thread instead of the main run loop (which a test binary, or a busy UI thread, never spins).
const RUN_LOOP: PropertyAddress = PropertyAddress {
    selector: fourcc(b"rnlp"),
    scope: SCOPE_GLOBAL,
    element: ELEMENT_MAIN,
};
const VIRTUAL_MAIN_VOLUME: PropertyAddress = PropertyAddress {
    selector: fourcc(b"vmvc"),
    scope: SCOPE_OUTPUT,
    element: ELEMENT_MAIN,
};
const MUTE: PropertyAddress = PropertyAddress {
    selector: fourcc(b"mute"),
    scope: SCOPE_OUTPUT,
    element: ELEMENT_MAIN,
};
/// Listened to in addition to 'vmvc' so a change to any channel control is observed too; the
/// thread re-reads and de-duplicates, so extra notifications cost nothing.
const CHANNEL_VOLUMES: PropertyAddress = PropertyAddress {
    selector: fourcc(b"volm"),
    scope: SCOPE_OUTPUT,
    element: ELEMENT_WILDCARD,
};
const DEVICE_LISTENS: [PropertyAddress; 3] = [VIRTUAL_MAIN_VOLUME, CHANNEL_VOLUMES, MUTE];

/// How long after an own write a matching notification still counts as its echo.
const ECHO_WINDOW: Duration = Duration::from_millis(250);

enum Command {
    Read(Sender<Result<u8, BackendError>>),
    Set(u8, Sender<Result<u8, BackendError>>),
    ReadMute(Sender<Result<bool, BackendError>>),
    SetMute(bool, Sender<Result<bool, BackendError>>),
    /// A volume/mute listener fired on the bound device.
    Changed,
    /// The default output device changed.
    Rebind,
    Shutdown,
}

/// Token -> command sender for every live backend. Listener callbacks resolve their token here
/// under the lock, so a callback can never reach a backend that has been torn down.
static LISTENERS: Mutex<BTreeMap<usize, Sender<Command>>> = Mutex::new(BTreeMap::new());
static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(1);

fn post(token: usize, command: Command) {
    // Never panic across the FFI boundary: a poisoned lock still holds a valid map.
    let map = LISTENERS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tx) = map.get(&token) {
        let _ = tx.send(command);
    }
}

/// HAL listener for the bound device's volume and mute. INGRESS ONLY.
unsafe extern "C" fn on_device_property(
    _id: AudioObjectId,
    _count: u32,
    _addresses: *const PropertyAddress,
    client_data: *mut c_void,
) -> OsStatus {
    post(client_data as usize, Command::Changed);
    0
}

/// HAL listener for the system object's default output device. INGRESS ONLY.
unsafe extern "C" fn on_default_output(
    _id: AudioObjectId,
    _count: u32,
    _addresses: *const PropertyAddress,
    client_data: *mut c_void,
) -> OsStatus {
    post(client_data as usize, Command::Rebind);
    0
}

pub struct MacVolumeBackend {
    commands: Sender<Command>,
    changes: Mutex<Receiver<VolumeChange>>,
    /// `Err(reason)` while there is no usable default output device.
    available: Arc<Mutex<Result<(), String>>>,
    thread: Option<JoinHandle<()>>,
}

impl MacVolumeBackend {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<Command>();
        let (change_tx, change_rx) = mpsc::channel::<VolumeChange>();
        let available = Arc::new(Mutex::new(Err("audio thread not started".to_string())));
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        LISTENERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(token, command_tx.clone());

        let thread_available = Arc::clone(&available);
        let thread = std::thread::Builder::new()
            .name("kivori-audio".to_string())
            .spawn(move || audio_thread(token, command_rx, change_tx, thread_available, ready_tx))
            .expect("spawn kivori-audio thread");

        // Wait for the initial bind so `availability()` is right as soon as `new()` returns. If
        // the thread dies first the channel closes and availability stays unavailable.
        let _ = ready_rx.recv();

        Self {
            commands: command_tx,
            changes: Mutex::new(change_rx),
            available,
            thread: Some(thread),
        }
    }

    fn call<T>(
        &self,
        make: impl FnOnce(Sender<Result<T, BackendError>>) -> Command,
    ) -> Result<T, BackendError> {
        let gone = || BackendError::Os("kivori-audio thread is gone".to_string());
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands.send(make(reply_tx)).map_err(|_| gone())?;
        reply_rx.recv().map_err(|_| gone())?
    }
}

impl Default for MacVolumeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MacVolumeBackend {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl VolumeBackend for MacVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        match &*self.available.lock().unwrap_or_else(|e| e.into_inner()) {
            Ok(()) => ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            // macOS IS implemented; a missing device is a runtime fact, never NotImplementedYet.
            Err(reason) => ActionAvailability::RuntimeUnavailable {
                reason: reason.clone(),
            },
        }
    }

    fn read(&self) -> Result<u8, BackendError> {
        self.call(Command::Read)
    }

    fn set(&self, percent: u8) -> Result<u8, BackendError> {
        self.call(|reply| Command::Set(percent, reply))
    }

    fn read_mute(&self) -> Result<bool, BackendError> {
        self.call(Command::ReadMute)
    }

    fn set_mute(&self, muted: bool) -> Result<bool, BackendError> {
        self.call(|reply| Command::SetMute(muted, reply))
    }

    fn try_recv_change(&self) -> Option<VolumeChange> {
        self.changes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_recv()
            .ok()
    }
}

/// An own write, as read back afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnWrite {
    Volume(u8),
    Mute(bool),
}

/// Pure echo classification. `observed` is the freshly read (percent, muted); `previous` the
/// last state reported. It is Kivori's echo only if the field Kivori wrote now holds the written
/// value, the other field is unchanged, and the write was at most [`ECHO_WINDOW`] ago.
fn classify(
    observed: (u8, bool),
    previous: Option<(u8, bool)>,
    last_write: Option<(OwnWrite, Instant)>,
    now: Instant,
) -> ChangeOrigin {
    let Some((write, at)) = last_write else {
        return ChangeOrigin::External;
    };
    if now.saturating_duration_since(at) > ECHO_WINDOW {
        return ChangeOrigin::External;
    }
    let other_unchanged = |same: bool| previous.is_none() || same;
    let echo = match write {
        OwnWrite::Volume(p) => {
            observed.0 == p && other_unchanged(previous.is_some_and(|prev| prev.1 == observed.1))
        }
        OwnWrite::Mute(m) => {
            observed.1 == m && other_unchanged(previous.is_some_and(|prev| prev.0 == observed.0))
        }
    };
    if echo {
        ChangeOrigin::Kivori
    } else {
        ChangeOrigin::External
    }
}

/// `f32 0.0..=1.0` -> `u8 0..=100`, rounded. The only place a scalar becomes a percent.
fn scalar_to_percent(level: f32) -> u8 {
    (level.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// `u8 0..=100` -> `f32 0.0..=1.0`. The only place a percent becomes a scalar.
fn percent_to_scalar(percent: u8) -> f32 {
    f32::from(percent.min(100)) / 100.0
}

fn os_error(what: &str, status: OsStatus) -> BackendError {
    BackendError::Os(format!("{what} failed (OSStatus {status})"))
}

/// Reads a fixed-size POD property.
fn get<T: Copy + Default>(id: AudioObjectId, address: &PropertyAddress) -> Result<T, OsStatus> {
    let mut value = T::default();
    let mut size = std::mem::size_of::<T>() as u32;
    // SAFETY: `address` is a valid property address; `value`/`size` describe a writable buffer
    // of exactly the property's documented type (f32, u32 or AudioObjectID here).
    let status = unsafe {
        AudioObjectGetPropertyData(
            id,
            address,
            0,
            std::ptr::null(),
            &mut size,
            std::ptr::addr_of_mut!(value).cast(),
        )
    };
    if status == 0 && size as usize == std::mem::size_of::<T>() {
        Ok(value)
    } else {
        Err(status)
    }
}

fn put<T: Copy>(id: AudioObjectId, address: &PropertyAddress, value: T) -> Result<(), OsStatus> {
    // SAFETY: `address` is a valid property address; `value` is a readable buffer of exactly the
    // property's documented type and size.
    let status = unsafe {
        AudioObjectSetPropertyData(
            id,
            address,
            0,
            std::ptr::null(),
            std::mem::size_of::<T>() as u32,
            std::ptr::addr_of!(value).cast(),
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(status)
    }
}

fn read_volume(device: AudioObjectId) -> Result<u8, BackendError> {
    get::<f32>(device, &VIRTUAL_MAIN_VOLUME)
        .map(scalar_to_percent)
        .map_err(|s| os_error("reading volume", s))
}

fn read_mute(device: AudioObjectId) -> Result<bool, BackendError> {
    get::<u32>(device, &MUTE)
        .map(|m| m != 0)
        .map_err(|s| os_error("reading mute", s))
}

/// The default output device, if it has a settable software volume. `Err` carries the
/// `BackendError` commands return and, through it, the availability reason.
fn default_output() -> Result<AudioObjectId, BackendError> {
    let device = get::<AudioObjectId>(SYSTEM_OBJECT, &DEFAULT_OUTPUT)
        .map_err(|s| os_error("reading the default output device", s))?;
    if device == UNKNOWN_OBJECT {
        return Err(BackendError::NoEndpoint);
    }
    let mut settable = 0u8;
    // SAFETY: `device` is an AudioObjectID the HAL just returned; both pointers are valid.
    let has = unsafe { AudioObjectHasProperty(device, &VIRTUAL_MAIN_VOLUME) } != 0
        && unsafe { AudioObjectIsPropertySettable(device, &VIRTUAL_MAIN_VOLUME, &mut settable) }
            == 0
        && settable != 0;
    if has {
        Ok(device)
    } else {
        Err(BackendError::Os(
            "the default output device has no software volume control".to_string(),
        ))
    }
}

fn unavailable_reason(binding: &Result<AudioObjectId, BackendError>) -> Result<(), String> {
    match binding {
        Ok(_) => Ok(()),
        Err(BackendError::NoEndpoint) => Err("no default output device".to_string()),
        Err(BackendError::Os(reason)) => Err(reason.clone()),
        Err(BackendError::ReadBackUnavailable) => Err("audio state unreadable".to_string()),
    }
}

/// State owned by the `kivori-audio` thread.
struct Audio {
    token: usize,
    binding: Result<AudioObjectId, BackendError>,
    /// Last state sent on `changes`; notifications that do not change it are dropped.
    reported: Option<(u8, bool)>,
    last_write: Option<(OwnWrite, Instant)>,
    changes: Sender<VolumeChange>,
}

impl Audio {
    fn client_data(&self) -> *mut c_void {
        self.token as *mut c_void
    }

    fn device_listeners(&self, device: AudioObjectId, add: bool) {
        for address in &DEVICE_LISTENS {
            // SAFETY: valid address and an `extern "C"` proc with the HAL's signature; the client
            // data is an opaque token, never dereferenced (module docs). A selector the device
            // lacks just returns an error, which is fine to ignore.
            let _ = unsafe {
                if add {
                    AudioObjectAddPropertyListener(
                        device,
                        address,
                        on_device_property,
                        self.client_data(),
                    )
                } else {
                    AudioObjectRemovePropertyListener(
                        device,
                        address,
                        on_device_property,
                        self.client_data(),
                    )
                }
            };
        }
    }

    /// Moves the device listeners to the current default output device and publishes its
    /// freshly read state as `EndpointRebind`.
    fn rebind(&mut self) {
        if let Ok(old) = self.binding {
            self.device_listeners(old, false);
        }
        self.binding = default_output();
        self.reported = None;
        self.last_write = None;
        let Ok(device) = self.binding else {
            return;
        };
        self.device_listeners(device, true);
        if let (Ok(percent), Ok(muted)) = (read_volume(device), read_mute(device)) {
            self.reported = Some((percent, muted));
            let _ = self.changes.send(VolumeChange {
                percent,
                muted,
                origin: ChangeOrigin::EndpointRebind,
            });
        }
    }

    fn on_changed(&mut self) {
        let Ok(device) = self.binding else {
            return;
        };
        let (Ok(percent), Ok(muted)) = (read_volume(device), read_mute(device)) else {
            return;
        };
        let observed = (percent, muted);
        if self.reported == Some(observed) {
            return;
        }
        let origin = classify(observed, self.reported, self.last_write, Instant::now());
        self.reported = Some(observed);
        let _ = self.changes.send(VolumeChange {
            percent,
            muted,
            origin,
        });
    }

    /// Write, then read back; a failed read-back is `ReadBackUnavailable`, never the request.
    fn set_volume(&mut self, percent: u8) -> Result<u8, BackendError> {
        let device = self.binding.clone()?;
        put(device, &VIRTUAL_MAIN_VOLUME, percent_to_scalar(percent))
            .map_err(|s| os_error("setting volume", s))?;
        let observed = read_volume(device).map_err(|_| BackendError::ReadBackUnavailable)?;
        self.last_write = Some((OwnWrite::Volume(observed), Instant::now()));
        Ok(observed)
    }

    fn set_mute(&mut self, muted: bool) -> Result<bool, BackendError> {
        let device = self.binding.clone()?;
        put(device, &MUTE, u32::from(muted)).map_err(|s| os_error("setting mute", s))?;
        let observed = read_mute(device).map_err(|_| BackendError::ReadBackUnavailable)?;
        self.last_write = Some((OwnWrite::Mute(observed), Instant::now()));
        Ok(observed)
    }
}

fn audio_thread(
    token: usize,
    commands: Receiver<Command>,
    changes: Sender<VolumeChange>,
    available: Arc<Mutex<Result<(), String>>>,
    ready: Sender<()>,
) {
    let set_available = |binding: &Result<AudioObjectId, BackendError>| {
        *available.lock().unwrap_or_else(|e| e.into_inner()) = unavailable_reason(binding);
    };

    // Process-wide and idempotent: every CoreAudio client in this process now gets HAL
    // notifications on a HAL-owned thread (the pattern SDL's CoreAudio backend uses).
    let _ = put::<*const c_void>(SYSTEM_OBJECT, &RUN_LOOP, std::ptr::null());

    let mut audio = Audio {
        token,
        binding: Err(BackendError::NoEndpoint),
        reported: None,
        last_write: None,
        changes,
    };
    // SAFETY: as in `Audio::device_listeners`; the system object always exists.
    let _ = unsafe {
        AudioObjectAddPropertyListener(
            SYSTEM_OBJECT,
            &DEFAULT_OUTPUT,
            on_default_output,
            audio.client_data(),
        )
    };
    audio.rebind();
    set_available(&audio.binding);
    let _ = ready.send(());

    loop {
        match commands.recv() {
            Ok(Command::Read(reply)) => {
                let _ = reply.send(audio.binding.clone().and_then(read_volume));
            }
            Ok(Command::Set(percent, reply)) => {
                let _ = reply.send(audio.set_volume(percent));
            }
            Ok(Command::ReadMute(reply)) => {
                let _ = reply.send(audio.binding.clone().and_then(read_mute));
            }
            Ok(Command::SetMute(muted, reply)) => {
                let _ = reply.send(audio.set_mute(muted));
            }
            Ok(Command::Changed) => audio.on_changed(),
            Ok(Command::Rebind) => {
                audio.rebind();
                set_available(&audio.binding);
            }
            Ok(Command::Shutdown) | Err(_) => break,
        }
    }

    // ORDER IS LOAD-BEARING: remove every HAL listener, THEN retire the token. A notification
    // the HAL dispatched before removal may still run; it finds no token and does nothing.
    if let Ok(device) = audio.binding {
        audio.device_listeners(device, false);
    }
    // SAFETY: removes the listener added above with the same proc and client data.
    let _ = unsafe {
        AudioObjectRemovePropertyListener(
            SYSTEM_OBJECT,
            &DEFAULT_OUTPUT,
            on_default_output,
            audio.client_data(),
        )
    };
    LISTENERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&token);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourcc_is_big_endian_ascii() {
        assert_eq!(fourcc(b"dOut"), 0x644F_7574);
        assert_eq!(fourcc(b"vmvc"), 0x766D_7663);
        assert_eq!(fourcc(b"outp"), 0x6F75_7470);
    }

    #[test]
    fn scalar_percent_round_trip_and_rounding() {
        for p in 0..=100u8 {
            assert_eq!(scalar_to_percent(percent_to_scalar(p)), p);
        }
        assert_eq!(scalar_to_percent(0.494), 49);
        assert_eq!(scalar_to_percent(0.495), 50);
        assert_eq!(scalar_to_percent(-1.0), 0);
        assert_eq!(scalar_to_percent(2.0), 100);
        assert_eq!(
            scalar_to_percent(f32::NAN),
            0,
            "NaN never escapes as garbage"
        );
        assert_eq!(percent_to_scalar(250), 1.0);
    }

    #[test]
    fn echo_is_kivori_only_for_the_written_field_within_the_window() {
        let t0 = Instant::now();
        let soon = t0 + Duration::from_millis(100);
        let late = t0 + Duration::from_millis(300);
        let prev = Some((40, false));
        let vol = Some((OwnWrite::Volume(50), t0));
        let mute = Some((OwnWrite::Mute(true), t0));

        assert_eq!(classify((50, false), prev, vol, soon), ChangeOrigin::Kivori);
        assert_eq!(
            classify((50, false), prev, vol, late),
            ChangeOrigin::External,
            "too late"
        );
        assert_eq!(
            classify((51, false), prev, vol, soon),
            ChangeOrigin::External,
            "other value"
        );
        assert_eq!(
            classify((50, true), prev, vol, soon),
            ChangeOrigin::External,
            "mute moved too"
        );
        assert_eq!(
            classify((50, false), prev, None, soon),
            ChangeOrigin::External,
            "no write"
        );
        assert_eq!(classify((40, true), prev, mute, soon), ChangeOrigin::Kivori);
        assert_eq!(
            classify((41, true), prev, mute, soon),
            ChangeOrigin::External
        );
        assert_eq!(
            classify((50, false), None, vol, soon),
            ChangeOrigin::Kivori,
            "nothing reported yet"
        );
    }
}
