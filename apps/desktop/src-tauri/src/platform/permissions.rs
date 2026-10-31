//! OS permissions Kivori can ask for. Only macOS has one: Accessibility, which lets Kivori press
//! keys on the user's behalf (shortcuts, media keys). Everywhere else there is nothing to grant.

/// The state of the Accessibility permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accessibility {
    Granted,
    Missing,
    /// This OS has no such permission.
    NotApplicable,
}

/// Every token [`Accessibility::token`] can return (shared with the webview).
pub const ACCESSIBILITY_TOKENS: [&str; 3] = ["granted", "missing", "notApplicable"];

impl Accessibility {
    /// The camelCase token used over IPC.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Missing => "missing",
            Self::NotApplicable => "notApplicable",
        }
    }
}

/// Whether Kivori may synthesize input right now. Never shows a prompt.
#[must_use]
pub fn accessibility() -> Accessibility {
    #[cfg(target_os = "macos")]
    {
        mac::trusted(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Accessibility::NotApplicable
    }
}

/// Like [`accessibility`], but when the permission is missing asks macOS to show its own
/// "Kivori would like to control this computer" prompt (which links to System Settings).
#[must_use]
pub fn request_accessibility() -> Accessibility {
    #[cfg(target_os = "macos")]
    {
        mac::trusted(true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Accessibility::NotApplicable
    }
}

/// Opens System Settings at Privacy & Security > Accessibility. The URL is a fixed string; nothing
/// from the caller reaches the command line.
///
/// # Errors
/// `Err(())` when the OS could not be asked, or on a platform without the permission.
#[allow(clippy::result_unit_err)]
pub fn open_accessibility_settings() -> Result<(), ()> {
    #[cfg(target_os = "macos")]
    {
        const URL: &str =
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
        std::process::Command::new("/usr/bin/open")
            .arg(URL)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()
            .filter(std::process::ExitStatus::success)
            .map(|_| ())
            .ok_or(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(())
    }
}

/// Why [`reset_accessibility`] could not clear the stale approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetError {
    /// This OS has no such permission, or the app is not running from a bundle (a plain
    /// `cargo run` binary has no bundle identifier of its own to reset).
    NotAvailable,
    /// `tccutil` ran and failed, or could not run.
    Failed,
}

/// The fixed tool that edits the privacy database. An absolute path, never looked up on `PATH`.
const TCCUTIL: &str = "/usr/bin/tccutil";

/// A bundle identifier is reverse-DNS: letters, digits, `.` and `-`, not starting with `-` or `.`.
/// Anything else is refused so nothing odd reaches `tccutil` as an option.
fn valid_bundle_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 155
        && !id.starts_with(['-', '.'])
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// The arguments that clear Accessibility approvals for exactly `bundle_id` (and no other app).
fn reset_args(bundle_id: &str) -> Option<[String; 3]> {
    valid_bundle_id(bundle_id).then(|| {
        [
            "reset".to_string(),
            "Accessibility".to_string(),
            bundle_id.to_string(),
        ]
    })
}

/// Clears this app's stale Accessibility approval through `run` (given the `tccutil` arguments),
/// when `bundle_id` is known. Split out so tests never touch the real privacy database.
fn reset_with(
    bundle_id: Option<String>,
    run: impl FnOnce(&[String]) -> bool,
) -> Result<(), ResetError> {
    let args = bundle_id
        .as_deref()
        .and_then(reset_args)
        .ok_or(ResetError::NotAvailable)?;
    if run(&args) {
        Ok(())
    } else {
        Err(ResetError::Failed)
    }
}

/// Reset, then ask once (`ask`), then open the settings pane (`open`); the order is the contract.
/// Returns the state `ask` reported. A settings pane that cannot open does not fail the reset.
fn reset_and_ask_with(
    bundle_id: Option<String>,
    run: impl FnOnce(&[String]) -> bool,
    ask: impl FnOnce() -> Accessibility,
    open: impl FnOnce(),
) -> Result<Accessibility, ResetError> {
    reset_with(bundle_id, run)?;
    let state = ask();
    open();
    Ok(state)
}

/// For an approval macOS still holds from an older build of this app (it is tied to the exact
/// code, so an update or rebuild invalidates it): clears this app's own Accessibility entry with
/// `tccutil reset Accessibility <own bundle id>`, asks for the permission once, and opens the
/// settings pane. Other apps' approvals are untouched.
///
/// # Errors
/// [`ResetError::NotAvailable`] off macOS or outside an app bundle; [`ResetError::Failed`] when
/// `tccutil` did not succeed.
pub fn reset_accessibility() -> Result<Accessibility, ResetError> {
    #[cfg(target_os = "macos")]
    let bundle_id = mac::main_bundle_identifier();
    #[cfg(not(target_os = "macos"))]
    let bundle_id = None;
    reset_and_ask_with(
        bundle_id,
        |args| {
            std::process::Command::new(TCCUTIL)
                .args(args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        },
        request_accessibility,
        || {
            let _ = open_accessibility_settings();
        },
    )
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;

    use super::Accessibility;

    type CfRef = *const c_void;

    /// The opaque callback tables CoreFoundation exports for `CFDictionaryCreate`.
    #[repr(C)]
    struct CfCallbacks {
        _opaque: [u8; 0],
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        static kAXTrustedCheckOptionPrompt: CfRef;
        fn AXIsProcessTrusted() -> u8;
        fn AXIsProcessTrustedWithOptions(options: CfRef) -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: CfRef;
        static kCFTypeDictionaryKeyCallBacks: CfCallbacks;
        static kCFTypeDictionaryValueCallBacks: CfCallbacks;
        fn CFDictionaryCreate(
            allocator: CfRef,
            keys: *const CfRef,
            values: *const CfRef,
            count: isize,
            key_callbacks: *const CfCallbacks,
            value_callbacks: *const CfCallbacks,
        ) -> CfRef;
        fn CFRelease(cf: CfRef);
        fn CFBundleGetMainBundle() -> CfRef;
        fn CFBundleGetIdentifier(bundle: CfRef) -> CfRef;
        fn CFStringGetCString(
            string: CfRef,
            buffer: *mut std::ffi::c_char,
            buffer_size: isize,
            encoding: u32,
        ) -> u8;
    }

    /// `kCFStringEncodingUTF8`.
    const UTF8: u32 = 0x0800_0100;

    /// The identifier of the running app's own bundle, read from the main bundle's Info.plist.
    /// `None` when not running from a bundle (a bare binary has none).
    pub(super) fn main_bundle_identifier() -> Option<String> {
        let mut buffer = [0 as std::ffi::c_char; 256];
        // SAFETY: the main bundle and its identifier are CoreFoundation objects owned by the
        // framework ("get" rule: not retained, not released here) and either may be null, which
        // is checked. The buffer is a local array whose length is passed along.
        let copied = unsafe {
            let bundle = CFBundleGetMainBundle();
            if bundle.is_null() {
                return None;
            }
            let id = CFBundleGetIdentifier(bundle);
            if id.is_null() {
                return None;
            }
            CFStringGetCString(id, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) != 0
        };
        if !copied {
            return None;
        }
        // SAFETY: on success CFStringGetCString wrote a NUL-terminated string into `buffer`.
        let text = unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) };
        text.to_str().ok().map(str::to_owned)
    }

    pub(super) fn trusted(prompt: bool) -> Accessibility {
        // SAFETY: both calls take no pointers we own beyond the options dictionary, which is
        // created, used and released within this block; the extern statics are immutable
        // CoreFoundation constants that live for the whole process.
        let trusted = unsafe {
            if prompt {
                let keys = [kAXTrustedCheckOptionPrompt];
                let values = [kCFBooleanTrue];
                let options = CFDictionaryCreate(
                    std::ptr::null(),
                    keys.as_ptr(),
                    values.as_ptr(),
                    1,
                    &raw const kCFTypeDictionaryKeyCallBacks,
                    &raw const kCFTypeDictionaryValueCallBacks,
                );
                let trusted = AXIsProcessTrustedWithOptions(options) != 0;
                if !options.is_null() {
                    CFRelease(options);
                }
                trusted
            } else {
                AXIsProcessTrusted() != 0
            }
        };
        if trusted {
            Accessibility::Granted
        } else {
            Accessibility::Missing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_the_documented_ones() {
        assert_eq!(Accessibility::Granted.token(), "granted");
        assert_eq!(Accessibility::Missing.token(), "missing");
        assert_eq!(Accessibility::NotApplicable.token(), "notApplicable");
    }

    #[test]
    fn the_reset_targets_only_the_given_bundle() {
        assert_eq!(
            reset_args("id.example.app"),
            Some([
                "reset".to_string(),
                "Accessibility".to_string(),
                "id.example.app".to_string()
            ])
        );
        assert_eq!(TCCUTIL, "/usr/bin/tccutil");
    }

    #[test]
    fn odd_bundle_ids_never_reach_tccutil() {
        for bad in [
            "", "-a", ".x", "a b", "a;b", "a/b", "a\nb", "--all", "é.app",
        ] {
            assert_eq!(reset_args(bad), None, "{bad:?}");
        }
        assert!(reset_args(&"a".repeat(156)).is_none());
    }

    #[test]
    fn the_reset_needs_a_bundle_and_a_successful_tccutil() {
        let never = |_: &[String]| -> bool { panic!("must not run") };
        assert_eq!(reset_with(None, never), Err(ResetError::NotAvailable));
        assert_eq!(
            reset_with(Some("-bad".into()), never),
            Err(ResetError::NotAvailable)
        );
        assert_eq!(
            reset_with(Some("id.example.app".into()), |_| false),
            Err(ResetError::Failed)
        );
        let mut seen = Vec::new();
        assert_eq!(
            reset_with(Some("id.example.app".into()), |args| {
                seen = args.to_vec();
                true
            }),
            Ok(())
        );
        assert_eq!(seen, ["reset", "Accessibility", "id.example.app"]);
    }

    #[test]
    fn reset_and_ask_runs_in_order_and_asks_exactly_once() {
        use std::cell::RefCell;
        let log = RefCell::new(Vec::new());
        let state = reset_and_ask_with(
            Some("id.example.app".into()),
            |_| {
                log.borrow_mut().push("reset");
                true
            },
            || {
                log.borrow_mut().push("ask");
                Accessibility::Missing
            },
            || log.borrow_mut().push("open"),
        );
        assert_eq!(state, Ok(Accessibility::Missing));
        assert_eq!(*log.borrow(), ["reset", "ask", "open"]);
    }

    #[test]
    fn a_failed_reset_neither_asks_nor_opens() {
        let result = reset_and_ask_with(
            Some("id.example.app".into()),
            |_| false,
            || panic!("must not ask"),
            || panic!("must not open"),
        );
        assert_eq!(result, Err(ResetError::Failed));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_reset_is_not_available_off_macos() {
        assert_eq!(reset_accessibility(), Err(ResetError::NotAvailable));
    }

    /// The test binary is not an app bundle, so there is no identifier (and so no real reset).
    #[cfg(target_os = "macos")]
    #[test]
    fn a_bare_binary_has_no_bundle_identifier_to_reset() {
        assert_eq!(mac::main_bundle_identifier(), None);
        assert_eq!(reset_accessibility(), Err(ResetError::NotAvailable));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn there_is_no_permission_to_grant_off_macos() {
        assert_eq!(accessibility(), Accessibility::NotApplicable);
        assert_eq!(request_accessibility(), Accessibility::NotApplicable);
        assert!(open_accessibility_settings().is_err());
    }

    /// Only the plain check runs here: the prompting variant would pop a dialog on a dev machine.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_macos_check_answers_without_panicking() {
        assert!(matches!(
            accessibility(),
            Accessibility::Granted | Accessibility::Missing
        ));
    }
}
