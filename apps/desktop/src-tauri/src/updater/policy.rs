//! The updater's decisions (ADR-0015; product.md, "App updates"). Pure: no network, no clock, no
//! Tauri. The worker in [`super`] feeds it times and check results and does what it says.
//!
//! - [`availability`]: whether this build may check at all (Windows, a release config with the
//!   release endpoint and a public key). Anything else is `disabled` or `unsupported` and never
//!   touches the network.
//! - [`Schedule`]: first check 30 s after start, then every 24 h while the automatic check is on;
//!   a failed check waits for the next interval.
//! - [`decide`]: only a version newer than the running one is offered, never a skipped one, and a
//!   snoozed one only after 3 days (or when the user presses "Check now").
//! - [`Machine`]: the state the UI and tray show, and which user action is allowed in which state.

use std::time::Duration;

use semver::Version;

use crate::config::{UpdateSettings, UpdateSnooze};

/// The one place updates come from: the latest non-draft, non-pre-release GitHub Release.
pub const ENDPOINT: &str = "https://github.com/cunilab/Kivori/releases/latest/download/latest.json";
/// The first check waits this long after start, so it never delays startup.
pub const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
/// Then one check a day while the app runs.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// "Later" asks again after this many seconds.
pub const SNOOZE_SECS: u64 = 3 * 24 * 60 * 60;
/// Release notes longer than this are cut (they are shown as plain text).
pub const MAX_NOTES: usize = 8 * 1024;

/// Why the updater is off in this build. It then makes no request at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledReason {
    /// No `plugins.updater` in the config: a development build (`tauri.conf.json` alone).
    DevBuild,
    /// The release config has no public key yet, so no update could be verified.
    NoKey,
    /// The configured endpoint is not the release endpoint.
    BadEndpoint,
}

impl DisabledReason {
    pub const ALL: [Self; 3] = [Self::DevBuild, Self::NoKey, Self::BadEndpoint];

    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::DevBuild => "devBuild",
            Self::NoKey => "noKey",
            Self::BadEndpoint => "badEndpoint",
        }
    }
}

/// Whether this build may check for updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Enabled,
    Disabled(DisabledReason),
    /// Not on this OS yet (macOS): nothing is checked.
    Unsupported,
}

/// Decides from the merged Tauri config's `plugins.updater` section (`None` when absent) and the OS.
#[must_use]
pub fn availability(section: Option<&serde_json::Value>, windows: bool) -> Availability {
    if !windows {
        return Availability::Unsupported;
    }
    let Some(section) = section.filter(|s| s.is_object()) else {
        return Availability::Disabled(DisabledReason::DevBuild);
    };
    let pubkey = section
        .get("pubkey")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if pubkey.trim().is_empty() {
        return Availability::Disabled(DisabledReason::NoKey);
    }
    let endpoints = section
        .get("endpoints")
        .and_then(serde_json::Value::as_array);
    match endpoints.map(Vec::as_slice) {
        Some([only]) if only.as_str() == Some(ENDPOINT) => Availability::Enabled,
        _ => Availability::Disabled(DisabledReason::BadEndpoint),
    }
}

/// When the next automatic check is due, on the worker's monotonic clock (time since start).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    auto: bool,
    next: Option<Duration>,
    last: Option<Duration>,
}

impl Schedule {
    /// At start: the first check [`FIRST_CHECK_DELAY`] in, if the automatic check is on.
    #[must_use]
    pub fn new(auto: bool) -> Self {
        Self {
            auto,
            next: auto.then_some(FIRST_CHECK_DELAY),
            last: None,
        }
    }

    /// The next automatic check, if any.
    #[must_use]
    pub const fn next(&self) -> Option<Duration> {
        self.next
    }

    #[must_use]
    pub fn due(&self, now: Duration) -> bool {
        self.next.is_some_and(|next| now >= next)
    }

    /// How long until the next automatic check (`None` = none is scheduled).
    #[must_use]
    pub fn wait(&self, now: Duration) -> Option<Duration> {
        self.next.map(|next| next.saturating_sub(now))
    }

