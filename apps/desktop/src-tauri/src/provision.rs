//! Pure evaluator for the per-unit QA helper (`examples/provision.rs`, M3 S7).
//!
//! DEVELOPMENT-ONLY (behind `device-studio`): the example gathers what the unit did, this module
//! decides PASS or FAIL and formats the log row. No I/O and no clock, so it is table-testable.
//! The log row carries only the short device hash and versions: never the raw device id or a port.

use kivori_model::input::Direction;
use kivori_model::Capabilities;
use kivori_protocol::{ControlId, InputKind};

/// Printed by the example. It is also the marker `scripts/check-release-surface.sh` greps for, so a
/// release build that contains this module fails the guard.
pub const BANNER: &str = "KIVORI-PROVISION per-unit QA";

/// How long a unit holds the heartbeat before the checks are read.
pub const HEARTBEAT_HOLD_SECS: u64 = 5;
/// How many detents each turn check needs.
pub const DETENTS_REQUIRED: u32 = 3;

/// Every capability a beta unit must negotiate.
pub const REQUIRED_CAPABILITIES: [(&str, Capabilities); 11] = [
    ("MASCOT_INTERACTION", Capabilities::MASCOT_INTERACTION),
    ("PHYSICAL_INPUT_V1", Capabilities::PHYSICAL_INPUT_V1),
    ("PRESENTATION_V1", Capabilities::PRESENTATION_V1),
    ("BUTTON_INPUT_V1", Capabilities::BUTTON_INPUT_V1),
    ("DESK_STATUS_V1", Capabilities::DESK_STATUS_V1),
    ("ACTION_FEEDBACK_V1", Capabilities::ACTION_FEEDBACK_V1),
    ("DOUBLE_PRESS_V1", Capabilities::DOUBLE_PRESS_V1),
    ("MEDIA_INFO_V1", Capabilities::MEDIA_INFO_V1),
    ("CONTROL_LABELS_V1", Capabilities::CONTROL_LABELS_V1),
    ("CONTEXT_BUTTONS_V1", Capabilities::CONTEXT_BUTTONS_V1),
    ("HOST_TAKEOVERS_V1", Capabilities::HOST_TAKEOVERS_V1),
];

/// One guided input the operator performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputCheck {
    /// Turn the knob clockwise [`DETENTS_REQUIRED`] detents.
    TurnRight,
    /// Turn the knob counter-clockwise [`DETENTS_REQUIRED`] detents.
    TurnLeft,
    /// A short press of the knob.
    Press,
    /// A hold of the knob.
    Hold,
    /// A short press of contextual button 0, 1 or 2 (left to right).
    ContextButton(u8),
}

impl InputCheck {
    /// The guided order.
    pub const ALL: [Self; 7] = [
        Self::TurnRight,
        Self::TurnLeft,
        Self::Press,
        Self::Hold,
        Self::ContextButton(0),
        Self::ContextButton(1),
        Self::ContextButton(2),
    ];

    /// Stable token used in the log and the failure reason.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::TurnRight => "turn_right",
            Self::TurnLeft => "turn_left",
            Self::Press => "press",
            Self::Hold => "hold",
            Self::ContextButton(0) => "button_left",
            Self::ContextButton(1) => "button_middle",
            Self::ContextButton(_) => "button_right",
        }
    }

    /// What the operator is told to do.
    #[must_use]
    pub const fn prompt(self) -> &'static str {
        match self {
            Self::TurnRight => "Turn the knob RIGHT (clockwise) 3 clicks.",
            Self::TurnLeft => "Turn the knob LEFT (counter-clockwise) 3 clicks.",
            Self::Press => "PRESS the knob once (quick tap).",
            Self::Hold => "HOLD the knob down for about one second, then release.",
            Self::ContextButton(0) => "Press the LEFT button under the screen.",
            Self::ContextButton(1) => "Press the MIDDLE button under the screen.",
            Self::ContextButton(_) => "Press the RIGHT button under the screen.",
        }
    }

    /// How many matching events complete the check.
    #[must_use]
    pub const fn required(self) -> u32 {
        match self {
            Self::TurnRight | Self::TurnLeft => DETENTS_REQUIRED,
            _ => 1,
        }
    }

    /// Whether an input event counts toward this check.
    #[must_use]
    pub fn matches(self, control: ControlId, kind: InputKind) -> bool {
        match (self, control, kind) {
            (Self::TurnRight, ControlId::Rotary, InputKind::Detent(Direction::Cw))
            | (Self::TurnLeft, ControlId::Rotary, InputKind::Detent(Direction::Ccw))
            | (Self::Press, ControlId::Button, InputKind::Press)
            | (Self::Hold, ControlId::Button, InputKind::Hold) => true,
            (Self::ContextButton(want), ControlId::ContextButton(got), InputKind::Press) => {
                want == got
            }
            _ => false,
        }
    }
}

