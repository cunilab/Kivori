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
        let model = tray_model(&status(connection, host, firmware), UpdateAdvice::Unknown);
        assert_eq!(model.status, expected, "{connection}/{host}");
    }
}

#[test]
fn tooltip_names_desktop_and_firmware_versions() {
    let desktop = env!("CARGO_PKG_VERSION");
    let connected = tray_model(
        &status("connected", "active", Some("1.3.0")),
        UpdateAdvice::UpToDate,
    );
    assert_eq!(
        connected.tooltip,
        format!("Kivori \u{b7} Desktop {desktop} \u{b7} Firmware 1.3.0")
    );
    let absent = tray_model(
        &status("disconnected", "active", None),
        UpdateAdvice::Unknown,
    );
    assert_eq!(absent.tooltip, format!("Kivori \u{b7} Desktop {desktop}"));
}

#[test]
fn tooltip_flags_an_available_update_only() {
    for advice in UpdateAdvice::ALL {
        let model = tray_model(&status("connected", "active", Some("1.2.0")), advice);
        assert_eq!(
            model.tooltip.ends_with("Update available"),
            advice == UpdateAdvice::UpdateAvailable,
            "{advice:?}"
        );
    }
}
