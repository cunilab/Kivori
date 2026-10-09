//! Windows sleep/wake, lock/unlock and console changes, as [`HostEvent`]s (M3 S2).
//!
//! Two sources, both feeding the device thread's channel:
//!
//! - `PowerRegisterSuspendResumeNotification(DEVICE_NOTIFY_CALLBACK)`: callback-based, needs no
//!   window, and covers Modern Standby. On `PBT_APMSUSPEND` the callback sends `Suspending` with
//!   an ack channel and waits at most [`SUSPEND_ACK_BUDGET`], so the device's `Bye` is written
//!   before the machine suspends. The OS never waits for us beyond that.
//! - A `kivori-session` thread that owns a message-only window registered with
//!   `WTSRegisterSessionNotification(NOTIFY_FOR_THIS_SESSION)`; `WM_WTSSESSION_CHANGE` becomes
//!   lock/unlock and console disconnect/connect (fast user switching).
//!
//! Neither callback does real work: they only push onto the channel. The device thread decides.

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
    DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY,
};
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW,
    PostQuitMessage, RegisterClassW, UnregisterClassW, DEVICE_NOTIFY_CALLBACK, HMENU, HWND_MESSAGE,
    MSG, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_WTSSESSION_CHANGE, WNDCLASSW, WTS_CONSOLE_CONNECT,
    WTS_CONSOLE_DISCONNECT, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
};

use crate::platform::host_events::{
    HostEvent, HostEventsError, HostEventsGuard, HostSignal, SUSPEND_ACK_BUDGET,
};

/// Starts both sources and returns the guard that stops them.
///
/// # Errors
/// [`HostEventsError::Unavailable`] when the OS refuses either registration.
pub fn start(tx: Sender<HostSignal>) -> Result<HostEventsGuard, HostEventsError> {
    let watcher = WindowsHostEvents::start(tx)?;
    Ok(HostEventsGuard::new(move || drop(watcher)))
}

/// The registered power callback plus the session window thread. Dropping it unregisters both.
pub struct WindowsHostEvents {
    /// `HPOWERNOTIFY` from the power registration.
    power_handle: isize,
    /// The boxed [`PowerContext`] the power callback reads, freed after unregistering.
    power_context: *mut PowerContext,
    /// The boxed subscribe parameters; the registration may keep reading them.
    power_params: *mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS,
    /// The message-only window, as an integer so this type stays `Send`.
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

// SAFETY: the raw pointers are owned boxes touched only in `start` and `Drop`; the OS callback
// reads `power_context` (immutable, `Send` payload) until `Drop` unregisters it.
unsafe impl Send for WindowsHostEvents {}

struct PowerContext {
    tx: Sender<HostSignal>,
}

impl WindowsHostEvents {
    /// Registers for suspend/resume and starts the session window thread.
    ///
    /// # Errors
    /// [`HostEventsError::Unavailable`] with the OS reason; nothing stays registered on failure.
    pub fn start(tx: Sender<HostSignal>) -> Result<Self, HostEventsError> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let session_tx = tx.clone();
        let thread = std::thread::Builder::new()
            .name("kivori-session".to_string())
            .spawn(move || session_thread(session_tx, &ready_tx))
            .map_err(|error| HostEventsError::Unavailable(error.to_string()))?;
        let hwnd = match ready_rx.recv() {
            Ok(Ok(hwnd)) => hwnd,
            Ok(Err(reason)) => {
                let _ = thread.join();
                return Err(HostEventsError::Unavailable(reason));
            }
            Err(_) => {
                return Err(HostEventsError::Unavailable(
                    "session thread exited".to_string(),
                ))
            }
        };

        let power_context = Box::into_raw(Box::new(PowerContext { tx }));
        let power_params = Box::into_raw(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(on_power),
            Context: power_context.cast::<c_void>(),
        }));
        let mut handle: *mut c_void = std::ptr::null_mut();
        // SAFETY: with `DEVICE_NOTIFY_CALLBACK` the recipient is a pointer to the subscribe
        // parameters, which (like the context) stay allocated until `Drop` unregisters.
        let status = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                HANDLE(power_params.cast::<c_void>()),
                &mut handle,
            )
        };
        let mut this = Self {
            power_handle: handle as isize,
            power_context,
            power_params,
            hwnd,
            thread: Some(thread),
        };
        if status != ERROR_SUCCESS {
            this.power_handle = 0;
            // `Drop` closes the window thread and frees the boxes.
            return Err(HostEventsError::Unavailable(format!(
                "PowerRegisterSuspendResumeNotification failed ({})",
                status.0
            )));
        }
        Ok(this)
    }

    /// The message-only window that receives `WM_WTSSESSION_CHANGE` (an `HWND` as an integer);
    /// tests post a synthetic session change to it.
    #[must_use]
    pub const fn window_handle(&self) -> isize {
        self.hwnd
    }
}

