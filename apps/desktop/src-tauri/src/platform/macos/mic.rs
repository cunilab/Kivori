//! macOS CoreAudio microphone mute on EVERY active capture device (ADR-0011).
//!
//! MUTE: `kAudioDevicePropertyMute` ('mute', input scope, main element), written per device and
//! read back per device. A device with no settable mute property (many built-in and virtual
//! microphones have none) is reported `Unsettable` and the action fails for it: there is no quiet
//! switch to input volume 0, which is not mute and which automatic gain can raise again
//! (invariant 19).
//! ponytail: no per-channel mute fallback; add it when a device with channel-only mute shows up.
//!
//! WHICH DEVICES: every device on `kAudioHardwarePropertyDevices` with at least one input stream,
//! except aggregate devices (apps create transient ones such as `CADefaultDeviceAggregate`; their
//! sub-devices are listed on their own) and hidden ones. A device that arrives while mic mute is
//! on is muted and read back too; if that fails it reads live, so the state says live.
//!
//! IN USE: any process object other than Kivori's own (`kAudioHardwarePropertyProcessObjectList`)
//! with `kAudioProcessPropertyIsRunningInput` set. A headset that is only PLAYING audio does not
//! count. PER-DEVICE CAPTURE (`MicDevice::capturing`): the devices a running-input process lists
//! on `kAudioProcessPropertyDevices` with the input scope (an aggregate expands to its active
//! sub-devices). Without the process API (property missing, or any read fails) it falls back to
//! `kAudioDevicePropertyDeviceIsRunningSomewhere`, trusted only on input-only devices (on a duplex
//! one it cannot tell playing from recording): there in use ignores them and capturing is `None`.
//! Capture start is not pushed: the desk re-reads `devices()` on its 1 s tick, which is prompt
//! enough and avoids a listener per process object that must follow processes coming and going.
//!
//! THREADING and LISTENER LIFETIME mirror `audio.rs`: one `kivori-mic` thread owns all state and
//! makes every CoreAudio call; HAL listeners only post a command, with an opaque token that is
//! looked up in [`LISTENERS`] and never dereferenced; Drop removes every listener before the
//! token.

use std::collections::BTreeMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::audio::{
    fourcc, get, put, AudioObjectAddPropertyListener, AudioObjectGetPropertyData,
    AudioObjectHasProperty, AudioObjectId, AudioObjectIsPropertySettable,
    AudioObjectRemovePropertyListener, OsStatus, PropertyAddress, ELEMENT_MAIN, RUN_LOOP,
    SCOPE_GLOBAL, SCOPE_OUTPUT, SYSTEM_OBJECT,
};
use crate::platform::{
    ActionAvailability, BackendError, ChangeOrigin, ConfirmationClass, MicBackend, MicChange,
    MicDevice, MicMute,
};

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        id: AudioObjectId,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        out_size: *mut u32,
    ) -> OsStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetCString(
        string: *const c_void,
        buffer: *mut c_char,
        size: isize,
        encoding: u32,
    ) -> u8;
    fn CFRelease(object: *const c_void);
}

const CF_UTF8: u32 = 0x0800_0100;
const SCOPE_INPUT: u32 = fourcc(b"inpt");
/// `kAudioDeviceTransportTypeAggregate`.
const TRANSPORT_AGGREGATE: u32 = fourcc(b"grup");

const fn address(selector: &[u8; 4], scope: u32) -> PropertyAddress {
    PropertyAddress {
        selector: fourcc(selector),
        scope,
        element: ELEMENT_MAIN,
    }
}

const DEVICES: PropertyAddress = address(b"dev#", SCOPE_GLOBAL);
const INPUT_STREAMS: PropertyAddress = address(b"stm#", SCOPE_INPUT);
const INPUT_MUTE: PropertyAddress = address(b"mute", SCOPE_INPUT);
const NAME: PropertyAddress = address(b"lnam", SCOPE_GLOBAL);
const TRANSPORT: PropertyAddress = address(b"tran", SCOPE_GLOBAL);
const HIDDEN: PropertyAddress = address(b"hidn", SCOPE_GLOBAL);
const RUNNING_SOMEWHERE: PropertyAddress = address(b"gone", SCOPE_GLOBAL);
const OUTPUT_STREAMS: PropertyAddress = address(b"stm#", SCOPE_OUTPUT);
/// `kAudioAggregateDevicePropertyActiveSubDeviceList`.
const AGGREGATE_SUB_DEVICES: PropertyAddress = address(b"agrp", SCOPE_GLOBAL);
/// `kAudioHardwarePropertyProcessObjectList`.
const PROCESSES: PropertyAddress = address(b"prs#", SCOPE_GLOBAL);
/// `kAudioProcessPropertyPID`.
const PROCESS_PID: PropertyAddress = address(b"ppid", SCOPE_GLOBAL);
/// `kAudioProcessPropertyIsRunningInput`.
const PROCESS_RUNNING_INPUT: PropertyAddress = address(b"piri", SCOPE_GLOBAL);
/// `kAudioProcessPropertyDevices`, input scope: the devices the process captures from.
const PROCESS_INPUT_DEVICES: PropertyAddress = address(b"pdv#", SCOPE_INPUT);

