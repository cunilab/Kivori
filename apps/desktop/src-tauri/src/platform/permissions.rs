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
