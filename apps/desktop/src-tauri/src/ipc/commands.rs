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
    self, ActivityEventDto, AppInfoDto, ConfigDto, ConnectionStatusDto, DeviceInternalsDto,
    DiagnosticsDto, OnboardingDto, StartupSettingsDto, UpdateStatusDto,
};
use crate::ipc::events;
use crate::runtime::state::{AppState, DeviceCommand};
use kivori_model::CompanionState;

/// Application/build info (all builds).
#[tauri::command]
pub fn get_app_info(app: State<'_, AppState>) -> AppInfoDto {
    dto::app_info(app.device_studio_enabled)
}

/// The frontend has rendered: lets the startup watcher show the main window (all builds). A no-op
/// on a login launch, which stays hidden.
#[tauri::command]
pub fn app_ready(readiness: State<'_, std::sync::Arc<crate::runtime::splash::Readiness>>) {
    readiness.mark_ready();
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

/// The Device internals card (developer mode). Asks the device for a fresh snapshot and returns the
/// latest one it has already answered with, so a card polling about once a second always shows the
/// previous reply and the device is only ever asked while the card is visible.
#[tauri::command]
pub fn get_device_internals(app: State<'_, AppState>) -> DeviceInternalsDto {
    // The device thread only stops at shutdown; a failed send just leaves the cached figures.
    let _ = app.send_command(DeviceCommand::RequestInternals);
    app.internals_snapshot()
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

/// Whether first-run setup is finished (all builds).
#[tauri::command]
pub fn get_onboarding(app: State<'_, AppState>) -> OnboardingDto {
    app.onboarding_snapshot()
}

/// Marks first-run setup finished (also used by "Skip setup").
///
/// # Errors
/// Returns an error when the setting cannot be saved.
#[tauri::command]
pub fn complete_onboarding(
    handle: AppHandle,
    app: State<'_, AppState>,
) -> Result<OnboardingDto, String> {
    set_onboarding(&handle, &app, true)
}

/// Marks first-run setup not finished, so the Device page's "Run setup again" can reopen it.
///
/// # Errors
/// Returns an error when the setting cannot be saved.
#[tauri::command]
pub fn restart_onboarding(
    handle: AppHandle,
    app: State<'_, AppState>,
) -> Result<OnboardingDto, String> {
    set_onboarding(&handle, &app, false)
}

fn set_onboarding(
    handle: &AppHandle,
    app: &AppState,
    completed: bool,
) -> Result<OnboardingDto, String> {
    commit_config(handle, app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| file.onboarding.completed = completed);
        store.save(next)
    })
    .map(|_| OnboardingDto { completed })
}

/// The app updater's status (all builds). A development build reports `disabled`, macOS
/// `unsupported`; neither ever makes a request.
#[tauri::command]
pub fn get_update_status(app: AppHandle) -> UpdateStatusDto {
    crate::updater::status(&app)
}

/// "Check now": starts a check and returns at once; `updater://status` carries the result. An
/// offline or failed check changes nothing and reports nothing (ADR-0015).
///
/// # Errors
/// Returns an error when this build does not check for updates.
#[tauri::command]
pub fn check_for_updates(app: AppHandle) -> Result<UpdateStatusDto, String> {
    crate::updater::check_now(&app)
}

/// "Install & restart" (or "Try again"): downloads and verifies the offered update, then installs
/// it and restarts as soon as no firmware flash or knob gesture is running.
///
/// # Errors
/// Returns an error when nothing is offered or an install already runs.
#[tauri::command]
pub fn install_update(app: AppHandle) -> Result<UpdateStatusDto, String> {
    crate::updater::install(&app)
}

/// "Later": the offered version is not offered again for 3 days (saved in the config).
///
/// # Errors
/// Returns an error when nothing is offered or the setting cannot be saved.
#[tauri::command]
pub fn snooze_update(
    handle: AppHandle,
    app: State<'_, AppState>,
) -> Result<UpdateStatusDto, String> {
    let version = crate::updater::offered(&handle)?;
    let snooze = crate::updater::policy::snooze(&version, crate::activity::unix_now());
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| file.updates.snoozed = Some(snooze));
        store.save(next)
    })?;
    Ok(crate::updater::dismissed(
        &handle,
        Some(crate::updater::policy::SNOOZE_SECS),
    ))
}

