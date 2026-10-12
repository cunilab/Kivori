//! Tray projection table (M3 S4). Pure: no tray, no window.

use kivori_desktop::firmware::UpdateAdvice;
use kivori_desktop::ipc::dto::{
    initial_status, ConnectionStatusDto, DeviceInfoDto, ProtocolVersionDto,
};
use kivori_desktop::runtime::lifecycle::tray_model;

fn status(connection: &str, host: &'static str, firmware: Option<&str>) -> ConnectionStatusDto {
    ConnectionStatusDto {
        connection: connection.to_string(),
        host,
        device: firmware.map(|version| DeviceInfoDto {
            firmware_version: version.to_string(),
            protocol_version: ProtocolVersionDto { major: 1, minor: 5 },
            device_id_hash_short: "deadbeef".to_string(),
            identity: None,
        }),
        ..initial_status()
    }
}

#[test]
fn status_line_table() {
    let cases = [
        ("disconnected", "active", None, "Not connected"),
        ("connecting", "active", None, "Not connected"),
        ("incompatible", "active", Some("0.9.0"), "Not connected"),
        ("connected", "active", Some("1.3.0"), "Connected"),
        ("connected", "locked", Some("1.3.0"), "Paused while locked"),
        (
            "connected",
            "sleeping",
            Some("1.3.0"),
            "Paused while locked",
        ),
        ("disconnected", "locked", None, "Paused while locked"),
    ];
    for (connection, host, firmware, expected) in cases {
        let model = tray_model(
            &status(connection, host, firmware),
            UpdateAdvice::Unknown,
            None,
            false,
        );
        assert_eq!(model.status, expected, "{connection}/{host}");
    }
}

#[test]
fn tooltip_names_desktop_and_firmware_versions() {
    let desktop = env!("CARGO_PKG_VERSION");
    let connected = tray_model(
        &status("connected", "active", Some("1.3.0")),
        UpdateAdvice::UpToDate,
        None,
        false,
    );
    assert_eq!(
        connected.tooltip,
        format!("Kivori \u{b7} Desktop {desktop} \u{b7} Firmware 1.3.0")
    );
    let absent = tray_model(
        &status("disconnected", "active", None),
        UpdateAdvice::Unknown,
        None,
        false,
    );
    assert_eq!(absent.tooltip, format!("Kivori \u{b7} Desktop {desktop}"));
}

#[test]
fn tooltip_flags_an_available_update_only() {
    for advice in UpdateAdvice::ALL {
        let model = tray_model(
            &status("connected", "active", Some("1.2.0")),
            advice,
            None,
            false,
        );
        assert_eq!(
            model.tooltip.ends_with("Update available"),
            advice == UpdateAdvice::UpdateAvailable,
            "{advice:?}"
        );
    }
}

#[test]
fn an_offered_app_update_adds_the_menu_item_and_tooltip() {
    let desktop = env!("CARGO_PKG_VERSION");
    let none = tray_model(
        &status("connected", "active", Some("1.3.0")),
        UpdateAdvice::UpToDate,
        None,
        false,
    );
    assert_eq!(none.app_update, None);
    let offered = tray_model(
        &status("connected", "active", Some("1.3.0")),
        UpdateAdvice::UpToDate,
        Some("0.4.0"),
        false,
    );
    assert_eq!(
        offered.app_update.as_deref(),
        Some("Update to v0.4.0\u{2026}")
    );
    assert_eq!(
        offered.tooltip,
        format!(
            "Kivori \u{b7} Desktop {desktop} \u{b7} Firmware 1.3.0 \u{b7} Kivori 0.4.0 is ready"
        )
    );
    // The app update is offered whether or not a device is connected.
    let disconnected = tray_model(
        &status("disconnected", "active", None),
        UpdateAdvice::Unknown,
        Some("0.4.0"),
        false,
    );
    assert_eq!(disconnected.status, "Not connected");
    assert!(disconnected.app_update.is_some());
}

#[test]
fn a_firmware_offer_adds_its_item_only_while_no_app_update_is_offered() {
    let connected = status("connected", "active", Some("1.2.0"));
    let none = tray_model(&connected, UpdateAdvice::UpdateAvailable, None, false);
    assert_eq!(none.firmware_update, None);
    let offered = tray_model(&connected, UpdateAdvice::UpdateAvailable, None, true);
    assert_eq!(
        offered.firmware_update,
        Some("Update Kivori firmware\u{2026}")
    );
    assert_eq!(offered.app_update, None);
    // App first: with both, only the app update's item shows.
    let both = tray_model(
        &connected,
        UpdateAdvice::UpdateAvailable,
        Some("0.4.0"),
        true,
    );
    assert_eq!(both.firmware_update, None);
    assert_eq!(both.app_update.as_deref(), Some("Update to v0.4.0\u{2026}"));
}
