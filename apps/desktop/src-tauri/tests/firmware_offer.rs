//! Firmware update offer decisions (ADR-0015, step 6; product.md, "Firmware updates (beta)").
//! Pure: no clock, no device, nothing flashed.

use std::time::Duration;

use kivori_desktop::firmware_marker::ImageIdentity;
use kivori_desktop::firmware_offer::{
    decide, Decision, Evaluation, FirmwareOffer, OfferDevice, OfferInputs, OfferKey, OfferTracker,
    Refused, RequiredReason, IDLE_BEFORE_OFFER,
};
use kivori_desktop::identity::{
    missing_required_capabilities, DeviceIdentity, IdentitySource, CAPABILITY_NAMES,
};
use kivori_desktop::ipc::dto::firmware_offer_dto;
use kivori_model::Capabilities;

/// The protocol major this app speaks.
const HOST_MAJOR: u16 = kivori_protocol::PROTOCOL_MAJOR;
/// The major after it. No major is older than 0, so the "device on an older major" rule is checked
/// the way an app on the next major will meet a device still on this one.
const NEXT_MAJOR: u16 = HOST_MAJOR + 1;

fn image(version: &str) -> ImageIdentity {
    ImageIdentity {
        product: 1,
        board: 1,
        version: version.into(),
        min_protocol: (0, 1),
    }
}

fn identity(product: u16, board: u16) -> DeviceIdentity {
    DeviceIdentity {
        product,
        board,
        hw_rev: None,
        source: IdentitySource::Build,
    }
}

fn connected(firmware: &str) -> OfferDevice {
    OfferDevice::Connected {
        hash: "abcd1234".into(),
        firmware: firmware.into(),
        identity: Some(DeviceIdentity::legacy()),
        missing_capabilities: false,
    }
}

fn inputs<'a>(device: &'a OfferDevice, image: Option<&'a ImageIdentity>) -> OfferInputs<'a> {
    OfferInputs {
        device,
        generation: 1,
        image,
        host_major: HOST_MAJOR,
        app_update_due: false,
        flashing: false,
        gesture: false,
    }
}

/// A device on [`HOST_MAJOR`], seen by an app that speaks [`NEXT_MAJOR`]: its major is older.
fn older_major() -> OfferDevice {
    OfferDevice::Incompatible {
        device_major: Some(HOST_MAJOR),
    }
}

/// [`inputs`] for an app on [`NEXT_MAJOR`].
fn next_major_inputs<'a>(
    device: &'a OfferDevice,
    image: Option<&'a ImageIdentity>,
) -> OfferInputs<'a> {
    OfferInputs {
        host_major: NEXT_MAJOR,
        ..inputs(device, image)
    }
}

fn optional(from: &str, to: &str) -> FirmwareOffer {
    FirmwareOffer::OfferOptional {
        from: from.into(),
        to: to.into(),
    }
}

fn key(generation: u32) -> OfferKey {
    OfferKey {
        hash: Some("abcd1234".into()),
        version: "0.2.0".into(),
        generation,
    }
}

/// The rest of a row: an app update due, a flash running, the idle wait passed.
#[derive(Clone, Copy)]
struct When {
    due: bool,
    flash: bool,
    idle: bool,
}

const IDLE: When = When {
    due: false,
    flash: false,
    idle: true,
};
const WAITING: When = When {
    idle: false,
    ..IDLE
};
const DUE: When = When { due: true, ..IDLE };
const DUE_WAITING: When = When { idle: false, ..DUE };
const FLASHING: When = When {
    flash: true,
    ..IDLE
};

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn the_tokens_list_every_kind_and_reason() {
    let decisions = [
        FirmwareOffer::None,
        optional("0.1.0", "0.2.0"),
        FirmwareOffer::Required {
            reason: RequiredReason::ProtocolTooOld,
        },
        FirmwareOffer::UpdateApp,
        FirmwareOffer::DeferredForAppUpdate,
    ];
    assert_eq!(decisions.map(|d| d.token()), FirmwareOffer::TOKENS);
    assert_eq!(
        RequiredReason::ALL.map(RequiredReason::token),
        ["protocolTooOld", "missingCapabilities"]
    );
}