/// "Skip this version": the offered version is never offered again; a newer one is.
///
/// # Errors
/// Returns an error when nothing is offered or the setting cannot be saved.
#[tauri::command]
pub fn skip_update_version(
    handle: AppHandle,
    app: State<'_, AppState>,
) -> Result<UpdateStatusDto, String> {
    let version = crate::updater::offered(&handle)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| {
            file.updates.skipped_version = Some(version.to_string());
            file.updates.snoozed = None;
        });
        store.save(next)
    })?;
    Ok(crate::updater::dismissed(&handle, None))
}

/// "Check for updates automatically" (saved in the config).
///
/// # Errors
/// Returns an error when the setting cannot be saved.
#[tauri::command]
pub fn set_auto_update_check(
    handle: AppHandle,
    app: State<'_, AppState>,
    enabled: bool,
) -> Result<UpdateStatusDto, String> {
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| file.updates.auto_check = enabled);
        store.save(next)
    })?;
    Ok(crate::updater::auto_check_changed(&handle, enabled))
}

/// The Accessibility permission as `granted`, `missing` or `notApplicable` (all builds). Never
/// prompts.
#[tauri::command]
pub fn get_accessibility() -> &'static str {
    crate::platform::permissions::accessibility().token()
}

/// Asks macOS to show its Accessibility prompt, and returns the state right after.
#[tauri::command]
pub fn request_accessibility() -> &'static str {
    crate::platform::permissions::request_accessibility().token()
}

/// Opens System Settings at the Accessibility list.
///
/// # Errors
/// Returns a fixed message when System Settings could not be opened.
#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    crate::platform::permissions::open_accessibility_settings()
        .map_err(|()| "System Settings could not be opened.".to_string())
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
        let next = store.edited(|file| {
            file.display = crate::config::DisplaySettings {
                brightness: file.display.brightness,
                ..display
            };
        });
        store.save(next)
    })
}

/// Sets the backlight brightness step (`1..=5`); the device follows at once.
///
/// # Errors
/// Returns an error for a step the device does not have or a failed save.
#[tauri::command]
pub fn set_brightness(
    handle: AppHandle,
    app: State<'_, AppState>,
    step: u8,
) -> Result<ConfigDto, String> {
    if !(1..=kivori_model::desk::BRIGHTNESS_STEPS).contains(&step) {
        return Err("unknown brightness step".to_string());
    }
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let next = store.edited(|file| file.display.brightness = step);
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

fn profile_ref(token: &str) -> Result<crate::config::ProfileRef, String> {
    crate::config::ProfileRef::parse(token).ok_or_else(|| "unknown profile".to_string())
}

/// Rebinds one control of a profile, or (`slot` = `null`) resets it to the built-in. A custom
/// profile cannot reset a single control.
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
    let profile = profile_ref(&profile)?;
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
        let mut next = store.file().clone();
        let resetting = slot.is_none();
        crate::config::profiles::edit_bindings(&mut next, &profile, resetting, |over| {
            control.set(over, slot);
        })
        .map_err(crate::config::ConfigError::Invalid)?;
        store.save(next)
    })
}

/// Changes what the knob does in a profile, or (`rotate` = `null`) resets it. A custom profile
/// cannot reset a single control.
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
    let profile = profile_ref(&profile)?;
    let rotate = rotate
        .as_ref()
        .map(crate::config::resolve::canonical_rotate)
        .transpose()
        .map_err(str::to_string)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        let mut next = store.file().clone();
        let resetting = rotate.is_none();
        crate::config::profiles::edit_bindings(&mut next, &profile, resetting, |over| {
            over.rotate = rotate;
        })
        .map_err(crate::config::ConfigError::Invalid)?;
        store.save(next)
    })
}

/// Creates a custom profile for an app, as a copy of General's bindings made now. `name` is the
/// app's display name (empty = derived from `app`); the app moves out of any profile that held it.
///
/// # Errors
/// Returns an error for an invalid app, 16 custom profiles already, or a failed save.
#[tauri::command]
pub fn add_custom_profile(
    handle: AppHandle,
    app: State<'_, AppState>,
    app_id: String,
    name: String,
) -> Result<ConfigDto, String> {
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        store.add_custom_profile(&app_id, &name)
    })
}

/// Moves one app into a profile (General = out of the profile that lists it), out of any other.
///
/// # Errors
/// Returns an error for an unknown profile, an invalid app, a profile with 32 apps, or a failed
/// save.
#[tauri::command]
pub fn assign_app(
    handle: AppHandle,
    app: State<'_, AppState>,
    profile: String,
    app_id: String,
) -> Result<ConfigDto, String> {
    let profile = profile_ref(&profile)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        store.assign_app(&profile, &app_id)
    })
}

