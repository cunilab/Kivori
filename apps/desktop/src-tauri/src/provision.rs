//! Pure evaluator for the per-unit QA helper (`examples/provision.rs`, M3 S7).
//!
//! DEVELOPMENT-ONLY (behind `device-studio`): the example gathers what the unit did, this module
//! decides PASS or FAIL and formats the log row. No I/O and no clock, so it is table-testable.
//! The log row carries only the short device hash and versions: never the raw device id or a port.

use crate::catalog::Catalog;
use crate::firmware_marker::ImageIdentity;
// Every capability name the catalog may require, with its flag.
use crate::identity::CAPABILITY_NAMES as KNOWN_CAPABILITIES;
use crate::identity::{check_image_for_device, DeviceIdentity, IdentitySource, WrongProduct};
use kivori_model::input::Direction;
use kivori_model::Capabilities;
use kivori_protocol::identity::{ChipRecord, ChipRecordState, RECORD_LEN};
use kivori_protocol::{ControlId, InputKind};

/// Printed by the example. It is also the marker `scripts/check-release-surface.sh` greps for, so a
/// release build that contains this module fails the guard.
pub const BANNER: &str = "KIVORI-PROVISION per-unit QA";

/// How long a unit holds the heartbeat before the checks are read.
pub const HEARTBEAT_HOLD_SECS: u64 = 5;
/// How many detents each turn check needs.
pub const DETENTS_REQUIRED: u32 = 3;

/// The product this QA helper checks (the only one that exists so far).
const PRODUCT_ID: &str = "pala";

/// Every capability a beta unit must negotiate: the catalog's list for the product.
///
/// # Panics
/// If the catalog names a capability this build does not know; a test rejects that.
#[must_use]
pub fn required_capabilities() -> Vec<(&'static str, Capabilities)> {
    let product = Catalog::embedded()
        .product(PRODUCT_ID)
        .expect("the catalog lists the QA product");
    product
        .required_capabilities
        .iter()
        .map(|name| {
            KNOWN_CAPABILITIES
                .iter()
                .find(|(known, _)| known == name)
                .copied()
                .unwrap_or_else(|| panic!("catalog names unknown capability `{name}`"))
        })
        .collect()
}

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
    /// `Some(passed)` when an identity burn was requested and its post-burn verification ran.
    pub identity_check: Option<bool>,
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
    /// The unit does not report the burned identity (source `Chip`, same product, board, revision).
    Identity,
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
            Self::Identity => "identity".into(),
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
    for (name, cap) in required_capabilities() {
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
    if obs.identity_check == Some(false) {
        failed.push(FailedCheck::Identity);
    }
    Verdict { failed }
}

/// The CSV header line.
pub const CSV_HEADER: &str = "timestamp_utc,device_hash,device_fw,bundled_fw,result,failed_checks,\
identity_source,identity_product,identity_board,identity_rev";

/// The identity columns of one log row: product and board machine ids (never the raw device id).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityLog {
    /// `legacy`, `build` or `chip`.
    pub source: Option<&'static str>,
    /// Product machine id, `None` when unknown to the catalog.
    pub product: Option<String>,
    /// Board machine id, `None` when unknown to the catalog.
    pub board: Option<String>,
    /// Hardware revision, `None` when unspecified.
    pub rev: Option<u8>,
}

impl IdentityLog {
    /// The columns for what the device reported (`None`: it never did).
    #[must_use]
    pub fn from_device(identity: Option<&DeviceIdentity>) -> Self {
        let Some(identity) = identity else {
            return Self::default();
        };
        let resolved = identity.resolve();
        Self {
            source: Some(identity.source.token()),
            product: resolved.map(|(p, _)| p.id.clone()),
            board: resolved.map(|(_, b)| b.id.clone()),
            rev: identity.hw_rev,
        }
    }
}