#[test]
fn the_decision_table() {
    let bundled = image("0.2.0");
    let foreign = OfferDevice::Connected {
        hash: "abcd1234".into(),
        firmware: "0.1.0".into(),
        identity: Some(identity(2, 1)),
        missing_capabilities: false,
    };
    let other_board = OfferDevice::Connected {
        hash: "abcd1234".into(),
        firmware: "0.1.0".into(),
        identity: Some(identity(1, 9)),
        missing_capabilities: false,
    };
    let pending = OfferDevice::Connected {
        hash: "abcd1234".into(),
        firmware: "0.1.0".into(),
        identity: None,
        missing_capabilities: false,
    };
    let lacking = OfferDevice::Connected {
        hash: "abcd1234".into(),
        firmware: "0.1.0".into(),
        identity: Some(identity(1, 1)),
        missing_capabilities: true,
    };
    let older = connected("0.1.0");
    let same = connected("0.2.0");
    let newer = connected("0.3.0");
    let garbled = connected("1.x");
    let old_major = older_major();
    // A board flashed before the version reset speaks major 1: newer than this app's 0.
    let new_major = OfferDevice::Incompatible {
        device_major: Some(NEXT_MAJOR),
    };
    let unknown_major = OfferDevice::Incompatible { device_major: None };
    let required = |reason| FirmwareOffer::Required { reason };

    let b = Some(&bundled);
    // (case, device, image, when, expected)
    let table: Vec<(
        &str,
        &OfferDevice,
        Option<&ImageIdentity>,
        When,
        FirmwareOffer,
    )> = vec![
        (
            "nothing connected",
            &OfferDevice::Absent,
            b,
            IDLE,
            FirmwareOffer::None,
        ),
        (
            "older firmware",
            &older,
            b,
            IDLE,
            optional("0.1.0", "0.2.0"),
        ),
        (
            "older, not idle long enough",
            &older,
            b,
            WAITING,
            FirmwareOffer::None,
        ),
        (
            "older, app update due",
            &older,
            b,
            DUE,
            FirmwareOffer::DeferredForAppUpdate,
        ),
        (
            "older, due, still waiting",
            &older,
            b,
            DUE_WAITING,
            FirmwareOffer::DeferredForAppUpdate,
        ),
        (
            "older, flash running",
            &older,
            b,
            FLASHING,
            FirmwareOffer::None,
        ),
        ("up to date", &same, b, IDLE, FirmwareOffer::None),
        ("device newer", &newer, b, IDLE, FirmwareOffer::UpdateApp),
        (
            "device newer, not idle yet",
            &newer,
            b,
            WAITING,
            FirmwareOffer::UpdateApp,
        ),
        (
            "device newer, app update due",
            &newer,
            b,
            DUE,
            FirmwareOffer::UpdateApp,
        ),
        (
            "unparseable version",
            &garbled,
            b,
            IDLE,
            FirmwareOffer::None,
        ),
        ("another product", &foreign, b, IDLE, FirmwareOffer::None),
        ("another board", &other_board, b, IDLE, FirmwareOffer::None),
        (
            "identity not reported yet",
            &pending,
            b,
            IDLE,
            FirmwareOffer::None,
        ),
        (
            "no firmware in this build",
            &older,
            None,
            IDLE,
            FirmwareOffer::None,
        ),
        (
            "lacks a capability",
            &lacking,
            b,
            IDLE,
            required(RequiredReason::MissingCapabilities),
        ),
        (
            "lacks a capability, due",
            &lacking,
            b,
            DUE,
            FirmwareOffer::DeferredForAppUpdate,
        ),
        (
            "older major",
            &old_major,
            b,
            IDLE,
            required(RequiredReason::ProtocolTooOld),
        ),
        (
            "unknown major",
            &unknown_major,
            b,
            IDLE,
            required(RequiredReason::ProtocolTooOld),
        ),
        (
            "older major, waiting",
            &old_major,
            b,
            WAITING,
            FirmwareOffer::None,
        ),
        (
            "older major, due",
            &old_major,
            b,
            DUE,
            FirmwareOffer::DeferredForAppUpdate,
        ),
        (
            "older major, no firmware",
            &old_major,
            None,
            IDLE,
            FirmwareOffer::None,
        ),
        ("newer major", &new_major, b, IDLE, FirmwareOffer::UpdateApp),
        (
            "newer major, flash running",
            &new_major,
            b,
            FLASHING,
            FirmwareOffer::None,
        ),
    ];
    for (case, device, image, when, expected) in table {
        let base = if *device == old_major {
            next_major_inputs(device, image)
        } else {
            inputs(device, image)
        };
        let inputs = OfferInputs {
            app_update_due: when.due,
            flashing: when.flash,
            ..base
        };
        assert_eq!(decide(&inputs, when.idle, None).offer, expected, "{case}");
    }
}