/// Counts events toward one [`InputCheck`].
#[derive(Debug, Clone, Copy)]
pub struct InputProgress {
    check: InputCheck,
    seen: u32,
}

impl InputProgress {
    /// Starts counting for `check`.
    #[must_use]
    pub const fn new(check: InputCheck) -> Self {
        Self { check, seen: 0 }
    }

    /// Feeds one event; returns `true` once the check is complete.
    pub fn observe(&mut self, control: ControlId, kind: InputKind) -> bool {
        if self.check.matches(control, kind) {
            self.seen += 1;
        }
        self.is_done()
    }

    /// Whether enough matching events arrived.
    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.seen >= self.check.required()
    }
}

/// Everything the example observed about one unit.
#[derive(Debug, Clone)]
pub struct Observations {
    /// The flash step failed with this class token (nothing else is meaningful then).
    pub flash_failure: Option<&'static str>,
    /// The handshake completed.
    pub handshake: bool,
    /// Capabilities negotiated at the handshake.
    pub negotiated_caps: Capabilities,
    /// The unit's reported firmware version (`x.y.z`).
    pub device_version: Option<String>,
    /// The bundled firmware version read from the image's marker (`x.y.z`).
    pub bundled_version: Option<String>,
    /// The link stayed healthy for the heartbeat hold.
    pub heartbeat_ok: bool,
    /// A `StateReport` arrived.
    pub state_report_seen: bool,
    /// A `Health` arrived.
    pub health_seen: bool,
    /// The guided inputs that completed.
    pub inputs_done: Vec<InputCheck>,
}

/// One failed check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailedCheck {
    /// The firmware flash failed (class token).
    Flash(&'static str),
    /// No handshake.
    Handshake,
    /// A required capability was not negotiated.
    Capability(&'static str),
    /// The unit's firmware differs from the bundled one, or either is unknown.
    FirmwareVersion,
    /// The heartbeat did not hold.
    Heartbeat,
    /// No `StateReport` arrived.
    StateReport,
    /// No `Health` arrived.
    Health,
    /// A guided input was not seen.
    Input(InputCheck),
}

impl FailedCheck {
    /// Stable token, free of commas and quotes (safe in the CSV).
    #[must_use]
    pub fn token(&self) -> String {
        match self {
            Self::Flash(class) => format!("flash:{class}"),
            Self::Handshake => "handshake".into(),
            Self::Capability(name) => format!("capability:{name}"),
            Self::FirmwareVersion => "firmware_version".into(),
            Self::Heartbeat => "heartbeat".into(),
            Self::StateReport => "state_report".into(),
            Self::Health => "health".into(),
            Self::Input(check) => format!("input:{}", check.token()),
        }
    }
}

/// The outcome for one unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// Every failed check, in a stable order. Empty means PASS.
    pub failed: Vec<FailedCheck>,
}

impl Verdict {
    /// Whether the unit passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failed.is_empty()
    }

    /// `PASS` or `FAIL`.
    #[must_use]
    pub fn label(&self) -> &'static str {
        if self.passed() {
            "PASS"
        } else {
            "FAIL"
        }
    }

    /// Failed check tokens joined with `;` (empty for a pass).
    #[must_use]
    pub fn failed_tokens(&self) -> String {
        self.failed
            .iter()
            .map(FailedCheck::token)
            .collect::<Vec<_>>()
            .join(";")
    }
}