    /// A check ran (automatic or "Check now", whatever its result): the next one is a day later.
    pub fn checked(&mut self, now: Duration) {
        self.last = Some(now);
        self.next = self.auto.then(|| now + CHECK_INTERVAL);
    }

    /// The setting changed. Turning it on checks a day after the last check (now if that has
    /// passed), and never before the first-check delay.
    pub fn set_auto(&mut self, auto: bool, now: Duration) {
        self.auto = auto;
        self.next = auto.then(|| {
            self.last
                .map_or(now, |last| last + CHECK_INTERVAL)
                .max(now)
                .max(FIRST_CHECK_DELAY)
        });
    }

    /// Checks no later than `at` (a snooze ending), while the automatic check is on.
    pub fn wake_by(&mut self, at: Duration) {
        if self.auto {
            self.next = Some(self.next.map_or(at, |next| next.min(at)));
        }
    }
}

/// A release the endpoint announced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// Plain text, at most [`MAX_NOTES`] bytes.
    pub notes: String,
    /// `pub_date` as the manifest gives it, if it looks like an ISO-8601 time.
    pub date: Option<String>,
}

impl Release {
    /// Builds one from the manifest's fields, bounding what the UI will show.
    #[must_use]
    pub fn new(version: Version, notes: Option<&str>, date: Option<&str>) -> Self {
        let mut notes = notes.unwrap_or("").trim().to_string();
        if notes.len() > MAX_NOTES {
            let mut cut = MAX_NOTES;
            while !notes.is_char_boundary(cut) {
                cut -= 1;
            }
            notes.truncate(cut);
            notes.push('\u{2026}');
        }
        let date = date
            .filter(|d| {
                d.len() <= 40
                    && d.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-:.+ ".contains(c))
            })
            .map(str::to_string);
        Self {
            version,
            notes,
            date,
        }
    }
}

/// What to do with a release a check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    Offer,
    /// Not newer than the running version.
    NotNewer,
    /// The user skipped this version.
    Skipped,
    /// The user pressed "Later" for this version less than 3 days ago.
    Snoozed,
}

/// Decides whether `release` is offered. `manual` is "Check now": the user is asking, so a
/// snooze does not hide it (a skip still does).
#[must_use]
pub fn decide(
    running: &Version,
    release: &Version,
    settings: &UpdateSettings,
    now: u64,
    manual: bool,
) -> Offer {
    if release <= running {
        return Offer::NotNewer;
    }
    let same = |stored: &str| Version::parse(stored).is_ok_and(|v| v == *release);
    if settings.skipped_version.as_deref().is_some_and(same) {
        return Offer::Skipped;
    }
    if !manual && snooze_remaining(settings.snoozed.as_ref(), release, now).is_some() {
        return Offer::Snoozed;
    }
    Offer::Offer
}

/// Seconds left on a snooze of `release`, if it still holds.
#[must_use]
pub fn snooze_remaining(snooze: Option<&UpdateSnooze>, release: &Version, now: u64) -> Option<u64> {
    let snooze = snooze?;
    if Version::parse(&snooze.version).ok()? != *release {
        return None;
    }
    let until = snooze.at.saturating_add(SNOOZE_SECS);
    (now < until).then(|| until - now)
}

/// "Later" on `release` at `now`.
#[must_use]
pub fn snooze(release: &Version, now: u64) -> UpdateSnooze {
    UpdateSnooze {
        version: release.to_string(),
        at: now,
    }
}

/// What else owns the device right now. An offer is not shown, and an install never starts,
/// while either is true.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Busy {
    /// A firmware flash or restore is running.
    pub flash: bool,
    /// A knob gesture is open (started and not ended).
    pub gesture: bool,
}

impl Busy {
    #[must_use]
    pub const fn any(self) -> bool {
        self.flash || self.gesture
    }
}