#[test]
fn a_board_flashed_before_the_reset_is_told_to_update_the_app_and_never_offered_a_flash() {
    // Boards flashed before the version reset speak protocol major 1; this app speaks 0. The
    // general rule (newer major: update the app) holds, and nothing is offered automatically: the
    // developer reflashes such a board with `just fw-flash` or the manual Flash firmware button.
    assert_eq!(HOST_MAJOR, 0);
    let bundled = image("0.1.0");
    let pre_reset = OfferDevice::Incompatible {
        device_major: Some(1),
    };
    for (settled, due) in [(false, false), (true, false), (true, true)] {
        let inputs = OfferInputs {
            app_update_due: due,
            ..inputs(&pre_reset, Some(&bundled))
        };
        let decision = decide(&inputs, settled, None);
        assert_eq!(decision.offer, FirmwareOffer::UpdateApp);
        assert_eq!(decision.key, None);
        assert!(!decision.offer.asks());
    }
}

#[test]
fn an_offer_is_keyed_by_device_bundled_version_and_connection() {
    let bundled = image("0.2.0");
    let older = connected("0.1.0");
    let decision = decide(&inputs(&older, Some(&bundled)), true, None);
    assert_eq!(decision.key, Some(key(1)));
    assert_eq!(decision.key.unwrap().token(), "1:abcd1234:0.2.0");
    // An incompatible device has no hash yet.
    let old_major = older_major();
    let required = decide(&next_major_inputs(&old_major, Some(&bundled)), true, None);
    assert_eq!(required.key.unwrap().token(), "1::0.2.0");
    // Nothing that is not a question carries a key.
    let newer = connected("0.3.0");
    assert_eq!(
        decide(&inputs(&newer, Some(&bundled)), true, None).key,
        None
    );
}

#[test]
fn later_hides_only_the_same_device_version_and_connection() {
    let bundled = image("0.2.0");
    let older = connected("0.1.0");
    let at = |generation| OfferInputs {
        generation,
        ..inputs(&older, Some(&bundled))
    };
    assert_eq!(
        decide(&at(1), true, Some(&key(1))).offer,
        FirmwareOffer::None
    );
    // The next connect asks again.
    assert_eq!(
        decide(&at(2), true, Some(&key(1))).offer,
        optional("0.1.0", "0.2.0")
    );
    // A newer bundled version is a different offer.
    let newer_bundle = image("0.3.0");
    let other_version = OfferInputs {
        image: Some(&newer_bundle),
        ..at(1)
    };
    assert_eq!(
        decide(&other_version, true, Some(&key(1))).offer,
        optional("0.1.0", "0.3.0")
    );
    // Another device is a different offer.
    let other_device = OfferDevice::Connected {
        hash: "ffff0000".into(),
        firmware: "0.1.0".into(),
        identity: Some(DeviceIdentity::legacy()),
        missing_capabilities: false,
    };
    assert_eq!(
        decide(&inputs(&other_device, Some(&bundled)), true, Some(&key(1))).offer,
        optional("0.1.0", "0.2.0")
    );
}