/// One log row (no trailing newline). Only the short hash and versions; every field is restricted
/// to a safe alphabet so a stray comma or newline cannot corrupt the file.
#[must_use]
pub fn csv_row(
    timestamp_utc: &str,
    device_hash: Option<&str>,
    obs: &Observations,
    verdict: &Verdict,
    identity: &IdentityLog,
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
        "{},{},{},{},{},{},{},{},{},{}",
        field(Some(timestamp_utc)),
        field(device_hash),
        field(obs.device_version.as_deref()),
        field(obs.bundled_version.as_deref()),
        verdict.label(),
        verdict.failed_tokens(),
        field(identity.source),
        field(identity.product.as_deref()),
        field(identity.board.as_deref()),
        field(identity.rev.map(|r| r.to_string()).as_deref()),
    )
}

// ---- identity burn (ADR-0014 step 3): ONE-TIME-PROGRAMMABLE eFuses, so every decision is pure ----

/// The espefuse major version this tool was written against (`esptool==4.7.0`, docs/provisioning.md).
pub const ESPEFUSE_MAJOR: u32 = 4;
/// The C3 `BLOCK_USR_DATA` size in bytes; `burn_block_data` insists on a file of exactly this size.
pub const USR_DATA_BLOCK_LEN: usize = 32;

/// Command-line options, as given (not yet resolved).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args {
    /// `--product <id>`.
    pub product: Option<String>,
    /// `--board <id>`.
    pub board: Option<String>,
    /// `--rev <n>`.
    pub rev: Option<String>,
    /// `--burn-identity`.
    pub burn: bool,
}

/// Parses the arguments after the program name. There is deliberately no flag that confirms a burn.
///
/// # Errors
/// A usage message for an unknown or incomplete option.
pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (arg.as_str(), None),
        };
        let mut value = |what: &str| -> Result<String, String> {
            inline
                .clone()
                .or_else(|| iter.next().cloned())
                .ok_or_else(|| format!("{name} needs a {what}."))
        };
        match name {
            "--product" => out.product = Some(value("product id")?),
            "--board" => out.board = Some(value("board id")?),
            "--rev" => out.rev = Some(value("revision number")?),
            "--burn-identity" if inline.is_none() => out.burn = true,
            // The app's espflash invocation has no whole-chip erase option.
            "--erase" => {
                return Err(
                    "--erase is not supported: the bundled flashing path has no erase option."
                        .into(),
                )
            }
            _ => return Err(format!("unknown option `{arg}`.")),
        }
    }
    Ok(out)
}

/// The usage line shown with an argument error.
pub const USAGE: &str =
    "usage: just provision [--product <id> --board <id> --rev <n> [--burn-identity]]";

/// A resolved identity to write: catalog ids and the wire numbers they stand for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityPlan {
    /// Product machine id (what the operator must type to confirm).
    pub product_id: String,
    /// Board machine id.
    pub board_id: String,
    /// Product wire number.
    pub product: u16,
    /// Board wire number.
    pub board: u16,
    /// Hardware revision, 1..=255.
    pub rev: u8,
}

impl IdentityPlan {
    /// The chip record this plan writes.
    #[must_use]
    pub fn record(&self) -> ChipRecord {
        ChipRecord {
            product: self.product,
            board: self.board,
            hw_rev: Some(self.rev),
        }
    }
}

/// Resolves the options through the catalog. `None` when no identity option was given.
///
/// # Errors
/// An unknown id (listing the valid ones), a bad revision, an incomplete set of options, or
/// `--burn-identity` without a product, board and revision.
pub fn resolve_plan(catalog: &Catalog, args: &Args) -> Result<Option<IdentityPlan>, String> {
    if args.product.is_none() && args.board.is_none() && args.rev.is_none() {
        return if args.burn {
            Err("--burn-identity needs --product, --board and --rev.".into())
        } else {
            Ok(None)
        };
    }
    let (Some(product_id), Some(board_id), Some(rev)) = (&args.product, &args.board, &args.rev)
    else {
        return Err("--product, --board and --rev must be given together.".into());
    };
    let Some(product) = catalog.product(product_id) else {
        let valid: Vec<&str> = catalog.products().iter().map(|p| p.id.as_str()).collect();
        return Err(format!(
            "unknown product `{product_id}`; valid products: {}.",
            valid.join(", ")
        ));
    };
    let Some(board) = product.boards.iter().find(|b| &b.id == board_id) else {
        let valid: Vec<&str> = product.boards.iter().map(|b| b.id.as_str()).collect();
        return Err(format!(
            "unknown board `{board_id}` for `{product_id}`; valid boards: {}.",
            valid.join(", ")
        ));
    };
    let rev: u8 = rev
        .parse()
        .ok()
        .filter(|r| *r >= 1)
        .ok_or_else(|| format!("--rev must be a whole number from 1 to 255, not `{rev}`."))?;
    Ok(Some(IdentityPlan {
        product_id: product.id.clone(),
        board_id: board.id.clone(),
        product: product.number,
        board: board.number,
        rev,
    }))
}