/// How long after an own write a notification still counts as its echo.
const ECHO_WINDOW: Duration = Duration::from_millis(250);

enum Command {
    Devices(Sender<Result<Vec<MicDevice>, BackendError>>),
    SetMute(bool, Sender<Result<Vec<MicDevice>, BackendError>>),
    InUse(Sender<Option<bool>>),
    /// A mute listener fired on a capture device.
    Changed,
    /// The set of audio devices changed.
    DevicesChanged,
    Shutdown,
}

/// Token -> command sender for every live backend (see `audio.rs`).
static LISTENERS: Mutex<BTreeMap<usize, Sender<Command>>> = Mutex::new(BTreeMap::new());
static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(1);

fn post(token: usize, command: Command) {
    let map = LISTENERS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tx) = map.get(&token) {
        let _ = tx.send(command);
    }
}

/// HAL listener for a capture device's mute. INGRESS ONLY.
unsafe extern "C" fn on_device_property(
    _id: AudioObjectId,
    _count: u32,
    _addresses: *const PropertyAddress,
    client_data: *mut c_void,
) -> OsStatus {
    post(client_data as usize, Command::Changed);
    0
}

/// HAL listener for the system object's device list. INGRESS ONLY.
unsafe extern "C" fn on_devices(
    _id: AudioObjectId,
    _count: u32,
    _addresses: *const PropertyAddress,
    client_data: *mut c_void,
) -> OsStatus {
    post(client_data as usize, Command::DevicesChanged);
    0
}

pub struct MacMicBackend {
    commands: Sender<Command>,
    changes: Mutex<Receiver<MicChange>>,
    /// `Err(reason)` while there is no capture device.
    available: Arc<Mutex<Result<(), String>>>,
    thread: Option<JoinHandle<()>>,
}

impl MacMicBackend {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<Command>();
        let (change_tx, change_rx) = mpsc::channel::<MicChange>();
        let available = Arc::new(Mutex::new(Err("audio thread not started".to_string())));
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        LISTENERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(token, command_tx.clone());