/// What the updater is doing, as the UI and tray show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Disabled(DisabledReason),
    Unsupported,
    /// Nothing to offer (never checked, up to date, or the offer was dismissed).
    Idle,
    Checking,
    Available(Release),
    Downloading {
        release: Release,
        downloaded: u64,
        /// `None` when the server sent no size (the UI shows an indeterminate bar).
        total: Option<u64>,
    },
    /// Downloaded and verified; installs as soon as no flash or gesture is running.
    ReadyToRestart(Release),
    /// The download, verification or installer start failed. "Try again" or "Later".
    Failed(Release),
}

impl UpdateState {
    /// Every state token, in order (shared with the webview).
    pub const TOKENS: [&'static str; 8] = [
        "disabled",
        "unsupported",
        "idle",
        "checking",
        "available",
        "downloading",
        "readyToRestart",
        "failed",
    ];

    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self {
            Self::Disabled(_) => "disabled",
            Self::Unsupported => "unsupported",
            Self::Idle => "idle",
            Self::Checking => "checking",
            Self::Available(_) => "available",
            Self::Downloading { .. } => "downloading",
            Self::ReadyToRestart(_) => "readyToRestart",
            Self::Failed(_) => "failed",
        }
    }

    /// The release this state is about, if any.
    #[must_use]
    pub const fn release(&self) -> Option<&Release> {
        match self {
            Self::Available(release)
            | Self::Downloading { release, .. }
            | Self::ReadyToRestart(release)
            | Self::Failed(release) => Some(release),
            Self::Disabled(_) | Self::Unsupported | Self::Idle | Self::Checking => None,
        }
    }
}

/// A check's result, as the worker saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    UpToDate,
    Found(Release),
    /// Offline, a server error, or an answer that could not be read.
    Failed,
}

/// What a finished check led to (the worker records activity from it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckReport {
    Failed,
    /// Up to date, or a release that is not offered (older, skipped or snoozed).
    NothingToOffer,
    Offered,
    /// Offered once the flash or gesture now running is over.
    Held,
}

/// Why a user action was refused in the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NothingOffered,
    Busy,
}

impl Refused {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::NothingOffered => "There is no update to install.",
            Self::Busy => "An update is already being installed.",
        }
    }
}

/// The updater state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    state: UpdateState,
    /// The last check that got an answer (Unix seconds); a failed check leaves it.
    last_checked: Option<u64>,
    /// Found while the device was busy; offered by [`Self::settle`].
    held: Option<Release>,
}

impl Machine {
    #[must_use]
    pub const fn new(availability: Availability) -> Self {
        let state = match availability {
            Availability::Enabled => UpdateState::Idle,
            Availability::Disabled(reason) => UpdateState::Disabled(reason),
            Availability::Unsupported => UpdateState::Unsupported,
        };
        Self {
            state,
            last_checked: None,
            held: None,
        }
    }

    #[must_use]
    pub const fn state(&self) -> &UpdateState {
        &self.state
    }

    #[must_use]
    pub const fn last_checked(&self) -> Option<u64> {
        self.last_checked
    }

    /// The version the tray and dialog offer: available, or failed and waiting for "Try again".
    #[must_use]
    pub fn offered(&self) -> Option<&Version> {
        match &self.state {
            UpdateState::Available(release) | UpdateState::Failed(release) => {
                Some(&release.version)
            }
            _ => None,
        }
    }

    /// Whether a release waits for the device to be idle.
    #[must_use]
    pub const fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// Starts a check. Only from `idle`: an open offer or a running install is left alone.
    pub fn begin_check(&mut self) -> bool {
        if self.state != UpdateState::Idle {
            return false;
        }
        self.state = UpdateState::Checking;
        true
    }