/// Decides PASS or FAIL. A flash or handshake failure stops there: nothing later could be read.
#[must_use]
pub fn evaluate(obs: &Observations) -> Verdict {
    let mut failed = Vec::new();
    if let Some(class) = obs.flash_failure {
        failed.push(FailedCheck::Flash(class));
        return Verdict { failed };
    }
    if !obs.handshake {
        failed.push(FailedCheck::Handshake);
        return Verdict { failed };
    }
    for (name, cap) in REQUIRED_CAPABILITIES {
        if !obs.negotiated_caps.contains(cap) {
            failed.push(FailedCheck::Capability(name));
        }
    }
    let versions_match = match (&obs.device_version, &obs.bundled_version) {
        (Some(device), Some(bundled)) => device == bundled,
        _ => false,
    };
    if !versions_match {
        failed.push(FailedCheck::FirmwareVersion);
    }
    if !obs.heartbeat_ok {
        failed.push(FailedCheck::Heartbeat);
    }
    if !obs.state_report_seen {
        failed.push(FailedCheck::StateReport);
    }
    if !obs.health_seen {
        failed.push(FailedCheck::Health);
    }
    for check in InputCheck::ALL {
        if !obs.inputs_done.contains(&check) {
            failed.push(FailedCheck::Input(check));
        }
    }
    Verdict { failed }
}

/// The CSV header line.
pub const CSV_HEADER: &str = "timestamp_utc,device_hash,device_fw,bundled_fw,result,failed_checks";

/// One log row (no trailing newline). Only the short hash and versions; every field is restricted
/// to a safe alphabet so a stray comma or newline cannot corrupt the file.
#[must_use]
pub fn csv_row(
    timestamp_utc: &str,
    device_hash: Option<&str>,
    obs: &Observations,
    verdict: &Verdict,
) -> String {
    let field = |value: Option<&str>| -> String {
        value
            .map(|v| {
                v.chars()
                    .filter(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | ';' | '_' | '-')
                    })
                    .collect::<String>()
            })
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "unknown".into())
    };
    format!(
        "{},{},{},{},{},{}",
        field(Some(timestamp_utc)),
        field(device_hash),
        field(obs.device_version.as_deref()),
        field(obs.bundled_version.as_deref()),
        verdict.label(),
        verdict.failed_tokens(),
    )
}