#[test]
fn the_offer_waits_for_five_idle_seconds_after_connecting() {
    let bundled = image("0.2.0");
    let older = connected("0.1.0");
    let mut tracker = OfferTracker::new();
    let idle = inputs(&older, Some(&bundled));
    assert_eq!(IDLE_BEFORE_OFFER, secs(5));
    assert_eq!(tracker.evaluate(secs(10), &idle), Evaluation::default());
    assert_eq!(tracker.evaluate(secs(14), &idle), Evaluation::default());
    assert_eq!(
        tracker.evaluate(secs(15), &idle),
        Evaluation {
            changed: true,
            shown: true
        }
    );
    assert_eq!(tracker.current().offer, optional("0.1.0", "0.2.0"));
    // Nothing changes while it stays.
    assert_eq!(tracker.evaluate(secs(16), &idle), Evaluation::default());
}

#[test]
fn a_gesture_or_flash_restarts_the_idle_wait_but_does_not_withdraw_a_shown_offer() {
    let bundled = image("0.2.0");
    let older = connected("0.1.0");
    let mut tracker = OfferTracker::new();
    let idle = inputs(&older, Some(&bundled));
    let turning = OfferInputs {
        gesture: true,
        ..idle
    };
    tracker.evaluate(secs(0), &idle);
    tracker.evaluate(secs(4), &turning);
    // The wait starts again when the gesture ends.
    tracker.evaluate(secs(5), &idle);
    tracker.evaluate(secs(9), &idle);
    assert_eq!(tracker.current().offer, FirmwareOffer::None);
    tracker.evaluate(secs(10), &idle);
    assert_eq!(tracker.current().offer, optional("0.1.0", "0.2.0"));
    // Once shown, turning the knob leaves it.
    tracker.evaluate(secs(11), &turning);
    assert_eq!(tracker.current().offer, optional("0.1.0", "0.2.0"));
    // A flash owns the device: no offer while it runs.
    let flashing = OfferInputs {
        flashing: true,
        ..idle
    };
    assert!(tracker.evaluate(secs(12), &flashing).changed);
    assert_eq!(tracker.current().offer, FirmwareOffer::None);
}

#[test]
fn a_new_connection_waits_again_and_unplugging_forgets_the_wait() {
    let bundled = image("0.2.0");
    let older = connected("0.1.0");
    let mut tracker = OfferTracker::new();
    let first = inputs(&older, Some(&bundled));
    tracker.evaluate(secs(0), &first);
    tracker.evaluate(secs(5), &first);
    assert!(tracker.current().offer.asks());
    tracker.evaluate(secs(6), &inputs(&OfferDevice::Absent, Some(&bundled)));
    assert_eq!(tracker.current().offer, FirmwareOffer::None);
    let second = OfferInputs {
        generation: 2,
        ..first
    };
    tracker.evaluate(secs(7), &second);
    assert_eq!(tracker.current().offer, FirmwareOffer::None);
    tracker.evaluate(secs(12), &second);
    assert_eq!(tracker.current().offer, optional("0.1.0", "0.2.0"));
}

#[test]
fn later_suppresses_until_the_next_connect_and_is_never_permanent() {
    let bundled = image("0.2.0");
    let older = connected("0.1.0");
    let mut tracker = OfferTracker::new();
    let first = inputs(&older, Some(&bundled));
    tracker.evaluate(secs(0), &first);
    tracker.evaluate(secs(5), &first);
    assert_eq!(tracker.later(), Ok(()));
    assert_eq!(tracker.current().offer, FirmwareOffer::None);
    // Not again in this connection.
    for t in 6..60 {
        assert_eq!(tracker.evaluate(secs(t), &first), Evaluation::default());
    }
    // The next connect asks again, after its own idle wait, and counts as a new "shown".
    let second = OfferInputs {
        generation: 2,
        ..first
    };
    tracker.evaluate(secs(100), &second);
    assert_eq!(
        tracker.evaluate(secs(105), &second),
        Evaluation {
            changed: true,
            shown: true
        }
    );
    assert_eq!(tracker.current().offer, optional("0.1.0", "0.2.0"));
}

