//! Splash window wiring over the pure [`crate::startup`] policy (M2.6).
//!
//! `setup` starts one watcher thread on a normal launch. It shows the main window once the frontend
//! calls `app_ready`, opening a small splash first only if that takes longer than the delay. A login
//! launch never starts the watcher: the window stays hidden and `app_ready` is a no-op.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::runtime::lifecycle::show_main;
use crate::runtime::state::AppState;
use crate::startup::{self, StartupPhase};

const SPLASH: &str = "splash";
const POLL: Duration = Duration::from_millis(25);
/// Matches the fade-out in `splash.css`.
const FADE: Duration = Duration::from_millis(200);

/// Set once by the `app_ready` command.
#[derive(Debug, Default)]
pub struct Readiness(AtomicBool);

impl Readiness {
    pub fn mark_ready(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    fn is_ready(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Starts the watcher for a normal (not login) launch.
pub fn watch_startup(app: &AppHandle, readiness: Arc<Readiness>) {
    let app = app.clone();
    thread::spawn(move || run(&app, &readiness));
}

fn run(app: &AppHandle, readiness: &Readiness) {
    let started = Instant::now();
    let mut splash_open = false;
    let mut last_phase = None;
    loop {
        thread::sleep(POLL);
        let elapsed = started.elapsed();
        let ready = readiness.is_ready();
        if ready || startup::fallback_due(false, ready, elapsed) {
            if !ready {
                tracing::warn!("frontend did not report ready in time; showing the window anyway");
            }
            if splash_open {
                let _ = app.emit_to(SPLASH, "splash-close", ());
                thread::sleep(FADE);
                if let Some(window) = app.get_webview_window(SPLASH) {
                    let _ = window.destroy();
                }
            }
            show_main(app);
            return;
        }
        if !splash_open && startup::splash_due(false, ready, elapsed) {
            splash_open = open_splash(app);
        }
        if splash_open {
            let phase = current_phase(app);
            if last_phase != Some(phase) {
                last_phase = Some(phase);
                let _ = app.emit_to(SPLASH, "splash-phase", phase.token());
            }
        }
    }
}

fn current_phase(app: &AppHandle) -> StartupPhase {
    let status = app.state::<AppState>().status_snapshot();
    startup::phase_for(&status.connection, status.retry_count)
}

/// Opens the undecorated splash. Returns whether it exists; failing to open it is not fatal, the
/// main window still shows when ready.
fn open_splash(app: &AppHandle) -> bool {
    let url = format!(
        "splash.html?v={}&phase={}",
        env!("CARGO_PKG_VERSION"),
        current_phase(app).token()
    );
    let built = WebviewWindowBuilder::new(app, SPLASH, WebviewUrl::App(url.into()))
        .title("Kivori")
        .inner_size(280.0, 240.0)
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .center()
        .focused(false)
        .build();
    if let Err(error) = &built {
        tracing::warn!(%error, "could not open the splash window");
    }
    built.is_ok()
}