/// `YYYY-MM-DDTHH:MM:SSZ` for a Unix time in seconds (UTC, no clock access here).
#[must_use]
pub fn format_utc(unix_secs: u64) -> String {
    let (days, rem) = (unix_secs / 86_400, unix_secs % 86_400);
    // Civil-from-days (Howard Hinnant), shifted to a 0000-03-01 epoch.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_caps() -> Capabilities {
        REQUIRED_CAPABILITIES
            .iter()
            .fold(Capabilities::NONE, |acc, (_, cap)| acc.union(*cap))
    }

    fn good() -> Observations {
        Observations {
            flash_failure: None,
            handshake: true,
            negotiated_caps: all_caps(),
            device_version: Some("1.3.0".into()),
            bundled_version: Some("1.3.0".into()),
            heartbeat_ok: true,
            state_report_seen: true,
            health_seen: true,
            inputs_done: InputCheck::ALL.to_vec(),
        }
    }

    #[test]
    fn all_inputs_seen_passes() {
        let verdict = evaluate(&good());
        assert!(verdict.passed(), "{:?}", verdict.failed);
        assert_eq!(verdict.label(), "PASS");
        assert_eq!(verdict.failed_tokens(), "");
    }

    #[test]
    fn a_missing_capability_fails_and_names_it() {
        let mut obs = good();
        obs.negotiated_caps =
            Capabilities::from_bits(all_caps().bits() & !Capabilities::HOST_TAKEOVERS_V1.bits());
        let verdict = evaluate(&obs);
        assert_eq!(
            verdict.failed,
            vec![FailedCheck::Capability("HOST_TAKEOVERS_V1")]
        );
    }

    #[test]
    fn a_version_mismatch_fails() {
        let mut obs = good();
        obs.device_version = Some("1.2.0".into());
        assert_eq!(evaluate(&obs).failed, vec![FailedCheck::FirmwareVersion]);
    }

    #[test]
    fn an_unknown_version_fails() {
        let mut obs = good();
        obs.bundled_version = None;
        assert_eq!(evaluate(&obs).failed, vec![FailedCheck::FirmwareVersion]);
    }

    #[test]
    fn missing_health_fails() {
        let mut obs = good();
        obs.health_seen = false;
        assert_eq!(evaluate(&obs).failed, vec![FailedCheck::Health]);
    }

    #[test]
    fn missing_state_report_and_heartbeat_fail() {
        let mut obs = good();
        obs.state_report_seen = false;
        obs.heartbeat_ok = false;
        assert_eq!(
            evaluate(&obs).failed,
            vec![FailedCheck::Heartbeat, FailedCheck::StateReport]
        );
    }

    #[test]
    fn partial_inputs_fail_listing_the_missing_ones() {
        let mut obs = good();
        obs.inputs_done = vec![
            InputCheck::TurnRight,
            InputCheck::Press,
            InputCheck::ContextButton(1),
        ];
        let verdict = evaluate(&obs);
        assert!(!verdict.passed());
        assert_eq!(
            verdict.failed_tokens(),
            "input:turn_left;input:hold;input:button_left;input:button_right"
        );
    }

    #[test]
    fn a_flash_or_handshake_failure_stops_the_evaluation() {
        let mut obs = good();
        obs.flash_failure = Some("tool_missing");
        assert_eq!(evaluate(&obs).failed_tokens(), "flash:tool_missing");
        let mut obs = good();
        obs.handshake = false;
        obs.inputs_done.clear();
        assert_eq!(evaluate(&obs).failed, vec![FailedCheck::Handshake]);
    }

    #[test]
    fn turn_checks_need_three_detents_in_the_right_direction() {
        let mut right = InputProgress::new(InputCheck::TurnRight);
        let ccw = InputKind::Detent(Direction::Ccw);
        let cw = InputKind::Detent(Direction::Cw);
        assert!(!right.observe(ControlId::Rotary, ccw));
        assert!(!right.observe(ControlId::Rotary, cw));
        assert!(!right.observe(ControlId::Rotary, cw));
        assert!(!right.observe(ControlId::Button, InputKind::Press));
        assert!(right.observe(ControlId::Rotary, cw));
    }

    #[test]
    fn press_hold_and_each_button_match_only_their_own_control() {
        let press = InputKind::Press;
        assert!(InputCheck::Press.matches(ControlId::Button, press));
        assert!(!InputCheck::Press.matches(ControlId::Button, InputKind::Hold));
        assert!(InputCheck::Hold.matches(ControlId::Button, InputKind::Hold));
        assert!(InputCheck::ContextButton(2).matches(ControlId::ContextButton(2), press));
        assert!(!InputCheck::ContextButton(2).matches(ControlId::ContextButton(1), press));
        assert!(!InputCheck::ContextButton(0).matches(ControlId::Button, press));
    }

    #[test]
    fn csv_row_has_only_the_hash_and_versions_and_stays_one_line() {
        let mut obs = good();
        obs.inputs_done.clear();
        let verdict = evaluate(&obs);
        let row = csv_row("2026-10-09T10:00:00Z", Some("ab12\ncd,34"), &obs, &verdict);
        assert_eq!(row.lines().count(), 1);
        assert_eq!(row.matches(',').count(), CSV_HEADER.matches(',').count());
        assert!(row.starts_with("2026-10-09T10:00:00Z,ab12cd34,1.3.0,1.3.0,FAIL,input:turn_right;"));
        let none = csv_row("t", None, &obs, &verdict);
        assert!(none.starts_with("t,unknown,"));
    }

    #[test]
    fn utc_formatting_matches_known_dates() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_791_540_000), "2026-10-09T10:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn the_banner_carries_the_release_guard_marker() {
        assert!(BANNER.contains("KIVORI-PROVISION"));
    }
}