impl Drop for WindowsHostEvents {
    fn drop(&mut self) {
        // SAFETY: each handle is released at most once, and the boxes are freed only after the
        // OS can no longer call into them (unregister returns only after in-flight callbacks).
        unsafe {
            if self.power_handle != 0 {
                let _ = PowerUnregisterSuspendResumeNotification(HPOWERNOTIFY(self.power_handle));
            }
            drop(Box::from_raw(self.power_params));
            drop(Box::from_raw(self.power_context));
            let _ = PostMessageW(
                HWND(self.hwnd as *mut c_void),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            );
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The power callback: runs on a system thread, so it only talks to the channel.
unsafe extern "system" fn on_power(
    context: *const c_void,
    kind: u32,
    _setting: *const c_void,
) -> u32 {
    // SAFETY: `context` is the boxed `PowerContext`, alive until unregistration completes.
    let context = &*context.cast::<PowerContext>();
    match kind {
        PBT_APMSUSPEND => {
            let (ack_tx, ack_rx) = mpsc::sync_channel(1);
            let sent = context
                .tx
                .send(HostSignal {
                    event: HostEvent::Suspending,
                    ack: Some(ack_tx),
                })
                .is_ok();
            if sent {
                // A late or missing ack must never hold the machine awake past the budget.
                let _ = ack_rx.recv_timeout(SUSPEND_ACK_BUDGET);
            }
        }
        // Windows may send both on one wake; the tracker ignores the repeat.
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
            let _ = context.tx.send(HostSignal::new(HostEvent::Resumed));
        }
        _ => {}
    }
    0
}

/// Maps a `WM_WTSSESSION_CHANGE` `wParam` to the event it means, if any.
#[must_use]
pub const fn session_event(code: u32) -> Option<HostEvent> {
    match code {
        WTS_SESSION_LOCK => Some(HostEvent::Locked),
        WTS_SESSION_UNLOCK => Some(HostEvent::Unlocked),
        WTS_CONSOLE_DISCONNECT => Some(HostEvent::ConsoleDisconnected),
        WTS_CONSOLE_CONNECT => Some(HostEvent::ConsoleConnected),
        _ => None,
    }
}

thread_local! {
    /// The window procedure runs on the `kivori-session` thread, which owns this sender.
    static SINK: RefCell<Option<Sender<HostSignal>>> = const { RefCell::new(None) };
}

unsafe extern "system" fn session_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_WTSSESSION_CHANGE => {
            if let Some(event) = u32::try_from(wparam.0).ok().and_then(session_event) {
                SINK.with(|sink| {
                    if let Some(tx) = sink.borrow().as_ref() {
                        let _ = tx.send(HostSignal::new(event));
                    }
                });
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

/// Creates the message-only window, registers for session notices and pumps messages until the
/// window is closed. Reports the window (or why there is none) on `ready` exactly once.
fn session_thread(tx: Sender<HostSignal>, ready: &Sender<Result<isize, String>>) {
    SINK.with(|sink| *sink.borrow_mut() = Some(tx));
    let class: PCWSTR = w!("KivoriHostEvents");
    // SAFETY: plain Win32 window lifecycle on this thread; every handle created here is
    // released here, and `ready` is answered on every path.
    unsafe {
        let hinstance = match GetModuleHandleW(PCWSTR::null()) {
            Ok(module) => HINSTANCE(module.0),
            Err(error) => {
                let _ = ready.send(Err(format!("GetModuleHandleW failed: {error}")));
                return;
            }
        };
        let class_info = WNDCLASSW {
            lpfnWndProc: Some(session_proc),
            hInstance: hinstance,
            lpszClassName: class,
            ..Default::default()
        };
        // Zero means failure (or already registered); a missing class fails the next call.
        RegisterClassW(&class_info);
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("Kivori host events"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            HMENU::default(),
            hinstance,
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                let _ = UnregisterClassW(class, hinstance);
                let _ = ready.send(Err(format!("CreateWindowExW failed: {error}")));
                return;
            }
        };
        if let Err(error) = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) {
            let _ = DestroyWindow(hwnd);
            let _ = UnregisterClassW(class, hinstance);
            let _ = ready.send(Err(format!(
                "WTSRegisterSessionNotification failed: {error}"
            )));
            return;
        }
        let _ = ready.send(Ok(hwnd.0 as isize));

        let mut message = MSG::default();
        while GetMessageW(&mut message, HWND::default(), 0, 0).0 > 0 {
            DispatchMessageW(&message);
        }
        let _ = WTSUnRegisterSessionNotification(hwnd);
        let _ = UnregisterClassW(class, hinstance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_codes_map_to_events() {
        assert_eq!(session_event(WTS_SESSION_LOCK), Some(HostEvent::Locked));
        assert_eq!(session_event(WTS_SESSION_UNLOCK), Some(HostEvent::Unlocked));
        assert_eq!(
            session_event(WTS_CONSOLE_DISCONNECT),
            Some(HostEvent::ConsoleDisconnected)
        );
        assert_eq!(
            session_event(WTS_CONSOLE_CONNECT),
            Some(HostEvent::ConsoleConnected)
        );
        // WTS_SESSION_LOGON and friends are not host presence.
        assert_eq!(session_event(5), None);
    }
}
