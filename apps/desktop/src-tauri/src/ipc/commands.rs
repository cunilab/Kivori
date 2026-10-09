//! The least-privilege Tauri command surface (contracts/ipc.md §1).
//!
//! Every command here is the ENTIRE privileged API the webview may call. State-carrying commands take
//! a lowercase wire token and parse it at the boundary, so `set_desired_state`/`mirror_state` accept a
//! `SendableState` only — `booting`/`offline` can never be transmitted (FR-014/015). The dev-only
//! commands are compiled out of release builds via the `device-studio` feature (FR-028).

use tauri::{AppHandle, State};

use crate::activity::ActivityEventKind;
use crate::firmware::FirmwareStatus;
use crate::ipc::dto::{
    self, ActivityEventDto, AppInfoDto, ConfigDto, ConnectionStatusDto, DiagnosticsDto,
    StartupSettingsDto,
};
use crate::ipc::events;
use crate::runtime::state::{AppState, DeviceCommand};
use kivori_model::CompanionState;

/// Application/build info (all builds).
#[tauri::command]
pub fn get_app_info(app: State<'_, AppState>) -> AppInfoDto {
    dto::app_info(app.device_studio_enabled)
}

/// The current three-axis connection snapshot (initial UI sync; all builds).
#[tauri::command]
pub fn get_connection_status(app: State<'_, AppState>) -> ConnectionStatusDto {
    app.status_snapshot()
}

/// All six companion states as wire tokens (all builds).
#[tauri::command]
pub fn list_states() -> Vec<String> {
    CompanionState::ALL
        .iter()
        .map(|state| dto::companion_token(*state).to_string())
        .collect()
}

/// Sets the desired companion state (production path). Accepts a sendable token only.
///
/// # Errors
/// Returns an error string if `state` is not a sendable token, or the device runtime is unavailable.
#[tauri::command]
pub fn set_desired_state(app: State<'_, AppState>, state: String) -> Result<(), String> {
    if app.firmware_busy() {
        return Err(
            "Firmware update is in progress; wait for the device to reconnect.".to_string(),
        );
    }
    let desired =
        dto::sendable_from_token(&state).ok_or_else(|| format!("not a sendable state: {state}"))?;
    app.send_command(DeviceCommand::SetDesired(desired))
}

/// The diagnostics page's figures (all builds). Safe to copy and share: no ports, paths or raw ids.
#[tauri::command]
pub fn get_diagnostics(app: State<'_, AppState>) -> DiagnosticsDto {
    app.diagnostics_snapshot()
}

/// Whether Kivori starts at login, read from the OS entry (all builds).
///
/// # Errors
/// Returns the OS error text when the login entry cannot be read.
#[tauri::command]
pub fn get_startup_settings(app: AppHandle) -> Result<StartupSettingsDto, String> {
    crate::runtime::lifecycle::startup_settings(&app)
}

/// Adds or removes the OS login entry and returns what the OS now says.
///
/// # Errors
/// Returns the OS error text when the entry cannot be changed.
#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<StartupSettingsDto, String> {
    crate::runtime::lifecycle::set_launch_at_login(&app, enabled)
}

/// The stored settings (initial sync; `config://changed` carries changes).
#[tauri::command]
pub fn get_config(app: State<'_, AppState>) -> ConfigDto {
    app.config_snapshot()
}

/// Runs one config change and publishes its outcome to the activity log and `config://changed`.
fn commit_config(
    handle: &AppHandle,
    app: &AppState,
    saved: ActivityEventKind,
    change: impl FnOnce(&mut crate::config::ConfigStore) -> Result<(), crate::config::ConfigError>,
) -> Result<ConfigDto, String> {
    let (outcome, activity) = app.update_config(saved, change);
    events::emit_activity_log(handle, &activity);
    let config = outcome?;
    events::emit_config_changed(handle, &config);
    Ok(config)
}

/// Sets the home view and what a double press shows (`cycle` = every view in turn).
///
/// # Errors
/// Returns an error for an unknown view, a double-press view equal to the default, or a failed save.
#[tauri::command]
pub fn set_display_settings(
    handle: AppHandle,
    app: State<'_, AppState>,
    default_view: String,
    secondary_view: String,
) -> Result<ConfigDto, String> {
    let display = dto::display_settings_from_tokens(&default_view, &secondary_view)
        .map_err(str::to_string)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| file.display = display);
        store.save(next)
    })
}

/// Sets whether the buddy plays reactions on its own, and how lively it is.
///
/// # Errors
/// Returns an error for an unknown intensity or a failed save.
#[tauri::command]
pub fn set_buddy_settings(
    handle: AppHandle,
    app: State<'_, AppState>,
    reactions: bool,
    intensity: String,
) -> Result<ConfigDto, String> {
    let intensity = dto::intensity_from_token(&intensity)
        .ok_or_else(|| "unknown buddy intensity".to_string())?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| {
            file.buddy = crate::config::BuddySettings {
                reactions,
                intensity,
            };
        });
        store.save(next)
    })
}

