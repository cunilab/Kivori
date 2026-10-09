//! Redacted native → webview events (contracts/ipc.md §2). Payloads are the safe DTOs only — never
//! raw serial bytes or hardware identifiers. All the semantic changes the UI needs (connection /
//! compatible / incompatible / disconnect / reconnect / desired / reported) are surfaced through the
//! single three-axis `connection://status` event; typed session activity (including lifecycle changes)
//! through `activity-log://event`.

use tauri::{AppHandle, Emitter};

use crate::activity::ActivityEvent;
use crate::ipc::dto::{self, ActivityEventDto, ConfigDto, ConnectionStatusDto, DeskStatusDto};

/// Event name for connection-snapshot changes.
pub const CONNECTION_STATUS: &str = "connection://status";
/// Event name for desk projection changes (mode, monitoring, last action).
pub const DESK_STATUS: &str = "desk://status";
/// Event name for a saved or reset config.
pub const CONFIG_CHANGED: &str = "config://changed";
/// Event name for typed session activity records.
pub const ACTIVITY_LOG_EVENT: &str = "activity-log://event";

/// The payload passed to Tauri when a native activity record is emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityLogEmission {
    pub name: &'static str,
    pub payload: ActivityEventDto,
}

/// Broadcasts the latest connection snapshot to the webview.
pub fn emit_status(app: &AppHandle, status: &ConnectionStatusDto) {
    let _ = app.emit(CONNECTION_STATUS, status);
}

/// Converts one recorded activity entry into its live Tauri emission contract.
#[must_use]
pub fn activity_log_emission(event: &ActivityEvent) -> ActivityLogEmission {
    ActivityLogEmission {
        name: ACTIVITY_LOG_EVENT,
        payload: dto::activity_event(event),
    }
}

/// Broadcasts a typed activity record to the webview.
pub fn emit_activity_log(app: &AppHandle, event: &ActivityEvent) {
    let emission = activity_log_emission(event);
    let _ = app.emit(emission.name, &emission.payload);
}

/// Broadcasts the latest desk projection to the webview.
pub fn emit_desk_status(app: &AppHandle, status: &DeskStatusDto) {
    let _ = app.emit(DESK_STATUS, status);
}

/// Broadcasts the config after every successful save or reset.
pub fn emit_config_changed(app: &AppHandle, config: &ConfigDto) {
    let _ = app.emit(CONFIG_CHANGED, config);
}
