//! Kivori desktop native core (library side).
//!
//! Host-side preview rendering for Device Studio uses the SAME shared `kivori-renderer` as the
//! firmware (constitution Principle II). The Tauri command surface, connection manager, and window
//! lifecycle are added in their feature phases; the binary (`main.rs`) wires them together.

pub mod action;
pub mod activity;
pub mod companion;
pub mod device;
pub mod firmware;
pub mod input;
pub mod ipc;
pub mod orchestrator;
pub mod platform;
pub mod presentation;
pub mod render;
pub mod runtime;
pub mod window_lifecycle;

/// Launches the Tauri application runtime. Called by the thin `main.rs` binary.
pub fn run() {
    // Structured, redacted logging (ADR-0005): raw payloads are never logged (no `debug-payloads`).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            runtime::lifecycle::show_main(app);
        }))
        .on_window_event(runtime::lifecycle::on_window_event)
        .setup(|app| {
            use tauri::Manager;
            let (commands_tx, commands_rx) = std::sync::mpsc::channel();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let status = std::sync::Arc::new(std::sync::Mutex::new(ipc::dto::initial_status()));
            let activity_log = std::sync::Arc::new(activity::ActivityLog::new(256));
            let firmware_status =
                std::sync::Arc::new(std::sync::Mutex::new(firmware::initial_status()));
            let device_thread = runtime::device_task::spawn(
                app.handle().clone(),
                std::sync::Arc::clone(&status),
                std::sync::Arc::clone(&activity_log),
                std::sync::Arc::clone(&firmware_status),
                commands_rx,
                std::sync::Arc::clone(&cancel),
            );
            app.manage(runtime::state::AppState::new_with_firmware(
                cfg!(feature = "device-studio"),
                status,
                activity_log,
                firmware_status,
                commands_tx,
                cancel,
                device_thread,
            ));
            runtime::lifecycle::build_tray(app.handle())?;
            Ok(())
        });

    #[cfg(feature = "device-studio")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        ipc::commands::get_app_info,
        ipc::commands::get_connection_status,
        ipc::commands::list_states,
        ipc::commands::set_desired_state,
        ipc::commands::configure_companion,
        ipc::commands::play_mascot_action,
        ipc::commands::get_activity_log,
        ipc::commands::get_firmware_status,
        ipc::commands::flash_firmware,
        ipc::commands::render_preview_frame,
        ipc::commands::mirror_state,
        ipc::channels::open_preview_stream,
        ipc::channels::close_preview_stream,
        ipc::channels::ack_preview_frame,
        ipc::channels::update_preview_stream,
    ]);
    #[cfg(not(feature = "device-studio"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        ipc::commands::get_app_info,
        ipc::commands::get_connection_status,
        ipc::commands::list_states,
        ipc::commands::set_desired_state,
        ipc::commands::configure_companion,
        ipc::commands::play_mascot_action,
        ipc::commands::get_activity_log,
        ipc::commands::get_firmware_status,
        ipc::commands::flash_firmware,
    ]);

    builder
        .build(tauri::generate_context!())
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