fn profile_id(token: &str) -> Result<crate::config::ProfileId, String> {
    crate::config::ProfileId::from_token(token).ok_or_else(|| "unknown profile".to_string())
}

/// Rebinds one control of a built-in profile, or (`slot` = `null`) resets it to the built-in.
///
/// # Errors
/// Returns an error for an unknown profile or control, the middle button's Hold (reserved for
/// profile pin), an invalid label, shortcut or launch target, or a failed save.
#[tauri::command]
pub fn set_binding(
    handle: AppHandle,
    app: State<'_, AppState>,
    profile: String,
    control: String,
    slot: Option<crate::config::SlotSpec>,
) -> Result<ConfigDto, String> {
    let profile = profile_id(&profile)?;
    let control = crate::config::resolve::Control::from_token(&control).map_err(str::to_string)?;
    let macros = app
        .config
        .lock()
        .expect("config lock")
        .resolved()
        .macros
        .clone();
    let slot = slot
        .as_ref()
        .map(|slot| crate::config::resolve::canonical_slot(slot, &macros))
        .transpose()
        .map_err(str::to_string)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| {
            let over = file.profiles.entry(profile).or_default();
            control.set(over, slot);
            if over.is_empty() {
                file.profiles.remove(&profile);
            }
        });
        store.save(next)
    })
}

/// Changes what the knob does in a built-in profile, or (`rotate` = `null`) resets it.
///
/// # Errors
/// Returns an error for an unknown profile, an invalid shortcut or label, or a failed save.
#[tauri::command]
pub fn set_rotate(
    handle: AppHandle,
    app: State<'_, AppState>,
    profile: String,
    rotate: Option<crate::config::RotateSpec>,
) -> Result<ConfigDto, String> {
    let profile = profile_id(&profile)?;
    let rotate = rotate
        .as_ref()
        .map(crate::config::resolve::canonical_rotate)
        .transpose()
        .map_err(str::to_string)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| {
            let over = file.profiles.entry(profile).or_default();
            over.rotate = rotate;
            if over.is_empty() {
                file.profiles.remove(&profile);
            }
        });
        store.save(next)
    })
}

/// Creates or replaces one macro (by its id). Bindings to it keep working and see the new steps.
///
/// # Errors
/// Returns an error for a bad id or name, more than 8 steps or 32 macros, a step that is a macro
/// or invalid, a delay outside 50 to 2000 ms, or a failed save.
#[tauri::command]
pub fn save_macro(
    handle: AppHandle,
    app: State<'_, AppState>,
    spec: crate::config::MacroSpec,
) -> Result<ConfigDto, String> {
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        store.save_macro(spec)
    })
}

/// Deletes one macro. Refused while a control is bound to it, so nothing becomes unbound silently.
///
/// # Errors
/// Returns an error for an unknown macro, a macro still bound, or a failed save.
#[tauri::command]
pub fn delete_macro(
    handle: AppHandle,
    app: State<'_, AppState>,
    id: String,
) -> Result<ConfigDto, String> {
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        store.delete_macro(&id)
    })
}

/// Drops every override of one built-in profile.
///
/// # Errors
/// Returns an error for an unknown profile or a failed save.
#[tauri::command]
pub fn reset_profile(
    handle: AppHandle,
    app: State<'_, AppState>,
    profile: String,
) -> Result<ConfigDto, String> {
    let profile = profile_id(&profile)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| {
            file.profiles.remove(&profile);
        });
        store.save(next)
    })
}

/// Restores every setting to its default; the previous file is kept as one backup.
///
/// # Errors
/// Returns an error if the defaults could not be saved.
#[tauri::command]
pub fn reset_config(handle: AppHandle, app: State<'_, AppState>) -> Result<ConfigDto, String> {
    commit_config(
        &handle,
        &app,
        ActivityEventKind::ConfigReset,
        crate::config::ConfigStore::reset,
    )
}

/// Requests one immediate social reaction from a compatible connected device.
///
/// # Errors
/// Returns an error for an unknown action, unavailable runtime, disconnected device, old firmware,
/// or active firmware update.
#[tauri::command]
pub fn play_mascot_action(app: State<'_, AppState>, action: String) -> Result<(), String> {
    if app.firmware_busy() {
        return Err(
            "Firmware update is in progress; wait for the device to reconnect.".to_string(),
        );
    }
    let action = dto::mascot_action_from_token(&action)
        .ok_or_else(|| format!("unknown mascot action: {action}"))?;
    let status = app.status_snapshot();
    if status.connection != "connected" {
        return Err("Connect Kivori before playing a reaction.".to_string());
    }
    if !status.mascot_interaction {
        return Err("Update Kivori firmware to enable mascot interactions.".to_string());
    }
    app.send_command(DeviceCommand::PlayMascotAction(action))
}

