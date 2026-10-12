//! App update decisions (ADR-0015; product.md, "App updates"). Pure: no network, no clock, no Tauri.

use std::time::Duration;

use kivori_desktop::config::{UpdateSettings, UpdateSnooze};
use kivori_desktop::updater::policy::{
    availability, decide, snooze, snooze_remaining, Availability, Busy, CheckOutcome, CheckReport,
    DisabledReason, Machine, Offer, Refused, Release, Schedule, UpdateState, CHECK_INTERVAL,
    ENDPOINT, FIRST_CHECK_DELAY, MAX_NOTES, SNOOZE_SECS,
};
use semver::Version;
use serde_json::json;

const NOW: u64 = 1_800_000_000;
const IDLE: Busy = Busy {
    flash: false,
    gesture: false,
};

fn v(text: &str) -> Version {
    Version::parse(text).unwrap()
}

fn release(version: &str) -> Release {
    Release::new(v(version), Some("Fixes."), Some("2026-10-12T00:00:00Z"))
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

// --- availability -----------------------------------------------------------------------------

#[test]
fn only_a_windows_release_config_with_the_endpoint_and_a_key_is_enabled() {
    let good = json!({ "endpoints": [ENDPOINT], "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ=" });
    assert_eq!(availability(Some(&good), true), Availability::Enabled);
    // macOS (and anything not Windows): nothing is checked, whatever the config says.
    assert_eq!(availability(Some(&good), false), Availability::Unsupported);
    assert_eq!(availability(None, false), Availability::Unsupported);
    // The development config has no updater section at all.
    assert_eq!(
        availability(None, true),
        Availability::Disabled(DisabledReason::DevBuild)
    );
    assert_eq!(
        availability(Some(&serde_json::Value::Null), true),
        Availability::Disabled(DisabledReason::DevBuild)
    );
}

#[test]
fn an_empty_or_missing_public_key_disables_the_updater() {
    for pubkey in [json!(""), json!("   "), json!(null)] {
        let section = json!({ "endpoints": [ENDPOINT], "pubkey": pubkey });
        assert_eq!(
            availability(Some(&section), true),
            Availability::Disabled(DisabledReason::NoKey),
            "{pubkey}"
        );
    }
    let no_field = json!({ "endpoints": [ENDPOINT] });
    assert_eq!(
        availability(Some(&no_field), true),
        Availability::Disabled(DisabledReason::NoKey)
    );
}

#[test]
fn any_endpoint_other_than_the_release_one_disables_the_updater() {
    let key = "dW50cnVzdGVkIGNvbW1lbnQ=";
    for endpoints in [
        json!([]),
        json!(["https://example.com/latest.json"]),
        json!([ENDPOINT, "https://example.com/latest.json"]),
        json!("not a list"),
    ] {
        let section = json!({ "endpoints": endpoints, "pubkey": key });
        assert_eq!(
            availability(Some(&section), true),
            Availability::Disabled(DisabledReason::BadEndpoint),
            "{endpoints}"
        );
    }
}

#[test]
fn the_release_config_names_the_release_endpoint_and_a_key_field() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.release.conf.json");
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("release config")).unwrap();
    let updater = &config["plugins"]["updater"];
    assert_eq!(updater["endpoints"], json!([ENDPOINT]));
    assert!(updater["pubkey"].is_string(), "pubkey field present");
    assert_eq!(updater["windows"]["installMode"], "passive");
    // Our own availability rule agrees: enabled exactly when the key is filled in.
    let expected = if updater["pubkey"].as_str().unwrap().trim().is_empty() {
        Availability::Disabled(DisabledReason::NoKey)
    } else {
        Availability::Enabled
    };
    assert_eq!(availability(Some(updater), true), expected);
}

#[test]
fn the_development_config_has_no_updater_section() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("base config")).unwrap();
    assert!(config["plugins"]["updater"].is_null());
}

// --- schedule ---------------------------------------------------------------------------------

#[test]
fn the_first_check_is_thirty_seconds_after_start_then_daily() {
    let mut schedule = Schedule::new(true);
    assert_eq!(schedule.next(), Some(FIRST_CHECK_DELAY));
    assert!(!schedule.due(Duration::ZERO), "never at startup");
    assert!(!schedule.due(secs(29)));
    assert!(schedule.due(secs(30)));
    assert_eq!(schedule.wait(secs(10)), Some(secs(20)));
    schedule.checked(secs(31));
    assert_eq!(schedule.next(), Some(secs(31) + CHECK_INTERVAL));
    assert!(!schedule.due(secs(31) + CHECK_INTERVAL - secs(1)));
    assert!(schedule.due(secs(31) + CHECK_INTERVAL));
}

