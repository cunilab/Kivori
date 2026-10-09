//! Window + tray + single-instance wiring over the pure [`crate::window_lifecycle`] policy (FR-030,
//! SC-007). The policy decides hide-vs-quit / show-on-reactivate (and is unit-tested standalone); this
//! module only performs the resulting OS action and keeps the device thread alive across hide/close.

use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Window, WindowEvent, Wry};
use tauri_plugin_autostart::ManagerExt;

use crate::firmware::UpdateAdvice;
use crate::ipc::dto::{ConnectionStatusDto, StartupSettingsDto};
use crate::runtime::state::AppState;
use crate::window_lifecycle::{LifecycleAction, LifecycleEvent};

const MAIN: &str = "main";
const TRAY_ID: &str = "kivori";

/// What the tray shows for one connection snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayModel {
    /// The disabled status line at the top of the menu.
    pub status: &'static str,
    /// The hover tooltip.
    pub tooltip: String,
}

/// Projects the tray's text from the connection snapshot and the firmware update advice. Pure.
#[must_use]
pub fn tray_model(status: &ConnectionStatusDto, advice: UpdateAdvice) -> TrayModel {
    let line = if status.host != "active" {
        "Paused while locked"
    } else if status.connection == "connected" {
        "Connected"
    } else {
        "Not connected"
    };
    let mut tooltip = format!("Kivori \u{b7} Desktop {}", env!("CARGO_PKG_VERSION"));
    if let Some(device) = &status.device {
        tooltip.push_str(&format!(" \u{b7} Firmware {}", device.firmware_version));
        if advice == UpdateAdvice::UpdateAvailable {
            tooltip.push_str(" \u{b7} Update available");
        }
    }
    TrayModel {
        status: line,
        tooltip,
    }
}

/// The tray items that change after the tray is built (managed by Tauri).
struct TrayHandles {
    status: MenuItem<Wry>,
    launch: CheckMenuItem<Wry>,
}

/// Whether Kivori is registered to start at login. The OS entry is the source of truth.
pub fn startup_settings(app: &AppHandle) -> Result<StartupSettingsDto, String> {
    let enabled = app
        .autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())?;
    Ok(StartupSettingsDto::new(enabled))
}

/// Registers or removes the login entry, then mirrors the OS state into the tray check item.
pub fn set_launch_at_login(app: &AppHandle, enabled: bool) -> Result<StartupSettingsDto, String> {
    let launch = app.autolaunch();
    let result = if enabled {
        launch.enable()
    } else {
        launch.disable()
    };
    let settings = startup_settings(app);
    if let (Some(handles), Ok(current)) = (app.try_state::<TrayHandles>(), &settings) {
        let _ = handles.launch.set_checked(current.launch_at_login);
    }
    result.map_err(|error| error.to_string())?;
    settings
}

/// Applies a fresh connection snapshot to the tray (called from the device thread on change).
pub fn refresh_tray(app: &AppHandle, status: &ConnectionStatusDto) {
    let advice = app
        .try_state::<AppState>()
        .map_or(UpdateAdvice::Unknown, |state| {
            state.firmware_status_snapshot().advice
        });
    let model = tray_model(status, advice);
    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles.status.set_text(model.status);
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(model.tooltip));
    }
}

/// Builder hook: a window close request hides the window (the device thread keeps running); the pure
/// policy makes the decision.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        perform(window, LifecycleEvent::CloseRequested);
    }
}

/// Installs the tray icon: a status line, Open, the Launch at login switch and Quit.
///
/// # Errors
/// Propagates Tauri errors while building the menu or tray icon.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let model = tray_model(
        &app.state::<AppState>().status_snapshot(),
        UpdateAdvice::Unknown,
    );
    let status = MenuItem::with_id(app, "status", model.status, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Kivori", true, None::<&str>)?;
    let enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let launch = CheckMenuItem::with_id(
        app,
        "launch",
        "Launch at login",
        true,
        enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&status, &open, &launch, &quit])?;
    // macOS: a monochrome template (alpha only) the menu bar tints to match light/dark mode. Other
    // platforms keep the full-colour app icon, which stays visible on a dark taskbar.
    // Source artwork: assets/icon/.
    #[cfg(target_os = "macos")]
    let icon = tauri::image::Image::from_bytes(include_bytes!("../../icons/tray-template.png"))?;
    #[cfg(not(target_os = "macos"))]
    let icon = app
        .default_window_icon()
        .expect("bundled default icon")
        .clone();
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip(model.tooltip)
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "launch" => {
                // The check item flips itself on click; the OS entry decides what it ends up as.
                let wanted = !app.autolaunch().is_enabled().unwrap_or(false);
                if let Err(error) = set_launch_at_login(app, wanted) {
                    tracing::warn!(%error, "could not change launch at login");
                }
            }
            "quit" => quit_app(app),
            _ => {}
        })
        .build(app)?;
    app.manage(TrayHandles { status, launch });
    Ok(())
}

/// Shows + focuses the single main window (dock/taskbar reactivate, tray Open, or a second instance).
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window
            .state::<AppState>()
            .lifecycle
            .lock()
            .expect("lifecycle lock")
            .on_event(LifecycleEvent::Reactivated);
    }
}

fn quit_app(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        window.state::<AppState>().shutdown();
    }
    app.exit(0);
}

fn perform(window: &Window, event: LifecycleEvent) {
    let action = {
        let state = window.state::<AppState>();
        let mut policy = state.lifecycle.lock().expect("lifecycle lock");
        policy.on_event(event)
    };
    match action {
        LifecycleAction::HideWindow => {
            let _ = window.hide();
        }
        LifecycleAction::ShowWindow => {
            let _ = window.show();
            let _ = window.set_focus();
        }
        LifecycleAction::Quit => quit_app(window.app_handle()),
        LifecycleAction::None => {}
    }
}