        let thread_available = Arc::clone(&available);
        let thread = std::thread::Builder::new()
            .name("kivori-mic".to_string())
            .spawn(move || mic_thread(token, command_rx, change_tx, thread_available, ready_tx))
            .expect("spawn kivori-mic thread");
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

impl Default for MacMicBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MacMicBackend {
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

impl MicBackend for MacMicBackend {
    fn availability(&self) -> ActionAvailability {
        match &*self.available.lock().unwrap_or_else(|e| e.into_inner()) {
            Ok(()) => ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            Err(reason) => ActionAvailability::RuntimeUnavailable {
                reason: reason.clone(),
            },
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
        self.call(Command::InUse).flatten()
    }

    fn try_recv_change(&self) -> Option<MicChange> {
        self.changes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_recv()
            .ok()
    }
}

/// The object ids an array property of `object` holds.
fn object_ids(
    object: AudioObjectId,
    address: &PropertyAddress,
) -> Result<Vec<AudioObjectId>, OsStatus> {
    let mut size = 0u32;
    // SAFETY: valid address and out pointer; the caller passes an object the HAL returned.
    let status =
        unsafe { AudioObjectGetPropertyDataSize(object, address, 0, std::ptr::null(), &mut size) };
    if status != 0 {
        return Err(status);
    }
    let mut ids = vec![0 as AudioObjectId; size as usize / std::mem::size_of::<AudioObjectId>()];
    let mut size = (ids.len() * std::mem::size_of::<AudioObjectId>()) as u32;
    // SAFETY: `ids` is a writable buffer of exactly `size` bytes.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            address,
            0,
            std::ptr::null(),
            &mut size,
            ids.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        return Err(status);
    }
    ids.truncate(size as usize / std::mem::size_of::<AudioObjectId>());
    Ok(ids)
}

/// The ids of every device the HAL knows.
fn all_device_ids() -> Result<Vec<AudioObjectId>, OsStatus> {
    object_ids(SYSTEM_OBJECT, &DEVICES)
}

fn has_streams(device: AudioObjectId, streams: &PropertyAddress) -> bool {
    let mut size = 0u32;
    // SAFETY: valid address and out pointer.
    let status =
        unsafe { AudioObjectGetPropertyDataSize(device, streams, 0, std::ptr::null(), &mut size) };
    status == 0 && size > 0
}

fn has_input(device: AudioObjectId) -> bool {
    has_streams(device, &INPUT_STREAMS)
}

/// Every active capture device: it has an input stream and is neither aggregate nor hidden.
fn capture_devices() -> Result<Vec<AudioObjectId>, BackendError> {
    let ids = all_device_ids()
        .map_err(|s| BackendError::Os(format!("listing audio devices failed (OSStatus {s})")))?;
    Ok(ids
        .into_iter()
        .filter(|&id| {
            has_input(id)
                && get::<u32>(id, &TRANSPORT) != Ok(TRANSPORT_AGGREGATE)
                && get::<u32>(id, &HIDDEN) != Ok(1)
        })
        .collect())
}

/// The device's user-visible name (user data: shown, never logged).
fn device_name(device: AudioObjectId) -> String {
    let fallback = || format!("Microphone {device}");
    let Ok(string) = get::<usize>(device, &NAME) else {
        return fallback();
    };
    if string == 0 {
        return fallback();
    }
    let mut buffer = [0 as c_char; 256];
    // SAFETY: `string` is the +1 retained CFString the HAL just returned; the buffer length is
    // passed; it is released exactly once below.
    let ok = unsafe {
        let ok = CFStringGetCString(
            string as *const c_void,
            buffer.as_mut_ptr(),
            buffer.len() as isize,
            CF_UTF8,
        );
        CFRelease(string as *const c_void);
        ok
    };
    if ok == 0 {
        return fallback();
    }
    // SAFETY: `CFStringGetCString` NUL-terminated the buffer on success.
    let name = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    if name.is_empty() {
        fallback()
    } else {
        name
    }
}

fn mute_settable(device: AudioObjectId) -> bool {
    let mut settable = 0u8;
    // SAFETY: `device` came from the HAL; both pointers are valid.
    let has = unsafe { AudioObjectHasProperty(device, &INPUT_MUTE) } != 0;
    has && unsafe { AudioObjectIsPropertySettable(device, &INPUT_MUTE, &mut settable) } == 0
        && settable != 0
}

/// The device's mute as it reads right now.
fn read_mute(device: AudioObjectId) -> MicMute {
    if !mute_settable(device) {
        return MicMute::Unsettable;
    }
    match get::<u32>(device, &INPUT_MUTE) {
        Ok(0) => MicMute::Live,
        Ok(_) => MicMute::Muted,
        Err(_) => MicMute::Unreadable,
    }
}

/// Writes the device's mute, then reads it back. A failed write is left to the read-back to
/// report (it then still reads live), so the result is always what the OS says.
fn write_mute(device: AudioObjectId, muted: bool) -> MicMute {
    if mute_settable(device) {
        let _ = put(device, &INPUT_MUTE, u32::from(muted));
    }
    read_mute(device)
}

/// The devices some process other than Kivori is recording from, and whether any is: `None` when
/// the process objects cannot be read (the API is missing, or a process vanished mid-read), which
/// is unknown, never "idle".
struct Capture {
    any: bool,
    devices: Vec<AudioObjectId>,
}

fn capture_scan() -> Option<Capture> {
    let own = std::process::id();
    let mut capture = Capture {
        any: false,
        devices: Vec::new(),
    };
    for process in object_ids(SYSTEM_OBJECT, &PROCESSES).ok()? {
        if get::<i32>(process, &PROCESS_PID).ok()? as u32 == own {
            continue;
        }
        if get::<u32>(process, &PROCESS_RUNNING_INPUT).ok()? == 0 {
            continue;
        }
        capture.any = true;
        for device in object_ids(process, &PROCESS_INPUT_DEVICES).ok()? {
            // A process on an aggregate device records from its sub-devices.
            match object_ids(device, &AGGREGATE_SUB_DEVICES) {
                Ok(subs) if get::<u32>(device, &TRANSPORT) == Ok(TRANSPORT_AGGREGATE) => {
                    capture.devices.extend(subs);
                }
                _ => {}
            }
            capture.devices.push(device);
        }
    }
    Some(capture)
}

/// `kAudioDevicePropertyDeviceIsRunningSomewhere`: true while ANY process runs IO on the device.
fn running_somewhere(device: AudioObjectId) -> Option<bool> {
    get::<u32>(device, &RUNNING_SOMEWHERE).ok().map(|v| v != 0)
}

/// Per device: is a process recording from it (see the module docs for the fallback).
fn capturing_of(ids: &[AudioObjectId]) -> Vec<Option<bool>> {
    match capture_scan() {
        Some(capture) => ids
            .iter()
            .map(|id| Some(capture.devices.contains(id)))
            .collect(),
        None => ids
            .iter()
            .map(|&id| {
                if has_streams(id, &OUTPUT_STREAMS) {
                    None
                } else {
                    running_somewhere(id)
                }
            })
            .collect(),
    }
}

fn snapshot(ids: &[AudioObjectId]) -> Vec<MicDevice> {
    let capturing = capturing_of(ids);
    ids.iter()
        .zip(capturing)
        .map(|(&id, capturing)| MicDevice {
            name: device_name(id),
            mute: read_mute(id),
            capturing,
        })
        .collect()
}

fn in_use_of(ids: &[AudioObjectId]) -> Option<bool> {
    if let Some(capture) = capture_scan() {
        return Some(capture.any);
    }
    // No process objects: only an input-only device's "running somewhere" means capture.
    let mut any_known = false;
    for &id in ids.iter().filter(|&&id| !has_streams(id, &OUTPUT_STREAMS)) {
        match running_somewhere(id) {
            Some(false) => any_known = true,
            Some(true) => return Some(true),
            // One unreadable device means "not running" cannot be claimed for all of them.
            None => return None,
        }
    }
    any_known.then_some(false)
}

/// State owned by the `kivori-mic` thread.
struct Mic {
    token: usize,
    /// Capture devices that carry a mute listener.
    listening: Vec<AudioObjectId>,
    /// Mic mute is on: a device that arrives is muted too.
    hold_muted: bool,
    /// Last device state sent on `changes`; notifications that do not change it are dropped.
    reported: Option<Vec<MicDevice>>,
    last_write: Option<Instant>,
    changes: Sender<MicChange>,
}

impl Mic {
    fn client_data(&self) -> *mut c_void {
        self.token as *mut c_void
    }

