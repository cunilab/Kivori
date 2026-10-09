//! macOS sleep/wake and lock/unlock, as [`HostEvent`]s (M3 S2).
//!
//! Sleep comes from IOKit's `IORegisterForSystemPower` (hand-declared C FFI, like the CoreAudio
//! code): a `kivori-power` thread owns the notification port and runs its CFRunLoop. On
//! `kIOMessageSystemWillSleep` the callback sends `Suspending` with an ack channel, waits at most
//! [`SUSPEND_ACK_BUDGET`] for the device thread to write its goodbye, then calls
//! `IOAllowPowerChange` so the Mac can sleep. NSWorkspace observers were rejected: they need
//! runtime-built ObjC classes or blocks and this crate has no objc dependency.
//!
//! Lock has no IOKit notification, so a `kivori-lock` thread polls `CGSSessionScreenIsLocked`
//! (the same read the foreground classifier uses) at 4 Hz and reports the edges.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::time::Duration;

use super::foreground::screen_locked;
use crate::platform::host_events::{
    lock_edge, HostEvent, HostEventsError, HostEventsGuard, HostSignal, SUSPEND_ACK_BUDGET,
};

type CfRef = *const c_void;
/// `mach_port_t` and its aliases (`io_connect_t`, `io_object_t`).
type MachPort = u32;
type IoNotificationPortRef = *mut c_void;
type IoServiceInterestCallback =
    unsafe extern "C" fn(refcon: *mut c_void, service: u32, message: u32, argument: *mut c_void);

// `iokit_common_msg(x)` = `sys_iokit | sub_iokit_common | x` = `0xE000_0000 | x`.
const K_IO_MESSAGE_CAN_SYSTEM_SLEEP: u32 = 0xE000_0270;
const K_IO_MESSAGE_SYSTEM_WILL_SLEEP: u32 = 0xE000_0280;
const K_IO_MESSAGE_SYSTEM_HAS_POWERED_ON: u32 = 0xE000_0300;

/// Lock polling rate (4 Hz).
const LOCK_POLL: Duration = Duration::from_millis(250);

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IORegisterForSystemPower(
        refcon: *mut c_void,
        port: *mut IoNotificationPortRef,
        callback: IoServiceInterestCallback,
        notifier: *mut MachPort,
    ) -> MachPort;
    fn IODeregisterForSystemPower(notifier: *mut MachPort) -> i32;
    fn IOAllowPowerChange(kernel_port: MachPort, notification_id: isize) -> i32;
    fn IOServiceClose(connect: MachPort) -> i32;
    fn IONotificationPortGetRunLoopSource(port: IoNotificationPortRef) -> CfRef;
    fn IONotificationPortDestroy(port: IoNotificationPortRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopCommonModes: CfRef;
    static kCFRunLoopDefaultMode: CfRef;
    fn CFRunLoopGetCurrent() -> CfRef;
    fn CFRunLoopAddSource(run_loop: CfRef, source: CfRef, mode: CfRef);
    fn CFRunLoopRemoveSource(run_loop: CfRef, source: CfRef, mode: CfRef);
    fn CFRunLoopRunInMode(mode: CfRef, seconds: f64, return_after_source: u8) -> i32;
}

/// What the power callback does for one IOKit message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PowerReaction {
    /// Idle sleep is being negotiated; nothing to wait for, so agree.
    AllowAtOnce,
    /// Sleep is committed: say goodbye, then agree.
    NoticeThenAllow,
    /// The Mac is awake again.
    Resumed,
    Ignore,
}

const fn react(message: u32) -> PowerReaction {
    match message {
        K_IO_MESSAGE_CAN_SYSTEM_SLEEP => PowerReaction::AllowAtOnce,
        K_IO_MESSAGE_SYSTEM_WILL_SLEEP => PowerReaction::NoticeThenAllow,
        K_IO_MESSAGE_SYSTEM_HAS_POWERED_ON => PowerReaction::Resumed,
        _ => PowerReaction::Ignore,
    }
}

/// Owned by the `kivori-power` thread, reached from the callback through `refcon`.
struct PowerContext {
    tx: Sender<HostSignal>,
    /// The `io_connect_t` `IORegisterForSystemPower` returned; set once, before the run loop starts.
    root_port: AtomicU32,
}

unsafe extern "C" fn on_power_message(
    refcon: *mut c_void,
    _service: u32,
    message: u32,
    argument: *mut c_void,
) {
    // SAFETY: `refcon` is the `PowerContext` the registering thread keeps alive until after it
    // deregisters, and this callback only runs on that thread's run loop.
    let context = &*refcon.cast::<PowerContext>();
    let allow = || {
        // The argument is the notification id, passed back verbatim.
        IOAllowPowerChange(context.root_port.load(Ordering::SeqCst), argument as isize);
    };
    match react(message) {
        PowerReaction::AllowAtOnce => allow(),
        PowerReaction::NoticeThenAllow => {
            let (ack_tx, ack_rx) = mpsc::sync_channel(1);
            let sent = context
                .tx
                .send(HostSignal {
                    event: HostEvent::Suspending,
                    ack: Some(ack_tx),
                })
                .is_ok();
            if sent {
                // Timeout or a gone device thread both fall through: sleep must never be blocked.
                let _ = ack_rx.recv_timeout(SUSPEND_ACK_BUDGET);
            }
            allow();
        }
        PowerReaction::Resumed => {
            let _ = context.tx.send(HostSignal::new(HostEvent::Resumed));
        }
        PowerReaction::Ignore => {}
    }
}