/// Lowercase hex, space separated.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The exact record, as shown before anything is written (and printed by the dry run).
#[must_use]
pub fn describe_record(plan: &IdentityPlan) -> String {
    let bytes = plan.record().encode();
    let crc = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    format!(
        "  product:   {} (wire {})\n  board:     {} (wire {})\n  revision:  {}\n  record:    {}\n  crc-32:    0x{crc:08x}",
        plan.product_id,
        plan.product,
        plan.board_id,
        plan.board,
        plan.rev,
        hex(&bytes),
    )
}

/// The file `espefuse burn_block_data` takes: the record padded with zeros to the whole block.
#[must_use]
pub fn block_file(plan: &IdentityPlan) -> [u8; USR_DATA_BLOCK_LEN] {
    let mut out = [0u8; USR_DATA_BLOCK_LEN];
    out[..RECORD_LEN].copy_from_slice(&plan.record().encode());
    out
}

/// What the chip's `BLOCK_USR_DATA` allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockCheck {
    /// All zero: safe to burn.
    Blank,
    /// Already holds exactly this record: nothing to burn.
    AlreadyProvisioned,
    /// Anything else: never write. The text says why.
    Refuse(String),
}

/// Decides from the block as read (the whole 32 bytes). Only a blank block may be written.
#[must_use]
pub fn check_block(block: &[u8], plan: &IdentityPlan) -> BlockCheck {
    if block.len() != USR_DATA_BLOCK_LEN {
        return BlockCheck::Refuse(format!(
            "read {} bytes of the user data block, expected {USR_DATA_BLOCK_LEN}.",
            block.len()
        ));
    }
    if block.iter().all(|&b| b == 0) {
        return BlockCheck::Blank;
    }
    let tail_clear = block[RECORD_LEN..].iter().all(|&b| b == 0);
    match ChipRecord::decode(block) {
        ChipRecordState::Valid(found) if tail_clear && found == plan.record() => {
            BlockCheck::AlreadyProvisioned
        }
        ChipRecordState::Valid(_) if tail_clear => BlockCheck::Refuse(
            "the chip already holds a different identity record; eFuses cannot be changed.".into(),
        ),
        _ => BlockCheck::Refuse(
            "the chip's user data block is not blank and is not this record; refusing to write."
                .into(),
        ),
    }
}

/// The operator confirms by typing the product id exactly (only the line ending is ignored).
#[must_use]
pub fn confirmation_matches(input: &str, plan: &IdentityPlan) -> bool {
    input.trim_end_matches(['\n', '\r']) == plan.product_id
}

/// The image about to be flashed must be for the product and board being burned, or the unit would
/// show the wrong-board screen right after.
///
/// # Errors
/// A message naming both sides.
pub fn check_image_matches_plan(
    image: Option<&ImageIdentity>,
    plan: &IdentityPlan,
) -> Result<(), String> {
    match image {
        Some(i) if i.product == plan.product && i.board == plan.board => Ok(()),
        Some(_) => Err(format!(
            "the bundled firmware is not for {} on {}; nothing was burned.",
            plan.product_id, plan.board_id
        )),
        None => Err("the bundled firmware has no identity record; nothing was burned.".into()),
    }
}

