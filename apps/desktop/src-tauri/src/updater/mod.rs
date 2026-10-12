//! In-app updates (ADR-0015). This module is the ONLY first-party code allowed network access
//! (offline-first rule; `scripts/check-offline-deps.sh` and `tests/offline_smoke.rs` enforce it).
//!
//! `tauri-plugin-updater` is used from Rust only: no JS package and no permission in
//! `capabilities/`, so the webview reaches it only through our typed commands. The plugin is
//! registered, and the worker thread started, only on Windows with a release config that names the
//! release endpoint and a public key ([`policy::availability`]). Every other build is `disabled` or
//! `unsupported` and makes no request at all. The worker makes one kind of request: the endpoint's
//! `latest.json`, then (after "Install & restart") the installer it names, which the plugin checks
//! against the public key before anything runs.

pub mod policy;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use semver::Version;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::activity::{self, ActivityEventKind};
use crate::config::UpdateSettings;
use crate::ipc::dto::{self, UpdateStatusDto};
use crate::ipc::events;
use crate::runtime::state::AppState;
use policy::{Availability, Busy, CheckOutcome, CheckReport, Machine, Release, Schedule};

/// Event name for a changed updater status.
pub const STATUS_EVENT: &str = "updater://status";
/// Event name asking the window to show the update dialog (the tray's "Update to vX.Y.Z…").
pub const SHOW_EVENT: &str = "updater://show";

/// How long one `latest.json` request may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the installer download may take.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// How often a held offer or a ready install looks again for an idle device.
const IDLE_POLL: Duration = Duration::from_secs(1);
/// At most this often a download progress update is sent to the webview.
const PROGRESS_EVERY: Duration = Duration::from_millis(150);
/// The worker wakes at least this often even with nothing scheduled.
const MAX_SLEEP: Duration = Duration::from_secs(60 * 60);

/// Whether this build may check for updates, from the merged Tauri config.
#[must_use]
pub fn availability(config: &tauri::Config) -> Availability {
    policy::availability(config.plugins.0.get("updater"), cfg!(windows))
}

/// Registers the updater plugin only when it will be used. The plugin requires its config at
/// init, so a build without one (development) never registers it and cannot fail to start.
#[must_use]
pub fn register<R: Runtime>(
    builder: tauri::Builder<R>,
    availability: Availability,
) -> tauri::Builder<R> {
    if availability == Availability::Enabled {
        builder.plugin(tauri_plugin_updater::Builder::new().build())
    } else {
        builder
    }
}

/// A request from a command to the worker thread.
enum Command {
    /// The command already moved the machine to `checking`.
    Check,
    /// The command already moved the machine to `downloading`.
    Install,
    AutoCheck(bool),
    /// A snooze ends in this many seconds.
    WakeIn(u64),
}

/// Tauri-managed updater state.
pub struct Updates {
    machine: Mutex<Machine>,
    /// `None` when the updater is disabled or unsupported: there is no worker.
    worker: Mutex<Option<Sender<Command>>>,
    /// Written by the device thread: a knob gesture is open.
    gesture: Arc<AtomicBool>,
}

/// Manages the updater state and, when enabled, starts the worker thread (its first check is
/// [`policy::FIRST_CHECK_DELAY`] away, so startup never waits on it).
pub fn start(
    app: &AppHandle,
    availability: Availability,
    gesture: Arc<AtomicBool>,
    auto_check: bool,
) {
    let (tx, rx) = mpsc::channel();
    let worker = (availability == Availability::Enabled).then_some(tx);
    let enabled = worker.is_some();
    app.manage(Updates {
        machine: Mutex::new(Machine::new(availability)),
        worker: Mutex::new(worker),
        gesture,
    });
    if !enabled {
        return;
    }
    let handle = app.clone();
    let spawned = std::thread::Builder::new()
        .name("kivori-updater".to_string())
        .spawn(move || run(&handle, &rx, auto_check));
    if spawned.is_err() {
        tracing::warn!("app updates are off: the updater thread did not start");
    }
}

/// The status the Device page and the update dialog show.
#[must_use]
pub fn status(app: &AppHandle) -> UpdateStatusDto {
    let auto_check = settings(app).auto_check;
    let Some(updates) = app.try_state::<Updates>() else {
        return dto::update_status_dto(&Machine::new(Availability::Unsupported), auto_check);
    };
    let machine = updates.machine.lock().expect("updater lock");
    dto::update_status_dto(&machine, auto_check)
}

/// The version the tray offers, if any.
#[must_use]
pub fn tray_offer(app: &AppHandle) -> Option<String> {
    let updates = app.try_state::<Updates>()?;
    let machine = updates.machine.lock().expect("updater lock");
    machine.offered().map(Version::to_string)
}

/// Whether an app update is due, so the firmware offer waits for it (see
/// [`Machine::app_update_due`]). `false` when the updater is off.
#[must_use]
pub fn app_update_due(app: &AppHandle) -> bool {
    app.try_state::<Updates>().is_some_and(|updates| {
        updates
            .machine
            .lock()
            .expect("updater lock")
            .app_update_due()
    })
}

