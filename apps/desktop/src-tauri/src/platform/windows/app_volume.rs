//! Per-app volume and mute through WASAPI audio sessions.
//!
//! Everything here runs on the owning `kivori-audio` thread (it needs that thread's COM
//! apartment); `WindowsVolumeBackend` sends a `Command` and waits for the reply. An app is found
//! by the lowercase file name of the executable behind each session's process, the same id the
//! foreground observer reports (`spotify.exe`). One app can have several sessions (browser tabs,
//! helper processes): a write goes to every one, and it is only confirmed if every read-back
//! agrees.

use windows::core::Interface;
use windows::Win32::Media::Audio::{
    IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume,
};
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use super::foreground::{image_path, Owned};
use super::{percent_to_scalar, scalar_to_percent, FLOW, KIVORI_EVENT_CONTEXT, ROLE};
use crate::platform::AppVolumeError;

/// The lowercase file name of an image path: `C:\Apps\Spotify.exe` -> `spotify.exe`.
pub(super) fn exe_file_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

fn os(err: &windows::core::Error) -> AppVolumeError {
    AppVolumeError::Os(err.to_string())
}

/// The executable file name behind `pid`, `None` when it cannot be opened (a protected or
/// elevated process, or one that already exited).
fn process_exe(pid: u32) -> Option<String> {
    // SAFETY: plain Win32 queries; the handle is owned by an `Owned` guard that closes it.
    unsafe {
        let process = Owned(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        image_path(process.0).map(|path| exe_file_name(&path))
    }
}

/// Every session `app` has on the default render endpoint. No endpoint means no session.
fn sessions_of(
    enumerator: Option<&IMMDeviceEnumerator>,
    app: &str,
) -> Result<Vec<ISimpleAudioVolume>, AppVolumeError> {
    let enumerator = enumerator.ok_or(AppVolumeError::NoSession)?;
    // SAFETY: standard WASAPI session enumeration on the thread that owns the COM apartment;
    // every interface is a live, activated COM object. A session that cannot be inspected is
    // skipped, never an error for the others.
    let found = unsafe {
        let Ok(device) = enumerator.GetDefaultAudioEndpoint(FLOW, ROLE) else {
            return Err(AppVolumeError::NoSession);
        };
        let manager = device
            .Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)
            .map_err(|err| os(&err))?;
        let sessions = manager.GetSessionEnumerator().map_err(|err| os(&err))?;
        let count = sessions.GetCount().map_err(|err| os(&err))?;
        let mut found = Vec::new();
        for index in 0..count {
            let Ok(control) = sessions.GetSession(index) else {
                continue;
            };
            let Ok(control) = control.cast::<IAudioSessionControl2>() else {
                continue;
            };
            // Process 0 is the "system sounds" session: no app.
            let Ok(pid) = control.GetProcessId() else {
                continue;
            };
            if pid == 0 || process_exe(pid).as_deref() != Some(app) {
                continue;
            }
            if let Ok(volume) = control.cast::<ISimpleAudioVolume>() {
                found.push(volume);
            }
        }
        found
    };
    if found.is_empty() {
        Err(AppVolumeError::NoSession)
    } else {
        Ok(found)
    }
}

/// The one value every session reports, or `ReadBackUnavailable` if any could not be read or
/// they disagree (the write is then not confirmed).
fn agreed<T: PartialEq>(
    values: impl Iterator<Item = windows::core::Result<T>>,
) -> Result<T, AppVolumeError> {
    let mut agreed = None;
    for value in values {
        let value = value.map_err(|_| AppVolumeError::ReadBackUnavailable)?;
        match &agreed {
            Some(first) if *first != value => return Err(AppVolumeError::ReadBackUnavailable),
            Some(_) => {}
            None => agreed = Some(value),
        }
    }
    agreed.ok_or(AppVolumeError::ReadBackUnavailable)
}

pub(super) fn read(
    enumerator: Option<&IMMDeviceEnumerator>,
    app: &str,
) -> Result<u8, AppVolumeError> {
    let sessions = sessions_of(enumerator, app)?;
    // SAFETY: live `ISimpleAudioVolume` interfaces on the owning thread.
    let level = unsafe { sessions[0].GetMasterVolume() }.map_err(|err| os(&err))?;
    Ok(scalar_to_percent(level))
}

pub(super) fn set(
    enumerator: Option<&IMMDeviceEnumerator>,
    app: &str,
    percent: u8,
) -> Result<u8, AppVolumeError> {
    let sessions = sessions_of(enumerator, app)?;
    let level = percent_to_scalar(percent);
    for session in &sessions {
        // SAFETY: a live `ISimpleAudioVolume`; `KIVORI_EVENT_CONTEXT` is a valid `'static` GUID.
        unsafe { session.SetMasterVolume(level, &KIVORI_EVENT_CONTEXT) }.map_err(|err| os(&err))?;
    }
    // SAFETY: live `ISimpleAudioVolume` interfaces on the owning thread.
    agreed(
        sessions
            .iter()
            .map(|session| unsafe { session.GetMasterVolume() }.map(scalar_to_percent)),
    )
}

pub(super) fn read_mute(
    enumerator: Option<&IMMDeviceEnumerator>,
    app: &str,
) -> Result<bool, AppVolumeError> {
    let sessions = sessions_of(enumerator, app)?;
    // SAFETY: live `ISimpleAudioVolume` interfaces on the owning thread.
    unsafe { sessions[0].GetMute() }
        .map(|muted| muted.as_bool())
        .map_err(|err| os(&err))
}

pub(super) fn set_mute(
    enumerator: Option<&IMMDeviceEnumerator>,
    app: &str,
    muted: bool,
) -> Result<bool, AppVolumeError> {
    let sessions = sessions_of(enumerator, app)?;
    for session in &sessions {
        // SAFETY: a live `ISimpleAudioVolume`; `KIVORI_EVENT_CONTEXT` is a valid `'static` GUID.
        unsafe { session.SetMute(muted, &KIVORI_EVENT_CONTEXT) }.map_err(|err| os(&err))?;
    }
    // SAFETY: live `ISimpleAudioVolume` interfaces on the owning thread.
    agreed(
        sessions
            .iter()
            .map(|session| unsafe { session.GetMute() }.map(|muted| muted.as_bool())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exe_file_name_is_lowercase_and_has_no_path() {
        assert_eq!(
            exe_file_name(r"C:\Users\me\AppData\Roaming\Spotify\Spotify.EXE"),
            "spotify.exe"
        );
        assert_eq!(exe_file_name("vlc.exe"), "vlc.exe");
    }
}