#[test]
fn a_failed_check_is_retried_at_the_next_interval_not_sooner() {
    // The worker calls `checked` whatever the result.
    let mut schedule = Schedule::new(true);
    schedule.checked(secs(30));
    assert_eq!(schedule.wait(secs(31)), Some(CHECK_INTERVAL - secs(1)));
}

#[test]
fn with_the_automatic_check_off_nothing_is_scheduled() {
    let mut schedule = Schedule::new(false);
    assert_eq!(schedule.next(), None);
    assert!(!schedule.due(CHECK_INTERVAL * 10));
    assert_eq!(schedule.wait(secs(1)), None);
    // "Check now" still works and schedules nothing after it.
    schedule.checked(secs(100));
    assert_eq!(schedule.next(), None);
    // A snooze ending does not turn checks back on either.
    schedule.wake_by(secs(200));
    assert_eq!(schedule.next(), None);
}

#[test]
fn turning_the_automatic_check_on_resumes_a_day_after_the_last_check() {
    let mut schedule = Schedule::new(false);
    schedule.set_auto(true, secs(5));
    assert_eq!(
        schedule.next(),
        Some(FIRST_CHECK_DELAY),
        "never before the first-check delay"
    );
    schedule.checked(secs(100));
    schedule.set_auto(false, secs(200));
    assert_eq!(schedule.next(), None);
    schedule.set_auto(true, secs(300));
    assert_eq!(schedule.next(), Some(secs(100) + CHECK_INTERVAL));
    // Long after the last check: due now.
    let late = secs(100) + CHECK_INTERVAL * 3;
    schedule.set_auto(true, late);
    assert_eq!(schedule.next(), Some(late));
}

#[test]
fn a_snooze_ending_before_the_next_check_moves_the_check_earlier_only() {
    let mut schedule = Schedule::new(true);
    schedule.checked(secs(30));
    schedule.wake_by(secs(60));
    assert_eq!(schedule.next(), Some(secs(60)));
    schedule.wake_by(secs(30) + CHECK_INTERVAL * 5);
    assert_eq!(schedule.next(), Some(secs(60)), "never later");
}

// --- decisions --------------------------------------------------------------------------------

#[test]
fn only_a_newer_version_is_offered() {
    let settings = UpdateSettings::default();
    let running = v("0.4.2");
    let cases = [
        ("0.4.3", Offer::Offer),
        ("0.5.0", Offer::Offer),
        ("1.0.0", Offer::Offer),
        ("0.4.2", Offer::NotNewer),
        ("0.4.1", Offer::NotNewer),
        ("0.3.9", Offer::NotNewer),
        // Semver, not string order.
        ("0.10.0", Offer::Offer),
        // A pre-release of the running version is older than it.
        ("0.4.2-beta.1", Offer::NotNewer),
    ];
    for (found, expected) in cases {
        assert_eq!(
            decide(&running, &v(found), &settings, NOW, false),
            expected,
            "{found}"
        );
    }
    assert_eq!(
        decide(&v("0.9.0"), &v("0.10.0"), &settings, NOW, false),
        Offer::Offer
    );
}

#[test]
fn a_skipped_version_is_never_offered_but_a_newer_one_is() {
    let settings = UpdateSettings {
        skipped_version: Some("0.5.0".to_string()),
        ..UpdateSettings::default()
    };
    let running = v("0.4.0");
    assert_eq!(
        decide(&running, &v("0.5.0"), &settings, NOW, false),
        Offer::Skipped
    );
    assert_eq!(
        decide(&running, &v("0.5.0"), &settings, NOW, true),
        Offer::Skipped,
        "not even on Check now"
    );
    assert_eq!(
        decide(&running, &v("0.5.1"), &settings, NOW, false),
        Offer::Offer
    );
}