#[test]
fn later_and_accept_need_an_offer_and_a_required_update_has_no_later() {
    let bundled = image("0.2.0");
    let mut tracker = OfferTracker::new();
    assert_eq!(tracker.later(), Err(Refused::NothingOffered));
    assert_eq!(tracker.accept(), Err(Refused::NothingOffered));
    let old_major = older_major();
    let incompatible = next_major_inputs(&old_major, Some(&bundled));
    tracker.evaluate(secs(0), &incompatible);
    tracker.evaluate(secs(5), &incompatible);
    assert_eq!(
        tracker.current().offer,
        FirmwareOffer::Required {
            reason: RequiredReason::ProtocolTooOld
        }
    );
    assert_eq!(tracker.accept(), Ok(()));
    // "Not now" only closes the dialog: there is nothing to remember.
    assert_eq!(tracker.later(), Err(Refused::NothingOffered));
    assert!(tracker.current().offer.asks());
    assert_eq!(
        Refused::NothingOffered.message(),
        "There is no firmware update to install."
    );
}

#[test]
fn the_app_update_goes_first_and_the_offer_follows_when_it_is_answered() {
    let bundled = image("0.2.0");
    let older = connected("0.1.0");
    let mut tracker = OfferTracker::new();
    let due = OfferInputs {
        app_update_due: true,
        ..inputs(&older, Some(&bundled))
    };
    tracker.evaluate(secs(0), &due);
    tracker.evaluate(secs(10), &due);
    assert_eq!(tracker.current().offer, FirmwareOffer::DeferredForAppUpdate);
    // "Later" on the app update: the firmware offer follows on the next tick.
    let answered = OfferInputs {
        app_update_due: false,
        ..due
    };
    assert_eq!(
        tracker.evaluate(secs(11), &answered),
        Evaluation {
            changed: true,
            shown: true
        }
    );
    assert_eq!(tracker.current().offer, optional("0.1.0", "0.2.0"));
    // An app update found while the offer is open takes over; the same offer comes back after
    // without being recorded twice.
    assert!(tracker.evaluate(secs(12), &due).changed);
    assert_eq!(tracker.current().offer, FirmwareOffer::DeferredForAppUpdate);
    assert_eq!(
        tracker.evaluate(secs(13), &answered),
        Evaluation {
            changed: true,
            shown: false
        }
    );
}

#[test]
fn the_dto_carries_versions_and_names_only_for_a_question() {
    let decision = Decision {
        offer: optional("0.1.0", "0.2.0"),
        key: Some(key(3)),
    };
    let dto = firmware_offer_dto(&decision, Some("Kivori Pip".into()));
    assert_eq!(dto.kind, "offerOptional");
    assert_eq!(dto.product.as_deref(), Some("Kivori Pip"));
    assert_eq!(
        (dto.from.as_deref(), dto.to.as_deref()),
        (Some("0.1.0"), Some("0.2.0"))
    );
    assert_eq!(dto.reason, None);
    assert_eq!(dto.key.as_deref(), Some("3:abcd1234:0.2.0"));

    let required = Decision {
        offer: FirmwareOffer::Required {
            reason: RequiredReason::ProtocolTooOld,
        },
        key: Some(OfferKey {
            hash: None,
            version: "0.2.0".into(),
            generation: 4,
        }),
    };
    let dto = firmware_offer_dto(&required, None);
    assert_eq!(
        (dto.kind, dto.reason, dto.from, dto.to.as_deref()),
        ("required", Some("protocolTooOld"), None, Some("0.2.0"))
    );

    let notice = Decision {
        offer: FirmwareOffer::UpdateApp,
        key: None,
    };
    let dto = firmware_offer_dto(&notice, Some("Kivori Pip".into()));
    assert_eq!(
        (dto.kind, dto.product.as_deref(), dto.key.as_deref()),
        ("updateApp", None, None)
    );
    let json = serde_json::to_value(&dto).unwrap();
    let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["from", "key", "kind", "product", "reason", "to"]);
}

#[test]
fn a_device_lacking_a_required_capability_is_detected_from_the_catalog() {
    let all = CAPABILITY_NAMES
        .iter()
        .fold(Capabilities::NONE, |acc, (_, flag)| acc.union(*flag));
    let legacy = DeviceIdentity::legacy();
    assert!(!missing_required_capabilities(&legacy, all));
    assert!(missing_required_capabilities(&legacy, Capabilities::NONE));
    // A product the catalog does not know requires nothing (and is never offered firmware).
    assert!(!missing_required_capabilities(
        &identity(9, 1),
        Capabilities::NONE
    ));
}