    fn listen(&self, device: AudioObjectId, add: bool) {
        // SAFETY: valid address and an `extern "C"` proc with the HAL's signature; the client
        // data is an opaque token, never dereferenced. A device that vanished just errors.
        let _ = unsafe {
            if add {
                AudioObjectAddPropertyListener(
                    device,
                    &INPUT_MUTE,
                    on_device_property,
                    self.client_data(),
                )
            } else {
                AudioObjectRemovePropertyListener(
                    device,
                    &INPUT_MUTE,
                    on_device_property,
                    self.client_data(),
                )
            }
        };
    }

    /// Aligns the listeners with the current capture devices, muting any that arrived while mic
    /// mute is on, and returns the current ids.
    fn rescan(&mut self) -> Result<Vec<AudioObjectId>, BackendError> {
        let current = capture_devices()?;
        let gone: Vec<_> = self
            .listening
            .iter()
            .copied()
            .filter(|id| !current.contains(id))
            .collect();
        for id in gone {
            self.listen(id, false);
        }
        for &id in &current {
            if !self.listening.contains(&id) {
                self.listen(id, true);
                if self.hold_muted {
                    let _ = write_mute(id, true);
                    self.last_write = Some(Instant::now());
                }
            }
        }
        self.listening = current.clone();
        Ok(current)
    }

    fn devices(&mut self) -> Result<Vec<MicDevice>, BackendError> {
        let ids = self.rescan()?;
        Ok(snapshot(&ids))
    }

    fn set_mute(&mut self, muted: bool) -> Result<Vec<MicDevice>, BackendError> {
        let ids = self.rescan()?;
        if ids.is_empty() {
            return Err(BackendError::NoEndpoint);
        }
        self.hold_muted = muted;
        let mutes = ids
            .iter()
            .map(|&id| write_mute(id, muted))
            .collect::<Vec<_>>();
        let devices = ids
            .iter()
            .zip(mutes)
            .zip(capturing_of(&ids))
            .map(|((&id, mute), capturing)| MicDevice {
                name: device_name(id),
                mute,
                capturing,
            })
            .collect::<Vec<_>>();
        self.last_write = Some(Instant::now());
        self.reported = Some(devices.clone());
        Ok(devices)
    }