/// The identity guard `flash_firmware` applies, for the provisioning flash: a unit that answered
/// the pre-flash handshake must be the image's product and board (legacy counts as the Kivori Pip
/// on the C3 SuperMini). `None` means the unit never handshook (blank or recovering).
///
/// # Errors
/// [`WrongProduct`] naming both sides.
pub fn guard_before_flash(
    image: Option<&ImageIdentity>,
    device: Option<&DeviceIdentity>,
) -> Result<(), WrongProduct> {
    match (image, device) {
        (Some(image), Some(device)) => check_image_for_device(image, device),
        _ => Ok(()),
    }
}

/// The espefuse version out of any command's first output line (`espefuse.py v4.7.0`).
#[must_use]
pub fn parse_espefuse_version(output: &str) -> Option<(u32, u32, u32)> {
    let line = output
        .lines()
        .find(|l| l.trim_start().starts_with("espefuse"))?;
    let (_, version) = line.split_once(" v")?;
    let mut parts = version
        .split(|c: char| !c.is_ascii_digit())
        .map(str::parse::<u32>);
    Some((
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ))
}

/// Whether this espefuse major is the one the tool was written against.
#[must_use]
pub const fn espefuse_version_accepted(version: (u32, u32, u32)) -> bool {
    version.0 == ESPEFUSE_MAJOR
}

