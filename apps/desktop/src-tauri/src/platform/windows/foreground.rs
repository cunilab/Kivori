//! Windows foreground app: `GetForegroundWindow` -> owning process -> image path and token
//! elevation; the input desktop's name when there is no foreground window. The decision itself is
//! `platform::foreground::classify_windows{,_no_window}`. Every handle is closed on every path.

use std::ffi::c_void;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_CONTROL_FLAGS,
    DESKTOP_READOBJECTS, UOI_NAME,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::platform::foreground::{classify_windows, classify_windows_no_window};
use crate::platform::{Foreground, ForegroundObserver};

pub struct WindowsForeground {
    /// Kivori's own elevation, fixed for the life of the process. Unreadable counts as not
    /// elevated, which only ever makes more surfaces Protected.
    self_elevated: bool,
}

impl WindowsForeground {
    pub fn new() -> Self {
        // SAFETY: the pseudo-handle from GetCurrentProcess needs no closing.
        let self_elevated = unsafe { elevation(GetCurrentProcess()) }.unwrap_or(false);
        Self { self_elevated }
    }
}

impl Default for WindowsForeground {
    fn default() -> Self {
        Self::new()
    }
}

impl ForegroundObserver for WindowsForeground {
    fn foreground(&self) -> Foreground {
        // SAFETY: plain Win32 queries; every handle opened below is owned by an `Owned` guard.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return classify_windows_no_window(input_desktop_name().as_deref());
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return Foreground::Unknown;
            }
            let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return Foreground::Unknown;
            };
            let process = Owned(process);
            let Some(path) = image_path(process.0) else {
                return Foreground::Unknown;
            };
            classify_windows(&path, elevation(process.0), self.self_elevated)
        }
    }
}

/// Closes a kernel handle on drop.
pub(super) struct Owned(pub(super) HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle was returned open by the OS and is closed exactly once, here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(super) unsafe fn image_path(process: HANDLE) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    QueryFullProcessImageNameW(
        process,
        PROCESS_NAME_WIN32,
        PWSTR(buf.as_mut_ptr()),
        &mut len,
    )
    .ok()?;
    Some(String::from_utf16_lossy(buf.get(..len as usize)?))
}

/// `TokenElevation` of `process`; `None` when the token can't be opened or read.
unsafe fn elevation(process: HANDLE) -> Option<bool> {
    let mut token = HANDLE::default();
    OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
    let token = Owned(token);
    let mut info = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    GetTokenInformation(
        token.0,
        TokenElevation,
        Some((&mut info as *mut TOKEN_ELEVATION).cast::<c_void>()),
        std::mem::size_of::<TOKEN_ELEVATION>() as u32,
        &mut returned,
    )
    .ok()?;
    Some(info.TokenIsElevated != 0)
}

/// Name of the desktop receiving input (`Default` normally, `Winlogon` on the secure desktop).
/// `None` when it can't be opened, which is what the secure desktop looks like from here.
unsafe fn input_desktop_name() -> Option<String> {
    let desk = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS).ok()?;
    let mut buf = [0u16; 256];
    let mut needed = 0u32;
    let read = GetUserObjectInformationW(
        HANDLE(desk.0),
        UOI_NAME,
        Some(buf.as_mut_ptr().cast::<c_void>()),
        std::mem::size_of_val(&buf) as u32,
        Some(&mut needed),
    );
    let _ = CloseDesktop(desk);
    read.ok()?;
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}
