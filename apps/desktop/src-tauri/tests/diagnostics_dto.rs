//! The diagnostics DTO is what "Copy diagnostics" puts on the clipboard, so its keys are an
//! allowlist: anything that could identify a machine or a device must never appear in it.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use kivori_desktop::config::ConfigStore;
use kivori_desktop::device::session::SessionDiagnostics;
use kivori_desktop::ipc::dto::{capability_names, diagnostics_dto, DiagnosticsSnapshot};
use kivori_model::Capabilities;

fn keys(value: &serde_json::Value, prefix: &str, out: &mut BTreeSet<String>) {
    if let serde_json::Value::Object(map) = value {
        for (key, child) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            out.insert(path.clone());
            keys(child, &path, out);
        }
    }
}

fn connected_snapshot(now: Instant) -> DiagnosticsSnapshot {
    let mut snapshot = DiagnosticsSnapshot::initial();
    snapshot.connection = "connected";
    snapshot.firmware_version = Some("0.4.0".to_string());
    snapshot.device_hash = Some("a1b2c3d4".to_string());
    snapshot.negotiated_caps = Capabilities::MASCOT_INTERACTION.union(Capabilities::DESK_STATUS_V1);
    snapshot.negotiated_minor = Some(4);
    snapshot.connected_since = Some(now - Duration::from_secs(90));
    snapshot.session = SessionDiagnostics {
        free_bytes: Some(200_000),
        last_pong: Some((now - Duration::from_millis(400), 10_000)),
        rtt_ms: Some(7),
        malformed_frames: 1,
        sequence_gaps: 2,
        device_errors: 3,
    };
    snapshot
}

#[test]
fn the_serialised_keys_are_exactly_the_allowlist() {
    let now = Instant::now();
    let dto = diagnostics_dto(&connected_snapshot(now), &ConfigStore::detached(), now);
    let value = serde_json::to_value(&dto).unwrap();
    let mut actual = BTreeSet::new();
    keys(&value, "", &mut actual);
    let expected: BTreeSet<String> = [
        "versions",
        "versions.app",
        "versions.firmware",
        "versions.protocol",
        "versions.negotiatedMinor",
        "versions.capabilities",
        "versions.deviceHash",
        "connection",
        "connection.state",
        "connection.connectedForSecs",
        "connection.reconnects",
        "connection.retryCount",
        "connection.lastPongAgeMs",
        "connection.rttMs",
        "health",
        "health.deviceUptimeMs",
        "health.freeBytes",
        "health.malformedFrames",
        "health.sequenceGaps",
        "health.deviceErrors",
        "host",
        "host.systemVolume",
        "host.appVolume",
        "host.media",
        "host.focus",
        "host.inputPermission",
        "config",
        "config.status",
        "config.schemaVersion",
        "config.customBindings",
        "config.macros",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(actual, expected);
    for key in &actual {
        let lower = key.to_lowercase();
        assert!(
            !lower.contains("port") && !lower.contains("path") && !lower.contains("deviceid"),
            "{key} could identify a machine or device"
        );
    }
}

#[test]
fn a_disconnected_snapshot_reports_unknowns_as_null() {
    let now = Instant::now();
    let dto = diagnostics_dto(
        &DiagnosticsSnapshot::initial(),
        &ConfigStore::detached(),
        now,
    );
    let value = serde_json::to_value(&dto).unwrap();
    assert_eq!(value["versions"]["firmware"], serde_json::Value::Null);
    assert_eq!(value["health"]["freeBytes"], serde_json::Value::Null);
    assert_eq!(value["health"]["deviceUptimeMs"], serde_json::Value::Null);
    assert_eq!(value["connection"]["rttMs"], serde_json::Value::Null);
    assert_eq!(value["connection"]["state"], "disconnected");
}

#[test]
fn ages_and_uptime_are_worked_out_when_asked() {
    let now = Instant::now();
    let dto = diagnostics_dto(&connected_snapshot(now), &ConfigStore::detached(), now);
    assert_eq!(dto.connection.connected_for_secs, Some(90));
    assert_eq!(dto.connection.last_pong_age_ms, Some(400));
    assert_eq!(dto.health.device_uptime_ms, Some(10_400));
    assert_eq!(
        dto.versions.capabilities,
        vec!["mascotInteraction", "deskStatusV1"]
    );
}

#[test]
fn capability_names_follow_bit_order_and_skip_unset_bits() {
    assert!(capability_names(Capabilities::NONE).is_empty());
    assert_eq!(
        capability_names(Capabilities::CONTEXT_BUTTONS_V1.union(Capabilities::PHYSICAL_INPUT_V1)),
        vec!["physicalInputV1", "contextButtonsV1"]
    );
    assert_eq!(
        capability_names(Capabilities::HOST_TAKEOVERS_V1),
        vec!["hostTakeoversV1"]
    );
}