/// After the burn: the unit must report exactly the burned record, from the chip.
#[must_use]
pub fn verify_identity(plan: &IdentityPlan, reported: Option<&DeviceIdentity>) -> bool {
    reported.is_some_and(|r| {
        r.source == IdentitySource::Chip
            && r.product == plan.product
            && r.board == plan.board
            && r.hw_rev == Some(plan.rev)
    })
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

    #[test]
    fn catalog_capabilities_are_all_known() {
        assert_eq!(required_capabilities().len(), 11);
    }

    fn all_caps() -> Capabilities {
        required_capabilities()
            .iter()
            .fold(Capabilities::NONE, |acc, (_, cap)| acc.union(*cap))
    }

    fn good() -> Observations {
        Observations {
            flash_failure: None,
            handshake: true,
            negotiated_caps: all_caps(),
            device_version: Some("0.1.0".into()),
            bundled_version: Some("0.1.0".into()),
            heartbeat_ok: true,
            state_report_seen: true,
            health_seen: true,
            inputs_done: InputCheck::ALL.to_vec(),
            identity_check: None,
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
        obs.device_version = Some("0.0.9".into());
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
        let none_log = IdentityLog::default();
        let row = csv_row(
            "2026-10-09T10:00:00Z",
            Some("ab12\ncd,34"),
            &obs,
            &verdict,
            &none_log,
        );
        assert_eq!(row.lines().count(), 1);
        assert_eq!(row.matches(',').count(), CSV_HEADER.matches(',').count());
        assert!(row.starts_with("2026-10-09T10:00:00Z,ab12cd34,0.1.0,0.1.0,FAIL,input:turn_right;"));
        let none = csv_row("t", None, &obs, &verdict, &none_log);
        assert!(none.starts_with("t,unknown,"));
        assert!(none.ends_with(",unknown,unknown,unknown,unknown"));
    }

    fn plan() -> IdentityPlan {
        resolve_plan(
            Catalog::embedded(),
            &parse_args(&strings(&[
                "--product",
                "pala",
                "--board=c3-supermini",
                "--rev",
                "2",
            ]))
            .unwrap(),
        )
        .unwrap()
        .unwrap()
    }

    fn strings(a: &[&str]) -> Vec<String> {
        a.iter().map(ToString::to_string).collect()
    }

    fn chip_identity(rev: Option<u8>, source: IdentitySource) -> DeviceIdentity {
        DeviceIdentity {
            product: 1,
            board: 1,
            hw_rev: rev,
            source,
        }
    }

    #[test]
    fn no_arguments_means_no_identity_and_no_burn() {
        let args = parse_args(&[]).unwrap();
        assert!(!args.burn);
        assert_eq!(resolve_plan(Catalog::embedded(), &args), Ok(None));
    }

    #[test]
    fn arguments_resolve_through_the_catalog() {
        let plan = plan();
        assert_eq!(
            (plan.product, plan.board, plan.rev, plan.product_id.as_str()),
            (1, 1, 2, "pala")
        );
        assert_eq!(plan.record().encode()[..6], [1, 1, 0, 1, 0, 2]);
    }

    #[test]
    fn bad_arguments_are_errors_listing_the_valid_ids() {
        let resolve = |a: &[&str]| resolve_plan(Catalog::embedded(), &parse_args(&strings(a))?);
        let err =
            resolve(&["--product", "lamp", "--board", "c3-supermini", "--rev", "1"]).unwrap_err();
        assert!(err.contains("lamp") && err.contains("pala"), "{err}");
        let err = resolve(&["--product", "pala", "--board", "x", "--rev", "1"]).unwrap_err();
        assert!(err.contains("c3-supermini"), "{err}");
        for rev in ["0", "256", "-1", "x", ""] {
            assert!(
                resolve(&["--product", "pala", "--board", "c3-supermini", "--rev", rev]).is_err(),
                "{rev}"
            );
        }
        assert!(resolve(&["--product", "pala"]).is_err());
        assert!(resolve(&["--burn-identity"]).is_err());
        assert!(parse_args(&strings(&["--yes"])).is_err());
        assert!(parse_args(&strings(&["--burn-identity=1"])).is_err());
        assert!(parse_args(&strings(&["--rev"])).is_err());
        assert!(parse_args(&strings(&["--erase"])).is_err());
    }

    #[test]
    fn burn_flag_is_the_only_switch_and_defaults_off() {
        let args = parse_args(&strings(&["--product", "a", "--board", "b", "--rev", "1"])).unwrap();
        assert!(!args.burn);
        assert!(parse_args(&strings(&["--burn-identity"])).unwrap().burn);
    }

    #[test]
    fn the_dry_run_text_shows_fields_bytes_and_crc() {
        let text = describe_record(&plan());
        assert!(text.contains("pala (wire 1)") && text.contains("revision:  2"));
        assert!(text.contains("01 01 00 01 00 02 00 00"), "{text}");
        let bytes = plan().record().encode();
        assert!(text.contains(&hex(&bytes)));
        assert!(text.contains("crc-32:    0x"));
    }

    #[test]
    fn the_block_file_is_the_whole_block_and_still_decodes() {
        let file = block_file(&plan());
        assert_eq!(file.len(), USR_DATA_BLOCK_LEN);
        assert_eq!(
            ChipRecord::decode(&file),
            ChipRecordState::Valid(plan().record())
        );
        assert!(file[RECORD_LEN..].iter().all(|&b| b == 0));
    }

    #[test]
    fn only_a_blank_block_may_be_written() {
        let plan = plan();
        assert_eq!(check_block(&[0; 32], &plan), BlockCheck::Blank);
        assert_eq!(
            check_block(&block_file(&plan), &plan),
            BlockCheck::AlreadyProvisioned
        );
        // A different valid record, a damaged one, junk elsewhere, a short read: all refused.
        let other = IdentityPlan {
            rev: 3,
            ..plan.clone()
        };
        assert!(matches!(
            check_block(&block_file(&other), &plan),
            BlockCheck::Refuse(_)
        ));
        let mut damaged = block_file(&plan);
        damaged[9] ^= 1;
        assert!(matches!(
            check_block(&damaged, &plan),
            BlockCheck::Refuse(_)
        ));
        let mut junk = [0u8; 32];
        junk[31] = 1;
        assert!(matches!(check_block(&junk, &plan), BlockCheck::Refuse(_)));
        let mut tail = block_file(&plan);
        tail[20] = 1;
        assert!(matches!(check_block(&tail, &plan), BlockCheck::Refuse(_)));
        assert!(matches!(
            check_block(&[0; 12], &plan),
            BlockCheck::Refuse(_)
        ));
        assert!(matches!(check_block(&[], &plan), BlockCheck::Refuse(_)));
    }

    #[test]
    fn confirmation_needs_the_exact_product_id() {
        let plan = plan();
        assert!(confirmation_matches("pala\n", &plan));
        assert!(confirmation_matches("pala\r\n", &plan));
        assert!(confirmation_matches("pala", &plan));
        for bad in ["", "\n", "y", "yes", "Pala", " pala", "pala ", "desk"] {
            assert!(!confirmation_matches(bad, &plan), "{bad:?}");
        }
    }

    #[test]
    fn the_image_must_be_for_the_burned_product() {
        let plan = plan();
        let image = |p, b| ImageIdentity {
            product: p,
            board: b,
            version: "0.1.0".into(),
            min_protocol: (0, 1),
        };
        assert!(check_image_matches_plan(Some(&image(1, 1)), &plan).is_ok());
        assert!(check_image_matches_plan(Some(&image(1, 2)), &plan).is_err());
        assert!(check_image_matches_plan(Some(&image(2, 1)), &plan).is_err());
        assert!(check_image_matches_plan(None, &plan).is_err());
    }

    #[test]
    fn the_flash_guard_refuses_a_mismatch_and_accepts_legacy() {
        let image = ImageIdentity {
            product: 1,
            board: 1,
            version: "0.1.0".into(),
            min_protocol: (0, 1),
        };
        assert!(guard_before_flash(Some(&image), Some(&DeviceIdentity::legacy())).is_ok());
        assert!(guard_before_flash(Some(&image), None).is_ok());
        let other = DeviceIdentity {
            product: 2,
            ..DeviceIdentity::legacy()
        };
        assert!(guard_before_flash(Some(&image), Some(&other)).is_err());
    }

    #[test]
    fn espefuse_versions_parse_and_only_the_pinned_major_is_accepted() {
        let v = parse_espefuse_version("warning\nespefuse.py v4.7.0\nusage").unwrap();
        assert_eq!(v, (4, 7, 0));
        assert!(espefuse_version_accepted(v));
        assert_eq!(parse_espefuse_version("espefuse v5.1.0"), Some((5, 1, 0)));
        assert!(!espefuse_version_accepted((5, 1, 0)));
        assert!(!espefuse_version_accepted((3, 9, 0)));
        assert_eq!(
            parse_espefuse_version("espefuse.py v4.7.0-dev1"),
            Some((4, 7, 0))
        );
        assert_eq!(parse_espefuse_version("nothing"), None);
        assert_eq!(parse_espefuse_version("espefuse.py vX"), None);
    }

    #[test]
    fn verification_needs_chip_source_and_the_same_numbers() {
        let plan = plan();
        assert!(verify_identity(
            &plan,
            Some(&chip_identity(Some(2), IdentitySource::Chip))
        ));
        assert!(!verify_identity(
            &plan,
            Some(&chip_identity(Some(2), IdentitySource::Build))
        ));
        assert!(!verify_identity(
            &plan,
            Some(&chip_identity(Some(2), IdentitySource::Legacy))
        ));
        assert!(!verify_identity(
            &plan,
            Some(&chip_identity(Some(3), IdentitySource::Chip))
        ));
        assert!(!verify_identity(
            &plan,
            Some(&chip_identity(None, IdentitySource::Chip))
        ));
        assert!(!verify_identity(&plan, None));
    }

    #[test]
    fn a_failed_identity_check_fails_the_unit_and_the_csv_carries_the_identity() {
        let mut obs = good();
        obs.identity_check = Some(false);
        let verdict = evaluate(&obs);
        assert_eq!(verdict.failed, vec![FailedCheck::Identity]);
        obs.identity_check = Some(true);
        assert!(evaluate(&obs).passed());
        let log = IdentityLog::from_device(Some(&chip_identity(Some(2), IdentitySource::Chip)));
        let row = csv_row("t", Some("ab"), &obs, &evaluate(&obs), &log);
        assert!(row.ends_with(",chip,pala,c3-supermini,2"), "{row}");
        assert_eq!(row.matches(',').count(), CSV_HEADER.matches(',').count());
        assert!(CSV_HEADER
            .starts_with("timestamp_utc,device_hash,device_fw,bundled_fw,result,failed_checks,"));
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