    fn publish(&mut self, origin: ChangeOrigin) {
        let Ok(ids) = self.rescan() else {
            return;
        };
        let devices = snapshot(&ids);
        if self.reported.as_ref() == Some(&devices) {
            return;
        }
        let origin = match origin {
            ChangeOrigin::External
                if self
                    .last_write
                    .is_some_and(|at| at.elapsed() <= ECHO_WINDOW) =>
            {
                ChangeOrigin::Kivori
            }
            other => other,
        };
        self.reported = Some(devices.clone());
        let _ = self.changes.send(MicChange { devices, origin });
    }
}

fn mic_thread(
    token: usize,
    commands: Receiver<Command>,
    changes: Sender<MicChange>,
    available: Arc<Mutex<Result<(), String>>>,
    ready: Sender<()>,
) {
    // Process-wide and idempotent; see `audio.rs`.
    let _ = put::<*const c_void>(SYSTEM_OBJECT, &RUN_LOOP, std::ptr::null());

    let mut mic = Mic {
        token,
        listening: Vec::new(),
        hold_muted: false,
        reported: None,
        last_write: None,
        changes,
    };
    // SAFETY: as in `Mic::listen`; the system object always exists.
    let _ = unsafe {
        AudioObjectAddPropertyListener(SYSTEM_OBJECT, &DEVICES, on_devices, mic.client_data())
    };
    let set_available = |has_devices: bool| {
        *available.lock().unwrap_or_else(|e| e.into_inner()) = if has_devices {
            Ok(())
        } else {
            Err("no microphone".to_string())
        };
    };
    set_available(mic.rescan().is_ok_and(|ids| !ids.is_empty()));
    let _ = ready.send(());

    loop {
        match commands.recv() {
            Ok(Command::Devices(reply)) => {
                let result = mic.devices();
                set_available(result.as_ref().is_ok_and(|d| !d.is_empty()));
                let _ = reply.send(result);
            }
            Ok(Command::SetMute(muted, reply)) => {
                let _ = reply.send(mic.set_mute(muted));
            }
            Ok(Command::InUse(reply)) => {
                let _ = reply.send(mic.rescan().ok().and_then(|ids| in_use_of(&ids)));
            }
            Ok(Command::Changed) => mic.publish(ChangeOrigin::External),
            Ok(Command::DevicesChanged) => {
                mic.publish(ChangeOrigin::EndpointRebind);
                set_available(!mic.listening.is_empty());
            }
            Ok(Command::Shutdown) | Err(_) => break,
        }
    }

    // ORDER IS LOAD-BEARING: remove every HAL listener, THEN retire the token (see `audio.rs`).
    for id in std::mem::take(&mut mic.listening) {
        mic.listen(id, false);
    }
    // SAFETY: removes the listener added above with the same proc and client data.
    let _ = unsafe {
        AudioObjectRemovePropertyListener(SYSTEM_OBJECT, &DEVICES, on_devices, mic.client_data())
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
    fn selectors_are_big_endian_ascii() {
        assert_eq!(DEVICES.selector, 0x6465_7623);
        assert_eq!(INPUT_MUTE.selector, 0x6D75_7465);
        assert_eq!(INPUT_MUTE.scope, 0x696E_7074);
        assert_eq!(RUNNING_SOMEWHERE.selector, 0x676F_6E65);
        assert_eq!(PROCESSES.selector, 0x7072_7323);
        assert_eq!(PROCESS_RUNNING_INPUT.selector, 0x7069_7269);
        assert_eq!(PROCESS_INPUT_DEVICES.selector, 0x7064_7623);
        assert_eq!(PROCESS_INPUT_DEVICES.scope, 0x696E_7074);
    }

    /// READ-ONLY real OS calls (never mutes): `cargo test -p kivori-desktop -- --ignored
    /// --nocapture mac_mics_now`.
    #[test]
    #[ignore = "reads the live capture devices, their mute and who is recording"]
    fn mac_mics_now() {
        let mic = MacMicBackend::new();
        for d in mic.devices().expect("devices") {
            println!("{d:?}");
        }
        println!("in use: {:?}", mic.in_use());
        println!(
            "process scan: {:?}",
            capture_scan().map(|c| (c.any, c.devices))
        );
    }
}