    /// Ends the check started by [`Self::begin_check`].
    #[allow(clippy::needless_pass_by_value)] // the outcome is consumed into the state
    pub fn check_finished(
        &mut self,
        outcome: CheckOutcome,
        running: &Version,
        settings: &UpdateSettings,
        now: u64,
        manual: bool,
        busy: Busy,
    ) -> CheckReport {
        if self.state != UpdateState::Checking {
            return CheckReport::NothingToOffer;
        }
        self.state = UpdateState::Idle;
        let release = match outcome {
            CheckOutcome::Failed => return CheckReport::Failed,
            CheckOutcome::UpToDate => {
                self.last_checked = Some(now);
                self.held = None;
                return CheckReport::NothingToOffer;
            }
            CheckOutcome::Found(release) => release,
        };
        self.last_checked = Some(now);
        if decide(running, &release.version, settings, now, manual) != Offer::Offer {
            self.held = None;
            return CheckReport::NothingToOffer;
        }
        if busy.any() {
            self.held = Some(release);
            CheckReport::Held
        } else {
            self.held = None;
            self.state = UpdateState::Available(release);
            CheckReport::Offered
        }
    }

    /// Offers a held release once nothing is busy. Returns whether the state changed.
    pub fn settle(&mut self, busy: Busy) -> bool {
        if busy.any() || self.state != UpdateState::Idle {
            return false;
        }
        match self.held.take() {
            Some(release) => {
                self.state = UpdateState::Available(release);
                true
            }
            None => false,
        }
    }

    /// "Install & restart" (or "Try again"): starts the download.
    ///
    /// # Errors
    /// [`Refused`] when nothing is offered or an install is already running.
    pub fn begin_install(&mut self) -> Result<Release, Refused> {
        match &self.state {
            UpdateState::Available(release) | UpdateState::Failed(release) => {
                let release = release.clone();
                self.state = UpdateState::Downloading {
                    release: release.clone(),
                    downloaded: 0,
                    total: None,
                };
                Ok(release)
            }
            UpdateState::Downloading { .. } | UpdateState::ReadyToRestart(_) => Err(Refused::Busy),
            _ => Err(Refused::NothingOffered),
        }
    }

    /// One downloaded chunk. `total` is the size the server announced, if any.
    pub fn progress(&mut self, chunk: u64, size: Option<u64>) {
        if let UpdateState::Downloading {
            downloaded, total, ..
        } = &mut self.state
        {
            *downloaded = downloaded.saturating_add(chunk);
            *total = size.filter(|&size| size > 0);
        }
    }

    /// Downloaded and verified.
    pub fn downloaded(&mut self) {
        if let UpdateState::Downloading { release, .. } = &self.state {
            self.state = UpdateState::ReadyToRestart(release.clone());
        }
    }

    /// The download, verification or installer start failed.
    pub fn install_failed(&mut self) {
        if let UpdateState::Downloading { release, .. } | UpdateState::ReadyToRestart(release) =
            &self.state
        {
            self.state = UpdateState::Failed(release.clone());
        }
    }

    /// "Later" or "Skip this version": withdraws the offer and returns its version.
    ///
    /// # Errors
    /// [`Refused`] when nothing is offered or an install is already running.
    pub fn dismiss(&mut self) -> Result<Version, Refused> {
        match &self.state {
            UpdateState::Available(release) | UpdateState::Failed(release) => {
                let version = release.version.clone();
                self.state = UpdateState::Idle;
                self.held = None;
                Ok(version)
            }
            UpdateState::Downloading { .. } | UpdateState::ReadyToRestart(_) => Err(Refused::Busy),
            _ => Err(Refused::NothingOffered),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_disabled_reasons_have_distinct_tokens() {
        let tokens = DisabledReason::ALL.map(DisabledReason::token);
        assert_eq!(tokens, ["devBuild", "noKey", "badEndpoint"]);
    }

    #[test]
    fn the_state_tokens_list_every_state() {
        let release = Release::new(Version::new(1, 0, 0), None, None);
        let states = [
            UpdateState::Disabled(DisabledReason::NoKey),
            UpdateState::Unsupported,
            UpdateState::Idle,
            UpdateState::Checking,
            UpdateState::Available(release.clone()),
            UpdateState::Downloading {
                release: release.clone(),
                downloaded: 0,
                total: None,
            },
            UpdateState::ReadyToRestart(release.clone()),
            UpdateState::Failed(release),
        ];
        assert_eq!(states.map(|s| s.token()), UpdateState::TOKENS);
    }
}