/// The version currently offered, for "Later" and "Skip this version".
///
/// # Errors
/// A fixed message when nothing is offered.
pub fn offered(app: &AppHandle) -> Result<Version, String> {
    app.try_state::<Updates>()
        .and_then(|updates| {
            updates
                .machine
                .lock()
                .expect("updater lock")
                .offered()
                .cloned()
        })
        .ok_or_else(|| policy::Refused::NothingOffered.message().to_string())
}

/// "Check now".
///
/// # Errors
/// A fixed message when this build does not check for updates.
pub fn check_now(app: &AppHandle) -> Result<UpdateStatusDto, String> {
    let updates = app.try_state::<Updates>().ok_or_else(not_available)?;
    let begun = updates.machine.lock().expect("updater lock").begin_check();
    if begun {
        send(&updates, Command::Check)?;
        publish(app);
    }
    Ok(status(app))
}

/// "Install & restart" or "Try again".
///
/// # Errors
/// A fixed message when nothing is offered, an install already runs, or updates are off.
pub fn install(app: &AppHandle) -> Result<UpdateStatusDto, String> {
    let updates = app.try_state::<Updates>().ok_or_else(not_available)?;
    updates
        .machine
        .lock()
        .expect("updater lock")
        .begin_install()
        .map_err(|refused| refused.message().to_string())?;
    if let Err(error) = send(&updates, Command::Install) {
        updates
            .machine
            .lock()
            .expect("updater lock")
            .install_failed();
        publish(app);
        return Err(error);
    }
    publish(app);
    Ok(status(app))
}

/// After "Later" or "Skip this version" was saved: withdraws the offer. `snooze_secs` is when to
/// look again (a snooze), `None` for a skip.
pub fn dismissed(app: &AppHandle, snooze_secs: Option<u64>) -> UpdateStatusDto {
    if let Some(updates) = app.try_state::<Updates>() {
        // Refused only while an install runs, which then carries on.
        let _ = updates.machine.lock().expect("updater lock").dismiss();
        if let Some(secs) = snooze_secs {
            let _ = send(&updates, Command::WakeIn(secs));
        }
    }
    publish(app);
    status(app)
}

/// After the automatic-check setting was saved.
pub fn auto_check_changed(app: &AppHandle, enabled: bool) -> UpdateStatusDto {
    if let Some(updates) = app.try_state::<Updates>() {
        let _ = send(&updates, Command::AutoCheck(enabled));
    }
    publish(app);
    status(app)
}

fn not_available() -> String {
    "Updates are not available in this build.".to_string()
}

fn send(updates: &Updates, command: Command) -> Result<(), String> {
    updates
        .worker
        .lock()
        .expect("updater lock")
        .as_ref()
        .ok_or_else(not_available)?
        .send(command)
        .map_err(|_| not_available())
}

/// Sends the status to the webview and refreshes the tray.
fn publish(app: &AppHandle) {
    let _ = app.emit(STATUS_EVENT, status(app));
    if let Some(state) = app.try_state::<AppState>() {
        crate::runtime::lifecycle::refresh_tray(app, &state.status_snapshot());
    }
}

fn settings(app: &AppHandle) -> UpdateSettings {
    app.try_state::<AppState>()
        .map_or_else(UpdateSettings::default, |state| {
            state
                .config
                .lock()
                .expect("config lock")
                .file()
                .updates
                .clone()
        })
}

fn busy(app: &AppHandle) -> Busy {
    Busy {
        flash: app
            .try_state::<AppState>()
            .is_some_and(|state| state.firmware_busy()),
        gesture: app
            .try_state::<Updates>()
            .is_some_and(|updates| updates.gesture.load(Ordering::Relaxed)),
    }
}

fn record(app: &AppHandle, kind: ActivityEventKind) {
    if let Some(state) = app.try_state::<AppState>() {
        let event = state.activity_log.record(kind, None);
        events::emit_activity_log(app, &event);
    }
}

fn with_machine<T>(app: &AppHandle, f: impl FnOnce(&mut Machine) -> T) -> Option<T> {
    let updates = app.try_state::<Updates>()?;
    let mut machine = updates.machine.lock().expect("updater lock");
    Some(f(&mut machine))
}

fn running_version(app: &AppHandle) -> Version {
    let version = app.package_info().version.to_string();
    Version::parse(&version).unwrap_or_else(|_| Version::new(0, 0, 0))
}

