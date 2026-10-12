//! Kivori desktop native core (library side).
//!
//! Host-side preview rendering for Device Studio uses the SAME shared `kivori-renderer` as the
//! firmware (constitution Principle II). The Tauri command surface, connection manager, and window
//! lifecycle are added in their feature phases; the binary (`main.rs`) wires them together.

pub mod action;
pub mod activity;
pub mod catalog;
pub mod companion;
pub mod config;
pub mod desk;
pub mod device;
pub mod firmware;
pub mod firmware_marker;
pub mod identity;
pub mod input;
pub mod ipc;
pub mod orchestrator;
pub mod platform;
pub mod presentation;
#[cfg(feature = "device-studio")]
pub mod provision;
pub mod render;
pub mod runtime;
pub mod startup;
pub mod updater;
pub mod window_lifecycle;

/// Launches the Tauri application runtime. Called by the thin `main.rs` binary.
pub fn run() {
    // Structured, redacted logging (ADR-0005): raw payloads are never logged (no `debug-payloads`).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let context = tauri::generate_context!();
    // App updates (ADR-0015): only a Windows release build with the endpoint and public key in its
    // config registers the updater plugin; any other build makes no request at all.
    let updates = updater::availability(context.config());

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            runtime::lifecycle::show_main(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![window_lifecycle::AUTOSTART_ARG]),
        ))
        .on_window_event(runtime::lifecycle::on_window_event)
        .setup(move |app| {
            use tauri::Manager;
            let (commands_tx, commands_rx) = std::sync::mpsc::channel();
            let (host_tx, host_rx) = std::sync::mpsc::channel();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let status = std::sync::Arc::new(std::sync::Mutex::new(ipc::dto::initial_status()));
            let activity_log = std::sync::Arc::new(activity::ActivityLog::new(256));
            // Per-user, per-machine: `app_local_data_dir` is Local (not Roaming) AppData on Windows.
            // Without it the app still runs on defaults; saving settings then reports an error.
            let config_store = app.path().app_local_data_dir().map_or_else(
                |_| config::ConfigStore::detached(),
                |dir| config::ConfigStore::open(&dir),
            );
            match config_store.notice() {
                Some(
                    config::ConfigNotice::RecoveredCorrupt
                    | config::ConfigNotice::RecoveredNewerVersion,
                ) => {
                    activity_log.record(activity::ActivityEventKind::ConfigRecovered, None);
                }
                Some(config::ConfigNotice::Migrated) => {
                    activity_log.record(activity::ActivityEventKind::ConfigSaved, None);
                }
                None => {}
            }
            let resolved = config_store.resolved();
            let desk_status = std::sync::Arc::new(std::sync::Mutex::new(
                ipc::dto::initial_desk_status(&resolved),
            ));
            let firmware_status =
                std::sync::Arc::new(std::sync::Mutex::new(firmware::initial_status()));
            let diagnostics = std::sync::Arc::new(std::sync::Mutex::new(
                ipc::dto::DiagnosticsSnapshot::initial(),
            ));
            let services = desk::catalog::ServicesCell::default();
            let knob_gesture = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let auto_update_check = config_store.file().updates.auto_check;
            let device_thread = runtime::device_task::spawn(
                app.handle().clone(),
                std::sync::Arc::clone(&status),
                std::sync::Arc::clone(&desk_status),
                std::sync::Arc::clone(&activity_log),
                std::sync::Arc::clone(&firmware_status),
                std::sync::Arc::clone(&diagnostics),
                std::sync::Arc::clone(&services),
                commands_rx,
                host_rx,
                std::sync::Arc::clone(&cancel),
                resolved,
                std::sync::Arc::clone(&knob_gesture),
            );
            app.manage(
                runtime::state::AppState::new_with_firmware(
                    cfg!(feature = "device-studio"),
                    status,
                    activity_log,
                    firmware_status,
                    commands_tx,
                    cancel,
                    device_thread,
                )
                .with_desk_status(desk_status)
                .with_diagnostics(diagnostics)
                .with_services(services)
                .with_config(std::sync::Arc::new(std::sync::Mutex::new(config_store))),
            );
            // After the device thread exists, so the first lock or sleep notice has a reader.
            app.manage(runtime::state::HostEventsHandle::start(host_tx));
            // The first check is 30 s away: startup never waits on the network.
            updater::start(app.handle(), updates, knob_gesture, auto_update_check);
            runtime::lifecycle::build_tray(app.handle())?;
            // The window is configured hidden. A login launch keeps it that way (tray only). Any
            // other launch shows it once the frontend calls `app_ready`, behind a splash if that
            // is slow (see `runtime::splash`).
            let readiness = std::sync::Arc::new(runtime::splash::Readiness::default());
            app.manage(std::sync::Arc::clone(&readiness));
            if window_lifecycle::launched_at_login(std::env::args()) {
                *app.state::<runtime::state::AppState>()
                    .lifecycle
                    .lock()
                    .expect("lifecycle lock") = window_lifecycle::WindowLifecycle::hidden();
            } else {
                runtime::splash::watch_startup(app.handle(), readiness);
            }
            Ok(())
        });

    #[cfg(feature = "device-studio")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        ipc::commands::get_app_info,
        ipc::commands::app_ready,
        ipc::commands::get_connection_status,
        ipc::commands::list_states,
        ipc::commands::set_desired_state,
        ipc::commands::get_config,
        ipc::commands::get_onboarding,
        ipc::commands::complete_onboarding,
        ipc::commands::restart_onboarding,
        ipc::commands::get_accessibility,
        ipc::commands::request_accessibility,
        ipc::commands::open_accessibility_settings,
        ipc::commands::get_startup_settings,
        ipc::commands::set_launch_at_login,
        ipc::commands::set_display_settings,
        ipc::commands::set_brightness,
        ipc::commands::set_buddy_settings,
        ipc::commands::set_binding,
        ipc::commands::set_rotate,
        ipc::commands::reset_profile,
        ipc::commands::add_custom_profile,
        ipc::commands::assign_app,
        ipc::commands::rename_profile,
        ipc::commands::delete_profile,
        ipc::commands::set_profile_pin_cycle,
        ipc::commands::list_running_apps,
        ipc::commands::get_app_icon,
        ipc::commands::reset_config,
        ipc::commands::save_macro,
        ipc::commands::delete_macro,
        ipc::commands::play_mascot_action,
        ipc::commands::get_activity_log,
        ipc::commands::get_firmware_status,
        ipc::commands::get_diagnostics,
        ipc::commands::get_device_internals,
        ipc::commands::flash_firmware,
        ipc::commands::restore_firmware,
        ipc::commands::render_preview_frame,
        ipc::commands::mirror_state,
        ipc::commands::get_desk_status,
        ipc::commands::set_display_mode,
        ipc::commands::list_action_catalog,
        ipc::commands::test_action,
        ipc::commands::test_rotate,
        ipc::commands::get_update_status,
        ipc::commands::check_for_updates,
        ipc::commands::install_update,
        ipc::commands::snooze_update,
        ipc::commands::skip_update_version,
        ipc::commands::set_auto_update_check,
        ipc::channels::open_preview_stream,
        ipc::channels::close_preview_stream,
        ipc::channels::ack_preview_frame,
        ipc::channels::update_preview_stream,
    ]);
    #[cfg(not(feature = "device-studio"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        ipc::commands::get_app_info,
        ipc::commands::app_ready,
        ipc::commands::get_connection_status,
        ipc::commands::list_states,
        ipc::commands::set_desired_state,
        ipc::commands::get_config,
        ipc::commands::get_onboarding,
        ipc::commands::complete_onboarding,
        ipc::commands::restart_onboarding,
        ipc::commands::get_accessibility,
        ipc::commands::request_accessibility,
        ipc::commands::open_accessibility_settings,
        ipc::commands::get_startup_settings,
        ipc::commands::set_launch_at_login,
        ipc::commands::set_display_settings,
        ipc::commands::set_brightness,
        ipc::commands::set_buddy_settings,
        ipc::commands::set_binding,
        ipc::commands::set_rotate,
        ipc::commands::reset_profile,
        ipc::commands::add_custom_profile,
        ipc::commands::assign_app,
        ipc::commands::rename_profile,
        ipc::commands::delete_profile,
        ipc::commands::set_profile_pin_cycle,
        ipc::commands::list_running_apps,
        ipc::commands::get_app_icon,
        ipc::commands::reset_config,
        ipc::commands::save_macro,
        ipc::commands::delete_macro,
        ipc::commands::play_mascot_action,
        ipc::commands::get_activity_log,
        ipc::commands::get_firmware_status,
        ipc::commands::get_diagnostics,
        ipc::commands::get_device_internals,
        ipc::commands::flash_firmware,
        ipc::commands::restore_firmware,
        ipc::commands::get_desk_status,
        ipc::commands::set_display_mode,
        ipc::commands::list_action_catalog,
        ipc::commands::test_action,
        ipc::commands::test_rotate,
        ipc::commands::get_update_status,
        ipc::commands::check_for_updates,
        ipc::commands::install_update,
        ipc::commands::snooze_update,
        ipc::commands::skip_update_version,
        ipc::commands::set_auto_update_check,
    ]);

    updater::register(builder, updates)
        .build(context)
        .expect("error while building the Kivori application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::ExitRequested { .. } => {
                use tauri::Manager;
                app_handle.state::<runtime::state::AppState>().shutdown();
            }
            // macOS Dock click while the window is hidden (FR-030 re-activate).
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => runtime::lifecycle::show_main(app_handle),
            _ => {}
        });
}