/// Renames a custom profile.
///
/// # Errors
/// Returns an error for an unknown or built-in profile, an empty or over-long name, or a failed
/// save.
#[tauri::command]
pub fn rename_profile(
    handle: AppHandle,
    app: State<'_, AppState>,
    profile: String,
    name: String,
) -> Result<ConfigDto, String> {
    let profile = profile_ref(&profile)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        store.rename_profile(&profile, &name)
    })
}

/// Deletes a custom profile; its apps stop matching it.
///
/// # Errors
/// Returns an error for an unknown or built-in profile, or a failed save.
#[tauri::command]
pub fn delete_profile(
    handle: AppHandle,
    app: State<'_, AppState>,
    profile: String,
) -> Result<ConfigDto, String> {
    let profile = profile_ref(&profile)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        store.delete_profile(&profile)
    })
}

/// Puts a custom profile in or out of the middle button's pin cycle.
///
/// # Errors
/// Returns an error for an unknown or built-in profile, or a failed save.
#[tauri::command]
pub fn set_profile_pin_cycle(
    handle: AppHandle,
    app: State<'_, AppState>,
    profile: String,
    enabled: bool,
) -> Result<ConfigDto, String> {
    let profile = profile_ref(&profile)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        store.set_pin_cycle(&profile, enabled)
    })
}

/// The apps with a window open right now, for "pick a running app". Runs off the UI thread.
#[tauri::command]
pub async fn list_running_apps() -> Vec<dto::RunningAppDto> {
    tauri::async_runtime::spawn_blocking(|| {
        crate::platform::apps::running_apps()
            .into_iter()
            .map(|a| dto::RunningAppDto {
                id: a.id,
                name: a.name,
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// An app's icon as a PNG data URL, `null` when it cannot be read (the UI shows a letter). Runs
/// off the UI thread and the focus poller; results are cached per app id.
#[tauri::command]
pub async fn get_app_icon(app_id: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || crate::platform::apps::icon_data_url(&app_id))
        .await
        .ok()
        .flatten()
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

/// Resets one profile: a built-in goes back to its built-in bindings and app list, a custom one to
/// a fresh copy of General's bindings (its name and apps stay).
///
/// # Errors
/// Returns an error for an unknown profile or a failed save.
#[tauri::command]
pub fn reset_profile(
    handle: AppHandle,
    app: State<'_, AppState>,
    profile: String,
) -> Result<ConfigDto, String> {
    let profile = profile_ref(&profile)?;
    commit_config(&handle, &app, ActivityEventKind::ConfigSaved, |store| {
        store.reset_profile(&profile)
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
/// refuses unless exactly one allowlisted device is connected. `product` is a machine id from the
/// status's restore choices, sent back only after the app asked which product the device is.
#[tauri::command]
pub fn restore_firmware(app: State<'_, AppState>, product: Option<String>) -> Result<(), String> {
    match product {
        Some(product) => app.queue_firmware_restore_for(product),
        None => app.queue_firmware_restore(),
    }
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

/// The action catalog: what can be bound, how each is verified, and what is available here. The
/// availability is the device thread's latest snapshot, or the OS expectation before its first.
#[tauri::command]
pub fn list_action_catalog(app: State<'_, AppState>) -> Vec<dto::ActionCatalogEntryDto> {
    use crate::desk::catalog::{Os, Services};
    let os = Os::current();
    dto::action_catalog(os, &Services::current(&app.services, os))
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

/// Tries the knob's binding once (all builds). A volume knob (system or one app) goes one detent
/// clockwise and back to exactly where it was, so the net change is zero; a shortcut pair sends
/// its clockwise shortcut once. Validated and refused like [`test_action`]: a protected foreground
/// refuses the shortcut. The outcome arrives via `desk://status` `lastAction`.
///
/// # Errors
/// Returns an error string for an invalid spec or an unavailable device runtime.
#[tauri::command]
pub fn test_rotate(
    app: State<'_, AppState>,
    rotate: crate::config::RotateSpec,
) -> Result<(), String> {
    let action = crate::config::resolve::resolve_rotate_test(&rotate).map_err(str::to_string)?;
    app.send_command(DeviceCommand::TestAction(action))
}