/// The fixed bundled firmware image and the native update workflow's safe status.
#[tauri::command]
pub fn get_firmware_status(app: State<'_, AppState>) -> FirmwareStatus {
    app.firmware_status_snapshot()
}

/// Queues a flash of this application's fixed bundled firmware on the currently verified device.
///
/// The webview supplies neither a port nor a path. The device thread validates connection ownership,
/// releases the serial link, programs the image, and verifies the same device after reconnecting.
#[tauri::command]
pub fn flash_firmware(app: State<'_, AppState>) -> Result<(), String> {
    app.queue_firmware_flash()
}

/// Queues recovery of this application's fixed bundled firmware on the one Kivori device present,
/// including one that cannot handshake. Takes no arguments; the device thread finds the port and
/// refuses unless exactly one allowlisted device is connected.
#[tauri::command]
pub fn restore_firmware(app: State<'_, AppState>) -> Result<(), String> {
    app.queue_firmware_restore()
}

/// Recent session activity, oldest first, capped at `limit` (all builds).
#[tauri::command]
pub fn get_activity_log(app: State<'_, AppState>, limit: u16) -> Vec<ActivityEventDto> {
    app.activity_log
        .recent(limit as usize)
        .into_iter()
        .map(|event| dto::activity_event(&event))
        .collect()
}

/// Renders a Device Studio preview frame as raw RGBA8888 bytes (dev-only; efficient binary response).
///
/// # Errors
/// Returns an error string if `state` is not a companion token.
#[cfg(feature = "device-studio")]
#[tauri::command]
pub fn render_preview_frame(
    state: String,
    elapsed_ms: u32,
    animation: Option<crate::render::animation::AnimationTimeline>,
) -> Result<tauri::ipc::Response, String> {
    if let Some(animation) = animation {
        return Ok(tauri::ipc::Response::new(
            crate::render::render_animation_rgba(
                crate::render::bundled_blob(),
                &animation,
                elapsed_ms,
            )?,
        ));
    }
    let companion = dto::companion_from_token(&state)
        .ok_or_else(|| format!("unknown companion state: {state}"))?;
    Ok(tauri::ipc::Response::new(
        crate::render::render_preview_bundled(companion, elapsed_ms),
    ))
}

/// Mirrors the selected sendable state to the device (dev-only; identical to `set_desired_state`).
///
/// # Errors
/// Returns an error string if `state` is not a sendable token, or the device runtime is unavailable.
#[cfg(feature = "device-studio")]
#[tauri::command]
pub fn mirror_state(app: State<'_, AppState>, state: String) -> Result<(), String> {
    if app.firmware_busy() {
        return Err(
            "Firmware update is in progress; wait for the device to reconnect.".to_string(),
        );
    }
    let desired =
        dto::sendable_from_token(&state).ok_or_else(|| format!("not a sendable state: {state}"))?;
    app.send_command(DeviceCommand::MirrorDesired(desired))
}

/// The current desk projection (initial sync; `desk://status` carries changes).
#[tauri::command]
pub fn get_desk_status(app: State<'_, AppState>) -> crate::ipc::dto::DeskStatusDto {
    app.desk_snapshot()
}

/// Selects the device's full-screen view.
///
/// # Errors
/// Returns an error string if `mode` is not a display-mode token.
#[tauri::command]
pub fn set_display_mode(app: State<'_, AppState>, mode: String) -> Result<(), String> {
    let mode = crate::ipc::dto::display_mode_from_token(&mode)
        .ok_or_else(|| format!("unknown display mode: {mode}"))?;
    app.send_command(DeviceCommand::SetDisplayMode(mode))
}

/// The action catalog: what can be bound, how each is verified, and what is available here.
#[tauri::command]
pub fn list_action_catalog() -> Vec<dto::ActionCatalogEntryDto> {
    use crate::desk::catalog::{Os, Services};
    let os = Os::current();
    dto::action_catalog(os, &Services::expected(os))
}

/// Tries one action now, exactly as if its control fired (all builds). The spec is validated
/// here, at the trust boundary, with the same rules as saving a binding. The outcome arrives via
/// `desk://status` `lastAction`. A protected foreground refuses everything but system actions.
///
/// # Errors
/// Returns an error string for an invalid spec or an unavailable device runtime.
#[tauri::command]
pub fn test_action(
    app: State<'_, AppState>,
    action: crate::config::ActionSpec,
) -> Result<(), String> {
    let macros = app
        .config
        .lock()
        .expect("config lock")
        .resolved()
        .macros
        .clone();
    let action =
        crate::config::resolve::resolve_action(&action, &macros).map_err(str::to_string)?;
    app.send_command(DeviceCommand::TestAction(action))
}