#[test]
fn later_hides_that_version_for_three_days() {
    let settings = UpdateSettings {
        snoozed: Some(snooze(&v("0.5.0"), NOW)),
        ..UpdateSettings::default()
    };
    let running = v("0.4.0");
    assert_eq!(
        settings.snoozed,
        Some(UpdateSnooze {
            version: "0.5.0".to_string(),
            at: NOW
        })
    );
    assert_eq!(
        decide(&running, &v("0.5.0"), &settings, NOW + 60, false),
        Offer::Snoozed
    );
    assert_eq!(
        decide(
            &running,
            &v("0.5.0"),
            &settings,
            NOW + SNOOZE_SECS - 1,
            false
        ),
        Offer::Snoozed
    );
    assert_eq!(
        decide(&running, &v("0.5.0"), &settings, NOW + SNOOZE_SECS, false),
        Offer::Offer,
        "asks again after 3 days"
    );
    // A newer version is offered straight away, and "Check now" ignores the snooze.
    assert_eq!(
        decide(&running, &v("0.5.1"), &settings, NOW + 60, false),
        Offer::Offer
    );
    assert_eq!(
        decide(&running, &v("0.5.0"), &settings, NOW + 60, true),
        Offer::Offer
    );
    assert_eq!(
        snooze_remaining(settings.snoozed.as_ref(), &v("0.5.0"), NOW + 60),
        Some(SNOOZE_SECS - 60)
    );
    assert_eq!(
        snooze_remaining(settings.snoozed.as_ref(), &v("0.5.1"), NOW),
        None
    );
}

#[test]
fn unreadable_stored_versions_never_hide_an_update() {
    let settings = UpdateSettings {
        skipped_version: Some("not a version".to_string()),
        snoozed: Some(UpdateSnooze {
            version: "v0.5".to_string(),
            at: NOW,
        }),
        ..UpdateSettings::default()
    };
    assert_eq!(
        decide(&v("0.4.0"), &v("0.5.0"), &settings, NOW, false),
        Offer::Offer
    );
}

#[test]
fn release_notes_and_dates_are_bounded() {
    let long = "é".repeat(MAX_NOTES);
    let release = Release::new(v("1.0.0"), Some(&long), Some("<script>"));
    assert!(release.notes.len() <= MAX_NOTES + '\u{2026}'.len_utf8());
    assert!(release.notes.ends_with('\u{2026}'));
    assert_eq!(release.date, None, "not an ISO time");
    let plain = Release::new(
        v("1.0.0"),
        Some("  Fixes.\n"),
        Some("2026-10-12T00:00:00.5Z"),
    );
    assert_eq!(plain.notes, "Fixes.");
    assert_eq!(plain.date.as_deref(), Some("2026-10-12T00:00:00.5Z"));
    assert_eq!(Release::new(v("1.0.0"), None, None).notes, "");
}

// --- state machine ----------------------------------------------------------------------------

fn checking() -> Machine {
    let mut machine = Machine::new(Availability::Enabled);
    assert!(machine.begin_check());
    assert_eq!(machine.state(), &UpdateState::Checking);
    machine
}

#[test]
fn the_machine_starts_in_the_state_the_build_allows() {
    assert_eq!(
        Machine::new(Availability::Enabled).state(),
        &UpdateState::Idle
    );
    assert_eq!(
        Machine::new(Availability::Unsupported).state(),
        &UpdateState::Unsupported
    );
    let mut disabled = Machine::new(Availability::Disabled(DisabledReason::NoKey));
    assert_eq!(
        disabled.state(),
        &UpdateState::Disabled(DisabledReason::NoKey)
    );
    // A disabled or unsupported updater never checks.
    assert!(!disabled.begin_check());
    assert!(!Machine::new(Availability::Unsupported).begin_check());
}

#[test]
fn a_found_newer_version_becomes_available() {
    let mut machine = checking();
    let report = machine.check_finished(
        CheckOutcome::Found(release("0.5.0")),
        &v("0.4.0"),
        &UpdateSettings::default(),
        NOW,
        false,
        IDLE,
    );
    assert_eq!(report, CheckReport::Offered);
    assert_eq!(machine.state(), &UpdateState::Available(release("0.5.0")));
    assert_eq!(machine.offered(), Some(&v("0.5.0")));
    assert_eq!(machine.last_checked(), Some(NOW));
    // An open offer is not replaced by the next scheduled check.
    assert!(!machine.begin_check());
}

#[test]
fn up_to_date_or_not_offered_goes_back_to_idle() {
    let mut machine = checking();
    assert_eq!(
        machine.check_finished(
            CheckOutcome::UpToDate,
            &v("0.4.0"),
            &UpdateSettings::default(),
            NOW,
            false,
            IDLE
        ),
        CheckReport::NothingToOffer
    );
    assert_eq!(machine.state(), &UpdateState::Idle);
    assert_eq!(machine.last_checked(), Some(NOW));

    let skipped = UpdateSettings {
        skipped_version: Some("0.5.0".to_string()),
        ..UpdateSettings::default()
    };
    assert!(machine.begin_check());
    assert_eq!(
        machine.check_finished(
            CheckOutcome::Found(release("0.5.0")),
            &v("0.4.0"),
            &skipped,
            NOW + 1,
            false,
            IDLE
        ),
        CheckReport::NothingToOffer
    );
    assert_eq!(machine.state(), &UpdateState::Idle);
    assert_eq!(machine.offered(), None);
}

