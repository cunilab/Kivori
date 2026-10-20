//! Windows running apps and app icons. Running apps: the processes that own a visible, titled,
//! un-owned top-level window (what the taskbar lists), classified like the foreground app. Icons:
//! `ExtractIconExW` of the running process's executable, its pixels read with `GetDIBits` and
//! re-encoded as PNG. Both run on a worker thread, never the device thread. Every handle and GDI
//! object is released on every path.

use std::ffi::c_void;

use windows::core::BOOL;
use windows::core::HSTRING;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::Graphics::Gdi::{
    GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GetIconInfo, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, GW_OWNER, HICON, ICONINFO,
    WS_EX_TOOLWINDOW,
};

use super::foreground::{elevation, image_path, Owned};
use crate::platform::apps::RunningApp;
use crate::platform::foreground::{classify_windows, is_own_app};
use crate::platform::png::encode_rgba;
use crate::platform::Foreground;

unsafe extern "system" fn collect_pid(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `Vec<u32>` `window_processes` passed, alive for the whole call.
    let pids = &mut *(lparam.0 as *mut Vec<u32>);
    let tool_window = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0;
    // A window with an owner is a dialog or popup of another window; GetWindow fails for none.
    if IsWindowVisible(hwnd).as_bool()
        && GetWindowTextLengthW(hwnd) > 0
        && !tool_window
        && GetWindow(hwnd, GW_OWNER).is_err()
    {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 && !pids.contains(&pid) {
            pids.push(pid);
        }
    }
    BOOL(1)
}

fn window_processes() -> Vec<u32> {
    let mut pids: Vec<u32> = Vec::new();
    // SAFETY: the callback only runs during this call and `pids` outlives it.
    unsafe {
        let _ = EnumWindows(
            Some(collect_pid),
            LPARAM((&mut pids as *mut Vec<u32>) as isize),
        );
    }
    pids
}

/// The apps with a window, with the full path of each executable.
fn apps_with_paths() -> Vec<(RunningApp, String)> {
    // SAFETY: the pseudo-handle from GetCurrentProcess needs no closing.
    let self_elevated = unsafe { elevation(GetCurrentProcess()) }.unwrap_or(false);
    let mut out = Vec::new();
    for pid in window_processes() {
        // SAFETY: the handle is owned by an `Owned` guard and closed on every path.
        let found = unsafe {
            let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                continue;
            };
            let process = Owned(process);
            image_path(process.0).map(|path| {
                let class = classify_windows(&path, elevation(process.0), self_elevated);
                (class, path)
            })
        };
        if let Some((Foreground::App { id, name }, path)) = found {
            if !is_own_app(&id) {
                out.push((RunningApp { id, name }, path));
            }
        }
    }
    out
}

pub fn running_apps() -> Vec<RunningApp> {
    apps_with_paths().into_iter().map(|(app, _)| app).collect()
}

/// The icon of the running app whose executable is `id`, as PNG bytes.
pub fn icon_png(id: &str) -> Option<Vec<u8>> {
    let (_, path) = apps_with_paths()
        .into_iter()
        .find(|(app, _)| app.id == id)?;
    let mut large = HICON::default();
    // SAFETY: one large icon is requested into `large`; it is destroyed below.
    let (w, h, rgba) = unsafe {
        let got = ExtractIconExW(&HSTRING::from(path.as_str()), 0, Some(&mut large), None, 1);
        if got == 0 || large.is_invalid() {
            return None;
        }
        let pixels = icon_rgba(large);
        let _ = DestroyIcon(large);
        pixels?
    };
    encode_rgba(w, h, &rgba)
}

/// One bitmap's pixels as top-down 32-bit BGRA, with its size.
unsafe fn bitmap_bgra(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut bm = BITMAP::default();
    let read = GetObjectW(
        HGDIOBJ(bitmap.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some((&mut bm as *mut BITMAP).cast::<c_void>()),
    );
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    if read == 0 || w <= 0 || h <= 0 || w > 512 || h > 512 {
        return None;
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            // Negative: rows run top to bottom.
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..BITMAPINFOHEADER::default()
        },
        ..BITMAPINFO::default()
    };
    let mut pixels = vec![0u8; (w as usize) * (h as usize) * 4];
    let dc = GetDC(None);
    let lines = GetDIBits(
        dc,
        bitmap,
        0,
        h as u32,
        Some(pixels.as_mut_ptr().cast::<c_void>()),
        &mut info,
        DIB_RGB_COLORS,
    );
    ReleaseDC(None, dc);
    (lines != 0).then_some((w as u32, h as u32, pixels))
}

/// The icon's pixels as RGBA. An icon without an alpha channel takes its transparency from the
/// mask bitmap.
unsafe fn icon_rgba(icon: HICON) -> Option<(u32, u32, Vec<u8>)> {
    let mut info = ICONINFO::default();
    GetIconInfo(icon, &mut info).ok()?;
    let color = bitmap_bgra(info.hbmColor);
    let mask = bitmap_bgra(info.hbmMask);
    for bitmap in [info.hbmColor, info.hbmMask] {
        if !bitmap.is_invalid() {
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(HGDIOBJ(bitmap.0));
        }
    }
    let (w, h, mut px) = color?;
    let has_alpha = px.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
    for (i, p) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        p.swap(0, 2);
        if !has_alpha {
            // A black mask pixel is opaque, a white one transparent.
            let opaque = mask
                .as_ref()
                .is_none_or(|(mw, mh, m)| (*mw, *mh) != (w, h) || m[i * 4] == 0);
            p[3] = if opaque { 255 } else { 0 };
        }
    }
    Some((w, h, px))
}
