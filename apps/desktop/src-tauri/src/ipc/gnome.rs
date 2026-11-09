//! GNOME bridge commands; no filesystem paths or arbitrary command arguments cross IPC.

#[tauri::command]
pub async fn get_gnome_focus_status() -> Result<&'static str, String> {
    task(Action::Status).await
}

#[tauri::command]
pub async fn enable_gnome_focus() -> Result<&'static str, String> {
    task(Action::Install).await
}

#[tauri::command]
pub async fn logout_gnome() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return tauri::async_runtime::spawn_blocking(crate::platform::linux::gnome::logout)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "linux"))]
    Err("GNOME is only available on Linux.".into())
}

enum Action {
    Status,
    Install,
}

async fn task(action: Action) -> Result<&'static str, String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::gnome::{self, FocusStatus};
        let state = tauri::async_runtime::spawn_blocking(move || match action {
            Action::Status => Ok(gnome::status()),
            Action::Install => gnome::install(),
        })
        .await
        .map_err(|e| e.to_string())??;
        Ok(match state {
            FocusStatus::NotApplicable => "notApplicable",
            FocusStatus::NotInstalled => "notInstalled",
            FocusStatus::NeedsLogout => "needsLogout",
            FocusStatus::Working => "working",
            FocusStatus::Disabled => "disabled",
            FocusStatus::Unsupported => "unsupported",
            FocusStatus::Unavailable => "unavailable",
            FocusStatus::UpdateAvailable => "updateAvailable",
            FocusStatus::CliMissing => "cliMissing",
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        match action {
            Action::Status => Ok("notApplicable"),
            Action::Install => Err("GNOME is only available on Linux.".into()),
        }
    }
}
