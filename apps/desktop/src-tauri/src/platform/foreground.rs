//! Pure foreground classification, shared by the Windows and macOS observers so the protected
//! rules (invariant 9) are unit-tested on every host. The OS backends only gather raw facts.

use super::Foreground;

/// Windows executables that are protected surfaces whatever their elevation.
const WINDOWS_PROTECTED: [&str; 4] = [
    "consent.exe",
    "logonui.exe",
    "lockapp.exe",
    "credentialuibroker.exe",
];

/// macOS bundle ids (lowercase) that are protected surfaces.
const MAC_PROTECTED: [&str; 2] = ["com.apple.loginwindow", "com.apple.securityagent"];

/// No foreground window. `input_desktop` is the input desktop's name, `None` when it can't be
/// opened (which is itself what the secure desktop looks like from a normal process).
pub fn classify_windows_no_window(input_desktop: Option<&str>) -> Foreground {
    match input_desktop {
        Some(name) if name.eq_ignore_ascii_case("default") => Foreground::Unknown,
        _ => Foreground::Protected,
    }
}

/// A foreground window owned by the process at `image_path` (full Win32 path or bare file name).
/// `elevated` is `None` when the process token couldn't be queried.
pub fn classify_windows(
    image_path: &str,
    elevated: Option<bool>,
    self_elevated: bool,
) -> Foreground {
    let file = image_path.rsplit(['\\', '/']).next().unwrap_or_default();
    if file.is_empty() {
        return Foreground::Unknown;
    }
    let id = file.to_lowercase();
    if WINDOWS_PROTECTED.contains(&id.as_str()) {
        return Foreground::Protected;
    }
    match elevated {
        None => Foreground::Unknown,
        Some(true) if !self_elevated => Foreground::Protected,
        Some(_) => {
            let name = file
                .rsplit_once('.')
                .map_or(file, |(stem, _)| stem)
                .to_string();
            Foreground::App { id, name }
        }
    }
}

/// macOS frontmost app. A locked screen wins over whatever app is still nominally frontmost.
pub fn classify_mac(bundle_id: Option<&str>, name: Option<&str>, locked: bool) -> Foreground {
    if locked {
        return Foreground::Protected;
    }
    let Some(bundle) = bundle_id.filter(|b| !b.is_empty()) else {
        return Foreground::Unknown;
    };
    let id = bundle.to_lowercase();
    if MAC_PROTECTED.contains(&id.as_str()) {
        return Foreground::Protected;
    }
    let name = name.filter(|n| !n.is_empty()).unwrap_or(bundle).to_string();
    Foreground::App { id, name }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str) -> Foreground {
        Foreground::App {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn windows_no_window_is_protected_off_the_default_desktop() {
        assert_eq!(
            classify_windows_no_window(Some("Default")),
            Foreground::Unknown
        );
        assert_eq!(
            classify_windows_no_window(Some("Winlogon")),
            Foreground::Protected
        );
        assert_eq!(classify_windows_no_window(None), Foreground::Protected);
    }

    #[test]
    fn windows_app_id_is_lowercase_file_name_and_name_is_stem() {
        let path = r"C:\Users\a\AppData\Local\Programs\Microsoft VS Code\Code.exe";
        assert_eq!(
            classify_windows(path, Some(false), false),
            app("code.exe", "Code")
        );
        assert_eq!(
            classify_windows("chrome.exe", Some(false), false),
            app("chrome.exe", "chrome")
        );
    }

    #[test]
    fn windows_protected_executables_win_even_unqueryable() {
        for exe in [
            r"C:\Windows\System32\consent.exe",
            r"C:\Windows\System32\LogonUI.exe",
            r"C:\Windows\SystemApps\Microsoft.LockApp_cw5n1h2txyewy\LockApp.exe",
            r"C:\Windows\System32\CredentialUIBroker.exe",
        ] {
            assert_eq!(
                classify_windows(exe, None, false),
                Foreground::Protected,
                "{exe}"
            );
            assert_eq!(
                classify_windows(exe, Some(false), true),
                Foreground::Protected,
                "{exe}"
            );
        }
    }

    #[test]
    fn windows_elevated_foreground_is_protected_only_when_kivori_is_not() {
        let exe = r"C:\Windows\regedit.exe";
        assert_eq!(
            classify_windows(exe, Some(true), false),
            Foreground::Protected
        );
        assert_eq!(
            classify_windows(exe, Some(true), true),
            app("regedit.exe", "regedit")
        );
    }

    #[test]
    fn windows_unqueryable_or_empty_is_unknown_not_a_guess() {
        assert_eq!(
            classify_windows(r"C:\x\notepad.exe", None, false),
            Foreground::Unknown
        );
        assert_eq!(
            classify_windows("", Some(false), false),
            Foreground::Unknown
        );
        assert_eq!(
            classify_windows(r"C:\dir\", Some(false), false),
            Foreground::Unknown
        );
    }

    #[test]
    fn mac_app_id_is_lowercase_bundle_id() {
        assert_eq!(
            classify_mac(Some("com.microsoft.VSCode"), Some("Code"), false),
            app("com.microsoft.vscode", "Code")
        );
        assert_eq!(
            classify_mac(Some("com.google.Chrome"), None, false),
            app("com.google.chrome", "com.google.Chrome")
        );
    }

    #[test]
    fn mac_protected_bundles_and_lock_screen() {
        assert_eq!(
            classify_mac(Some("com.apple.loginwindow"), None, false),
            Foreground::Protected
        );
        assert_eq!(
            classify_mac(
                Some("com.apple.SecurityAgent"),
                Some("SecurityAgent"),
                false
            ),
            Foreground::Protected
        );
        assert_eq!(
            classify_mac(Some("com.google.Chrome"), Some("Chrome"), true),
            Foreground::Protected
        );
        assert_eq!(classify_mac(None, None, true), Foreground::Protected);
    }

    #[test]
    fn mac_no_frontmost_app_is_unknown() {
        assert_eq!(classify_mac(None, None, false), Foreground::Unknown);
        assert_eq!(
            classify_mac(Some(""), Some("x"), false),
            Foreground::Unknown
        );
    }
}
