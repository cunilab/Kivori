//! macOS foreground app: `[[NSWorkspace sharedWorkspace] frontmostApplication]` through the raw
//! Objective-C runtime (no objc crate), plus the CoreGraphics session dictionary for the lock
//! screen. Runs on the device thread, so every call sits inside its own autorelease pool. The
//! decision itself is `platform::foreground::classify_mac`.
//!
//! NSWorkspace refreshes `frontmostApplication` from notifications on the MAIN run loop, which
//! Tauri keeps running; a process without one sees a stale value.

use std::ffi::{c_char, c_void, CStr};

use crate::platform::foreground::classify_mac;
use crate::platform::{Foreground, ForegroundObserver};

type Id = *mut c_void;
type Sel = *mut c_void;
type CfRef = *const c_void;

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

// Makes sure NSWorkspace is registered with the runtime even if nothing else pulled AppKit in.
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGSessionCopyCurrentDictionary() -> CfRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(alloc: CfRef, s: *const c_char, encoding: u32) -> CfRef;
    fn CFDictionaryGetValue(dict: CfRef, key: CfRef) -> CfRef;
    fn CFGetTypeID(cf: CfRef) -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(b: CfRef) -> u8;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(n: CfRef, kind: isize, out: *mut c_void) -> u8;
    fn CFRelease(cf: CfRef);
}

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT64_TYPE: isize = 4;

#[derive(Debug, Clone, Copy, Default)]
pub struct MacForeground;

impl ForegroundObserver for MacForeground {
    fn foreground(&self) -> Foreground {
        let locked = screen_locked();
        // SAFETY: the pool brackets every Objective-C object touched in `frontmost`, and the
        // strings it returns are owned copies taken before the pop.
        let (bundle, name) = unsafe {
            let pool = objc_autoreleasePoolPush();
            let app = frontmost();
            objc_autoreleasePoolPop(pool);
            app
        };
        classify_mac(bundle.as_deref(), name.as_deref(), locked)
    }
}

/// `objc_msgSend` with no arguments and an object result.
unsafe fn send(receiver: Id, selector: &CStr) -> Id {
    if receiver.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: on both Apple ABIs a zero-argument message returning an object pointer is exactly
    // this signature; the caller only sends selectors that take no arguments.
    let msg: unsafe extern "C" fn(Id, Sel) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    msg(receiver, sel_registerName(selector.as_ptr()))
}

/// Copies an `NSString` out; `None` for nil.
unsafe fn string(ns: Id) -> Option<String> {
    let utf8 = send(ns, c"UTF8String") as *const c_char;
    (!utf8.is_null()).then(|| CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

/// (bundle identifier, localized name) of the frontmost app. Caller owns the autorelease pool.
unsafe fn frontmost() -> (Option<String>, Option<String>) {
    let workspace = send(objc_getClass(c"NSWorkspace".as_ptr()), c"sharedWorkspace");
    let app = send(workspace, c"frontmostApplication");
    (
        string(send(app, c"bundleIdentifier")),
        string(send(app, c"localizedName")),
    )
}

/// `CGSSessionScreenIsLocked` in the current session dictionary. A missing dictionary or key is
/// "not locked"; the frontmost-app rules still catch loginwindow.
pub(super) fn screen_locked() -> bool {
    // SAFETY: every CF object created or copied here is released before returning; the value
    // read from the dictionary is borrowed (Get rule) and type-checked before use.
    unsafe {
        let dict = CGSessionCopyCurrentDictionary();
        if dict.is_null() {
            return false;
        }
        let key = CFStringCreateWithCString(
            std::ptr::null(),
            c"CGSSessionScreenIsLocked".as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );
        let value = if key.is_null() {
            std::ptr::null()
        } else {
            CFDictionaryGetValue(dict, key)
        };
        let locked = if value.is_null() {
            false
        } else if CFGetTypeID(value) == CFBooleanGetTypeID() {
            CFBooleanGetValue(value) != 0
        } else if CFGetTypeID(value) == CFNumberGetTypeID() {
            let mut n: i64 = 0;
            CFNumberGetValue(value, K_CF_NUMBER_SINT64_TYPE, (&mut n as *mut i64).cast()) != 0
                && n != 0
        } else {
            false
        };
        if !key.is_null() {
            CFRelease(key);
        }
        CFRelease(dict);
        locked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real OS call: `cargo test -p kivori-desktop -- --ignored --nocapture frontmost_app_now`.
    #[test]
    #[ignore = "reads the live frontmost app"]
    fn frontmost_app_now() {
        let seen = MacForeground.foreground();
        println!("foreground: {seen:?}");
        if let Foreground::App { id, .. } = &seen {
            assert_eq!(id, &id.to_lowercase());
        }
    }
}