/// Registers for power messages on this thread and serves them until `stop`.
fn power_thread(tx: Sender<HostSignal>, stop: &AtomicBool, ready: &Sender<Result<(), String>>) {
    let context = Box::new(PowerContext {
        tx,
        root_port: AtomicU32::new(0),
    });
    let context_ptr = (&*context as *const PowerContext)
        .cast_mut()
        .cast::<c_void>();
    let mut port: IoNotificationPortRef = std::ptr::null_mut();
    let mut notifier: MachPort = 0;
    // SAFETY: out-pointers are valid locals; `context_ptr` outlives the registration (it is
    // dropped below, after `IODeregisterForSystemPower`).
    let root = unsafe {
        IORegisterForSystemPower(context_ptr, &mut port, on_power_message, &mut notifier)
    };
    if root == 0 || port.is_null() {
        let _ = ready.send(Err("IORegisterForSystemPower failed".to_string()));
        return;
    }
    context.root_port.store(root, Ordering::SeqCst);
    // SAFETY: `port` is live until destroyed below; the run-loop source is borrowed from it.
    let (run_loop, source) = unsafe {
        let source = IONotificationPortGetRunLoopSource(port);
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        (run_loop, source)
    };
    let _ = ready.send(Ok(()));
    while !stop.load(Ordering::SeqCst) {
        // Bounded slices, so `stop` is noticed without a cross-thread CFRunLoopStop race.
        // SAFETY: plain run-loop service on the thread that owns the source.
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, 0) };
    }
    // SAFETY: tears down exactly what was set up above, in reverse order.
    unsafe {
        CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
        IODeregisterForSystemPower(&mut notifier);
        IOServiceClose(root);
        IONotificationPortDestroy(port);
    }
    drop(context);
}

/// Starts the power and lock watchers.
///
/// # Errors
/// [`HostEventsError::Unavailable`] when IOKit refuses the power registration (lock polling alone
/// is not offered: the notice without the lock look would be half a feature).
pub fn start(tx: Sender<HostSignal>) -> Result<HostEventsGuard, HostEventsError> {
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let power_tx = tx.clone();
    let power_stop = Arc::clone(&stop);
    std::thread::Builder::new()
        .name("kivori-power".to_string())
        .spawn(move || power_thread(power_tx, &power_stop, &ready_tx))
        .map_err(|error| HostEventsError::Unavailable(error.to_string()))?;
    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(reason)) => return Err(HostEventsError::Unavailable(reason)),
        Err(_) => {
            return Err(HostEventsError::Unavailable(
                "power thread exited".to_string(),
            ))
        }
    }

    let lock_stop = Arc::clone(&stop);
    let lock_thread = std::thread::Builder::new()
        .name("kivori-lock".to_string())
        .spawn(move || {
            let mut was_locked = false;
            while !lock_stop.load(Ordering::SeqCst) {
                let locked = screen_locked();
                if let Some(event) = lock_edge(was_locked, locked) {
                    if tx.send(HostSignal::new(event)).is_err() {
                        return;
                    }
                }
                was_locked = locked;
                std::thread::sleep(LOCK_POLL);
            }
        });
    if let Err(error) = lock_thread {
        stop.store(true, Ordering::SeqCst);
        return Err(HostEventsError::Unavailable(error.to_string()));
    }
    Ok(HostEventsGuard::new(move || {
        stop.store(true, Ordering::SeqCst)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iokit_messages_map_to_reactions() {
        assert_eq!(react(0xE000_0270), PowerReaction::AllowAtOnce);
        assert_eq!(react(0xE000_0280), PowerReaction::NoticeThenAllow);
        assert_eq!(react(0xE000_0300), PowerReaction::Resumed);
        // WillNotSleep, WillPowerOn and anything else are not ours to act on.
        assert_eq!(react(0xE000_0290), PowerReaction::Ignore);
        assert_eq!(react(0xE000_0320), PowerReaction::Ignore);
        assert_eq!(react(0), PowerReaction::Ignore);
    }

    /// Real IOKit registration, no sleep: it must start, stay quiet, and stop cleanly.
    #[test]
    fn the_watcher_registers_with_iokit_and_stops() {
        let (tx, rx) = mpsc::channel();
        let guard = start(tx).expect("IOKit registration");
        std::thread::sleep(Duration::from_millis(600));
        drop(guard);
        // A lock edge is possible on a locked screen; a sleep notice never is.
        while let Ok(signal) = rx.try_recv() {
            assert!(matches!(
                signal.event,
                HostEvent::Locked | HostEvent::Unlocked
            ));
        }
    }
}