/// The worker: the schedule, the checks, and the install. Blocking calls run here, never on the
/// UI or device thread.
fn run(app: &AppHandle, rx: &Receiver<Command>, auto_check: bool) {
    let started = Instant::now();
    let mut schedule = Schedule::new(auto_check);
    // The plugin's handle on the release last found; the install downloads from it.
    let mut found: Option<Update> = None;
    loop {
        let now = started.elapsed();
        let holding = with_machine(app, |m| m.holding()).unwrap_or(false);
        let mut timeout = schedule.wait(now).unwrap_or(MAX_SLEEP).min(MAX_SLEEP);
        if holding {
            timeout = timeout.min(IDLE_POLL);
        }
        match rx.recv_timeout(timeout) {
            Ok(Command::Check) => run_check(app, &mut schedule, started, &mut found, true),
            Ok(Command::Install) => run_install(app, found.as_ref()),
            Ok(Command::AutoCheck(on)) => schedule.set_auto(on, started.elapsed()),
            Ok(Command::WakeIn(secs)) => {
                schedule.wake_by(started.elapsed() + Duration::from_secs(secs));
            }
            Err(RecvTimeoutError::Timeout) => {
                let busy = busy(app);
                if with_machine(app, |m| m.settle(busy)).unwrap_or(false) {
                    record(app, ActivityEventKind::AppUpdateAvailable);
                    publish(app);
                }
                if schedule.due(started.elapsed()) {
                    let begun = with_machine(app, Machine::begin_check).unwrap_or(false);
                    if begun {
                        publish(app);
                        run_check(app, &mut schedule, started, &mut found, false);
                    } else {
                        // An offer is open or an install runs: look again a day later.
                        schedule.checked(started.elapsed());
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// One check; the machine is already `checking`.
fn run_check(
    app: &AppHandle,
    schedule: &mut Schedule,
    started: Instant,
    found: &mut Option<Update>,
    manual: bool,
) {
    let (outcome, update) = check(app);
    schedule.checked(started.elapsed());
    let running = running_version(app);
    let settings = settings(app);
    let busy = busy(app);
    let report = with_machine(app, |m| {
        m.check_finished(
            outcome,
            &running,
            &settings,
            activity::unix_now(),
            manual,
            busy,
        )
    });
    match report {
        Some(CheckReport::Offered) => {
            *found = update;
            record(app, ActivityEventKind::AppUpdateAvailable);
        }
        Some(CheckReport::Held) => *found = update,
        // A held release (found while busy) survives a failed check.
        Some(CheckReport::Failed) => record(app, ActivityEventKind::AppUpdateCheckFailed),
        Some(CheckReport::NothingToOffer) => *found = None,
        None => {}
    }
    publish(app);
}

/// Asks the endpoint for `latest.json`. Errors are not logged in detail: they can carry URLs.
fn check(app: &AppHandle) -> (CheckOutcome, Option<Update>) {
    let updater = app
        .updater_builder()
        .timeout(CHECK_TIMEOUT)
        .on_before_exit(before_exit(app))
        .build();
    let Ok(updater) = updater else {
        tracing::info!("app update check could not start");
        return (CheckOutcome::Failed, None);
    };
    match tauri::async_runtime::block_on(updater.check()) {
        Ok(None) => (CheckOutcome::UpToDate, None),
        Ok(Some(mut update)) => match Version::parse(&update.version) {
            Ok(version) => {
                let release = Release::new(
                    version,
                    update.body.as_deref(),
                    update
                        .raw_json
                        .get("pub_date")
                        .and_then(serde_json::Value::as_str),
                );
                update.timeout = Some(DOWNLOAD_TIMEOUT);
                (CheckOutcome::Found(release), Some(update))
            }
            Err(_) => (CheckOutcome::Failed, None),
        },
        Err(_) => {
            tracing::info!("app update check failed");
            (CheckOutcome::Failed, None)
        }
    }
}

/// Downloads and verifies the update, waits until no flash or gesture runs, then starts the
/// installer (passive NSIS), which closes Kivori and starts the new version.
fn run_install(app: &AppHandle, update: Option<&Update>) {
    let Some(update) = update else {
        fail(app);
        return;
    };
    record(app, ActivityEventKind::AppUpdateInstallStarted);
    let mut last_sent: Option<Instant> = None;
    let downloaded = tauri::async_runtime::block_on(update.download(
        |chunk, total| {
            with_machine(app, |m| m.progress(chunk as u64, total));
            if last_sent.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY) {
                last_sent = Some(Instant::now());
                publish(app);
            }
        },
        || {},
    ));
    // The plugin verified the signature against the public key before returning the bytes.
    let Ok(bytes) = downloaded else {
        fail(app);
        return;
    };
    with_machine(app, Machine::downloaded);
    publish(app);
    while busy(app).any() {
        std::thread::sleep(IDLE_POLL);
    }
    // On Windows this exits the app once the installer has started.
    match update.install(bytes) {
        Ok(()) => app.restart(),
        Err(_) => fail(app),
    }
}

fn fail(app: &AppHandle) {
    tracing::info!("app update install failed");
    with_machine(app, Machine::install_failed);
    record(app, ActivityEventKind::AppUpdateInstallFailed);
    publish(app);
}

/// Before the installer replaces the app: stop the device thread (it releases the serial port),
/// then Tauri's own cleanup.
fn before_exit(app: &AppHandle) -> impl Fn() + Send + Sync + 'static {
    let app = app.clone();
    move || {
        if let Some(state) = app.try_state::<AppState>() {
            state.shutdown();
        }
        app.cleanup_before_exit();
    }
}