#[test]
fn a_failed_check_says_nothing_and_keeps_the_last_check_time() {
    let mut machine = checking();
    machine.check_finished(
        CheckOutcome::UpToDate,
        &v("0.4.0"),
        &UpdateSettings::default(),
        NOW,
        false,
        IDLE,
    );
    assert!(machine.begin_check());
    let report = machine.check_finished(
        CheckOutcome::Failed,
        &v("0.4.0"),
        &UpdateSettings::default(),
        NOW + 100,
        true,
        IDLE,
    );
    assert_eq!(report, CheckReport::Failed);
    assert_eq!(machine.state(), &UpdateState::Idle, "no error state");
    assert_eq!(machine.last_checked(), Some(NOW));
}

#[test]
fn an_offer_waits_for_a_flash_or_a_gesture_to_end() {
    for busy in [
        Busy {
            flash: true,
            gesture: false,
        },
        Busy {
            flash: false,
            gesture: true,
        },
    ] {
        let mut machine = checking();
        let report = machine.check_finished(
            CheckOutcome::Found(release("0.5.0")),
            &v("0.4.0"),
            &UpdateSettings::default(),
            NOW,
            false,
            busy,
        );
        assert_eq!(report, CheckReport::Held);
        assert_eq!(machine.state(), &UpdateState::Idle, "{busy:?}");
        assert!(machine.holding());
        assert!(!machine.settle(busy), "still busy");
        assert!(machine.settle(IDLE));
        assert_eq!(machine.state(), &UpdateState::Available(release("0.5.0")));
        assert!(!machine.holding());
        assert!(!machine.settle(IDLE), "only once");
    }
}

#[test]
fn install_downloads_then_waits_to_restart_and_can_fail_and_retry() {
    let mut machine = checking();
    machine.check_finished(
        CheckOutcome::Found(release("0.5.0")),
        &v("0.4.0"),
        &UpdateSettings::default(),
        NOW,
        false,
        IDLE,
    );
    assert_eq!(machine.begin_install(), Ok(release("0.5.0")));
    assert_eq!(
        machine.begin_install(),
        Err(Refused::Busy),
        "one install at a time"
    );
    assert_eq!(
        machine.dismiss(),
        Err(Refused::Busy),
        "no Later mid-install"
    );
    assert_eq!(
        machine.offered(),
        None,
        "the tray item goes while installing"
    );
    machine.progress(100, None);
    assert_eq!(
        machine.state(),
        &UpdateState::Downloading {
            release: release("0.5.0"),
            downloaded: 100,
            total: None
        },
        "no size: indeterminate"
    );
    machine.progress(50, Some(1000));
    assert_eq!(
        machine.state(),
        &UpdateState::Downloading {
            release: release("0.5.0"),
            downloaded: 150,
            total: Some(1000)
        }
    );
    machine.install_failed();
    assert_eq!(machine.state(), &UpdateState::Failed(release("0.5.0")));
    assert_eq!(machine.offered(), Some(&v("0.5.0")));
    // "Try again".
    assert!(machine.begin_install().is_ok());
    machine.downloaded();
    assert_eq!(
        machine.state(),
        &UpdateState::ReadyToRestart(release("0.5.0"))
    );
    machine.install_failed();
    assert_eq!(machine.state(), &UpdateState::Failed(release("0.5.0")));
    // "Later" after a failure withdraws the offer.
    assert_eq!(machine.dismiss(), Ok(v("0.5.0")));
    assert_eq!(machine.state(), &UpdateState::Idle);
}

#[test]
fn nothing_installs_or_dismisses_without_an_offer() {
    let mut machine = Machine::new(Availability::Enabled);
    assert_eq!(machine.begin_install(), Err(Refused::NothingOffered));
    assert_eq!(machine.dismiss(), Err(Refused::NothingOffered));
    let mut disabled = Machine::new(Availability::Disabled(DisabledReason::DevBuild));
    assert_eq!(disabled.begin_install(), Err(Refused::NothingOffered));
    // Progress or completion outside a download changes nothing.
    disabled.progress(10, Some(10));
    disabled.downloaded();
    disabled.install_failed();
    assert_eq!(
        disabled.state(),
        &UpdateState::Disabled(DisabledReason::DevBuild)
    );
}

#[test]
fn busy_means_a_flash_or_a_gesture() {
    assert!(!IDLE.any());
    assert!(Busy {
        flash: true,
        gesture: false
    }
    .any());
    assert!(Busy {
        flash: false,
        gesture: true
    }
    .any());
}
