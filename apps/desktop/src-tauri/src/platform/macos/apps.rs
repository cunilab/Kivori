//! macOS running apps and app icons through the raw Objective-C runtime (like `foreground`):
//! `NSWorkspace runningApplications`, and `iconForFile:` of the app bundle re-encoded as PNG by
//! `NSBitmapImageRep`. Every call sits inside its own autorelease pool and copies what it returns
//! before the pool is popped. Runs on a worker thread, never the device thread.

use std::ffi::{c_char, CStr, CString};

use super::foreground::{
    objc_autoreleasePoolPop, objc_autoreleasePoolPush, objc_getClass, objc_msgSend,
    sel_registerName, send, string, Id, Sel,
};
use crate::platform::apps::RunningApp;
use crate::platform::foreground::{classify_mac, is_own_app};
use crate::platform::Foreground;

// Makes sure NSWorkspace and NSBitmapImageRep are registered with the runtime.
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

/// `NSApplicationActivationPolicyRegular`: apps with a Dock icon, not agents or daemons.
const POLICY_REGULAR: isize = 0;
/// `NSBitmapImageFileTypePNG`.
const FILE_TYPE_PNG: usize = 4;
/// The icon is taken from the first representation at least this many pixels wide (the 32 pt
/// icon at 2x), else the widest there is.
const WANTED_PIXELS: isize = 64;

fn msg() -> unsafe extern "C" fn() {
    objc_msgSend
}

unsafe fn send_isize(receiver: Id, selector: &CStr) -> isize {
    // SAFETY: a zero-argument message returning an integer has exactly this signature.
    let f: unsafe extern "C" fn(Id, Sel) -> isize = std::mem::transmute(msg());
    f(receiver, sel_registerName(selector.as_ptr()))
}

unsafe fn send_usize_arg(receiver: Id, selector: &CStr, arg: usize) -> Id {
    // SAFETY: one NSUInteger argument, object result.
    let f: unsafe extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(msg());
    f(receiver, sel_registerName(selector.as_ptr()), arg)
}

unsafe fn send_id_arg(receiver: Id, selector: &CStr, arg: Id) -> Id {
    // SAFETY: one object argument, object result.
    let f: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(msg());
    f(receiver, sel_registerName(selector.as_ptr()), arg)
}

unsafe fn workspace() -> Id {
    send(objc_getClass(c"NSWorkspace".as_ptr()), c"sharedWorkspace")
}

/// Apps with a Dock icon, by bundle id and localized name.
pub fn running_apps() -> Vec<RunningApp> {
    // SAFETY: the pool brackets every object touched; strings are copied out before the pop.
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let apps = send(workspace(), c"runningApplications");
        let count = send_isize(apps, c"count");
        let mut out = Vec::new();
        for i in 0..usize::try_from(count).unwrap_or(0) {
            let app = send_usize_arg(apps, c"objectAtIndex:", i);
            if send_isize(app, c"activationPolicy") != POLICY_REGULAR {
                continue;
            }
            let bundle = string(send(app, c"bundleIdentifier"));
            let name = string(send(app, c"localizedName"));
            if let Foreground::App { id, name } =
                classify_mac(bundle.as_deref(), name.as_deref(), false)
            {
                if !is_own_app(&id) {
                    out.push(RunningApp { id, name });
                }
            }
        }
        objc_autoreleasePoolPop(pool);
        out
    }
}

/// The icon of the app with bundle id `id` as PNG bytes; `None` when it is not installed.
pub fn icon_png(id: &str) -> Option<Vec<u8>> {
    let id = CString::new(id).ok()?;
    // SAFETY: as in `running_apps`; the PNG bytes are copied before the pool is popped.
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let png = icon_png_in_pool(&id);
        objc_autoreleasePoolPop(pool);
        png
    }
}

unsafe fn icon_png_in_pool(id: &CStr) -> Option<Vec<u8>> {
    let ns_string = objc_getClass(c"NSString".as_ptr());
    let name = send_cstr_arg(ns_string, c"stringWithUTF8String:", id.as_ptr());
    let url = send_id_arg(workspace(), c"URLForApplicationWithBundleIdentifier:", name);
    // Not installed: there is no bundle, and `iconForFile:` would only give a generic icon.
    if url.is_null() {
        return None;
    }
    let path = send(url, c"path");
    let image = send_id_arg(workspace(), c"iconForFile:", path);
    let tiff = send(image, c"TIFFRepresentation");
    if tiff.is_null() {
        return None;
    }
    let reps = send_id_arg(
        objc_getClass(c"NSBitmapImageRep".as_ptr()),
        c"imageRepsWithData:",
        tiff,
    );
    let count = usize::try_from(send_isize(reps, c"count")).ok()?;
    // The smallest representation that is big enough, else the biggest.
    let mut best: Option<(Id, isize)> = None;
    for i in 0..count {
        let rep = send_usize_arg(reps, c"objectAtIndex:", i);
        let wide = send_isize(rep, c"pixelsWide");
        let better = match best {
            None => true,
            Some((_, have)) if have < WANTED_PIXELS => wide > have,
            Some((_, have)) => (WANTED_PIXELS..have).contains(&wide),
        };
        if better {
            best = Some((rep, wide));
        }
    }
    let (rep, _) = best?;
    let props = send(objc_getClass(c"NSDictionary".as_ptr()), c"dictionary");
    // SAFETY: (NSBitmapImageFileType, NSDictionary) -> NSData.
    let encode: unsafe extern "C" fn(Id, Sel, usize, Id) -> Id = std::mem::transmute(msg());
    let data = encode(
        rep,
        sel_registerName(c"representationUsingType:properties:".as_ptr()),
        FILE_TYPE_PNG,
        props,
    );
    let len = usize::try_from(send_isize(data, c"length")).ok()?;
    // SAFETY: `-[NSData bytes]` returns a pointer to `length` readable bytes.
    let bytes = send(data, c"bytes") as *const u8;
    (len > 0 && !bytes.is_null()).then(|| std::slice::from_raw_parts(bytes, len).to_vec())
}

unsafe fn send_cstr_arg(receiver: Id, selector: &CStr, arg: *const c_char) -> Id {
    // SAFETY: one C-string argument, object result.
    let f: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id = std::mem::transmute(msg());
    f(receiver, sel_registerName(selector.as_ptr()), arg)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real OS calls: `cargo test -p kivori-desktop -- --ignored --nocapture mac_apps_now`.
    #[test]
    #[ignore = "reads the live running apps and an installed app's icon"]
    fn mac_apps_now() {
        let apps = running_apps();
        println!("running: {apps:?}");
        let finder = icon_png("com.apple.finder").expect("Finder has an icon");
        println!("finder icon: {} bytes", finder.len());
        assert_eq!(&finder[..4], &[0x89, b'P', b'N', b'G']);
        assert!(icon_png("not.an.app.zzz").is_none());
    }
}
