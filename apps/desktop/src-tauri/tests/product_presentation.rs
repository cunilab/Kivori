//! Catalog presentation is current-session metadata, not capability authorization or diagnostics.
use kivori_desktop::catalog::Catalog;
use kivori_desktop::identity::{DeviceIdentity, IdentitySource};
use kivori_desktop::ipc::dto::{
    initial_status, product_presentation_dto, ConnectionStatusDto, DeviceInfoDto,
    ProtocolVersionDto,
};

fn status() -> ConnectionStatusDto {
    ConnectionStatusDto {
        connection: "connected".into(),
        device: Some(DeviceInfoDto {
            firmware_version: "0.1.0".into(),
            protocol_version: ProtocolVersionDto { major: 0, minor: 1 },
            device_id_hash_short: "test".into(),
            identity: None,
            presentation: None,
        }),
        ..initial_status()
    }
}

#[test]
fn fake_product_presentation_comes_from_its_own_catalog_metadata() {
    let catalog = Catalog::parse(
        "[products]\nfixture = 42\n[boards]\ntest-board = 9\n",
        &[("fixture", "display_name = 'Test dial'\nshort_name = 'Dial'\nmodel = 'KV-T2'\nimage = 'unit.png'\nrequired_capabilities = []\n[boards.test-board]\ndisplay_name = 'Test board'\nflash = 'espflash'\nusb = []\n[boards.test-board.firmware]\ncrate = 'test-firmware'\nartifact = 'test-image'\nmin_protocol = '0.1'\n")],
    ).unwrap();
    let product = catalog.product("fixture").unwrap();
    let mut without_art = product.clone();
    without_art.image = None;
    assert!(product_presentation_dto(&without_art).art.is_none());
    let dto = product_presentation_dto(product);
    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        serde_json::json!({"displayName": "Test dial", "model": "KV-T2", "art": "fixture"})
    );
}

#[test]
fn known_and_legacy_identities_get_presentation_without_authorizing_features() {
    for source in [
        IdentitySource::Legacy,
        IdentitySource::Build,
        IdentitySource::Chip,
    ] {
        let identity = DeviceIdentity {
            source,
            ..DeviceIdentity::legacy()
        };
        let dto = status().with_identity(Some(identity));
        let presentation = dto.device.unwrap().presentation.unwrap();
        assert_eq!(presentation.display_name, "Kivori Pip");
        assert_eq!(presentation.model, "KV-P1");
        assert!(dto.capabilities.is_empty());
    }
}

#[test]
fn pending_unknown_ended_and_replaced_identities_never_keep_presentation() {
    let known = DeviceIdentity::legacy();
    for identity in [
        None,
        Some(DeviceIdentity {
            product: 9,
            ..known
        }),
        Some(DeviceIdentity { board: 9, ..known }),
    ] {
        let dto = status().with_identity(Some(known)).with_identity(identity);
        assert!(dto.device.unwrap().presentation.is_none());
    }
    for state in ["disconnected", "connecting", "incompatible", "error"] {
        let mut ended = status();
        ended.connection = state.into();
        assert!(ended
            .with_identity(Some(known))
            .device
            .unwrap()
            .presentation
            .is_none());
    }
}
