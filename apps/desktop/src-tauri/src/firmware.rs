//! Native-only firmware flashing workflow.
//!
//! The webview can only request a flash of the fixed firmware bundled into this binary. Port names
//! and filesystem paths remain inside the native process.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::activity::{ActivityEventKind, SessionActivity};
use crate::device::discovery::{default_allowlist, filter_candidates, PortCandidate};
use crate::firmware_bundle::{FirmwareBundle, FirmwareImage};
use crate::identity::{RecoveryRefusal, RestoreOption, WrongProduct};
use crate::uf2;
use serde::Serialize;

/// Maximum time allowed for one flash (entering the bootloader and writing the image).
pub const FLASH_TIMEOUT: Duration = Duration::from_secs(90);
/// Maximum time to wait for the flashed device to reconnect and complete a handshake.
pub const RECONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the bootloader drive may take to appear after the 1200-baud touch.
const BOOTLOADER_TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(200);

include!(concat!(env!("OUT_DIR"), "/firmware_sources.rs"));

/// User-visible firmware flashing phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FirmwarePhase {
    /// No flash has been requested.
    Idle,
    /// Native validation has accepted a queued flash request.
    Preparing,
    /// The native flasher is putting the board in its bootloader and copying the fixed artifact.
    Flashing,
    /// Programming finished; the desktop is waiting for the same device to handshake again.
    Reconnecting,
    /// The same device completed its post-flash handshake.
    Succeeded,
    /// Flashing or required reconnect verification failed.
    Failed,
}

/// Why a firmware update or restore did not complete. A closed set of tokens; the UI owns the wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FlashFailure {
    /// Another program holds the serial port, or the OS denied access to it.
    PortBusy,
    /// The board's bootloader drive (RPI-RP2) did not appear, so it is not in download mode.
    NoDownloadMode,
    /// The bootloader drive appeared but the firmware could not be copied to it.
    WriteFailed,
    /// The flash did not finish in time.
    Timeout,
    /// The user (or an application shutdown) cancelled the flash.
    Cancelled,
    /// The bundled firmware image is missing or could not be prepared.
    ImageUnavailable,
    /// Flashing finished but the device did not come back and verify.
    ReconnectTimedOut,
    /// The bundled image is for another product or board than the connected device; nothing was
    /// written. Both names ride in [`FirmwareStatus::wrong_product`].
    WrongProduct,
    /// More than one product could be on this port; the user picks one in
    /// [`FirmwareStatus::restore_choices`] and restores again. Nothing was written.
    ChooseProduct,
    /// Anything not recognized.
    Unknown,
}

impl FlashFailure {
    /// Every class, in declaration order.
    pub const ALL: [Self; 10] = [
        Self::PortBusy,
        Self::NoDownloadMode,
        Self::WriteFailed,
        Self::Timeout,
        Self::Cancelled,
        Self::ImageUnavailable,
        Self::ReconnectTimedOut,
        Self::WrongProduct,
        Self::ChooseProduct,
        Self::Unknown,
    ];

    /// The token the webview receives (matches the serialized form).
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::PortBusy => "portBusy",
            Self::NoDownloadMode => "noDownloadMode",
            Self::WriteFailed => "writeFailed",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::ImageUnavailable => "imageUnavailable",
            Self::ReconnectTimedOut => "reconnectTimedOut",
            Self::WrongProduct => "wrongProduct",
            Self::ChooseProduct => "chooseProduct",
            Self::Unknown => "unknown",
        }
    }

    /// Safe native status text. It carries no port, path or process output.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::PortBusy => "The device port is in use by another program.",
            Self::NoDownloadMode => "The device did not enter firmware download mode.",
            Self::WriteFailed => "The firmware could not be written to the device.",
            Self::Timeout => "Firmware flashing timed out.",
            Self::Cancelled => "Firmware flashing was cancelled.",
            Self::ImageUnavailable => "Firmware is unavailable in this application build.",
            Self::ReconnectTimedOut => {
                "Firmware flashed, but the device could not be verified after reconnecting."
            }
            Self::WrongProduct => "This firmware is for a different Kivori product.",
            Self::ChooseProduct => "Choose the Kivori product and board.",
            Self::Unknown => "Firmware flashing failed.",
        }
    }
}

/// Whether the device's firmware is older than, equal to, or newer than the one bundled here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateAdvice {
    /// The device runs the bundled version.
    UpToDate,
    /// The bundled firmware is newer than the device's.
    UpdateAvailable,
    /// The device runs newer firmware than this app bundles.
    DeviceNewer,
    /// No device connected, no bundled version, or a version that does not parse.
    Unknown,
}

impl UpdateAdvice {
    /// Every advice, in declaration order.
    pub const ALL: [Self; 4] = [
        Self::UpToDate,
        Self::UpdateAvailable,
        Self::DeviceNewer,
        Self::Unknown,
    ];

    /// The token the webview receives (matches the serialized form).
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::UpToDate => "upToDate",
            Self::UpdateAvailable => "updateAvailable",
            Self::DeviceNewer => "deviceNewer",
            Self::Unknown => "unknown",
        }
    }
}

/// Advises whether to update, from the connected device's firmware version and the bundled one.
/// Advice only: nothing ever flashes without an explicit request.
#[must_use]
pub fn update_advice(device: Option<&str>, bundled: Option<&str>) -> UpdateAdvice {
    let (Some(device), Some(bundled)) = (
        device.and_then(parse_triple),
        bundled.and_then(parse_triple),
    ) else {
        return UpdateAdvice::Unknown;
    };
    match device.cmp(&bundled) {
        std::cmp::Ordering::Less => UpdateAdvice::UpdateAvailable,
        std::cmp::Ordering::Equal => UpdateAdvice::UpToDate,
        std::cmp::Ordering::Greater => UpdateAdvice::DeviceNewer,
    }
}

fn parse_triple(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.split('.');
    let triple = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(triple)
}

/// What a flash is allowed to touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashTarget {
    /// A verified, connected device: the post-flash handshake must show the same device identity.
    Verified {
        /// Native-only port name.
        port: String,
        /// Short device-identity hash seen before flashing.
        hash: String,
    },
    /// Recovery of the one allowlisted device present: any compatible device on the port is accepted
    /// afterwards, and its new identity is recorded.
    Recovery {
        /// Native-only port name.
        port: String,
    },
}

impl FlashTarget {
    fn port(&self) -> &str {
        match self {
            Self::Verified { port, .. } | Self::Recovery { port } => port,
        }
    }
}

/// Safe status projection returned to the webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareStatus {
    /// Whether this app build contains any validated firmware images.
    pub available: bool,
    /// Current native workflow phase.
    pub phase: FirmwarePhase,
    /// Human-readable status with no port, path, or process output.
    pub message: String,
    /// Selected image byte length, or zero when no target is established.
    pub image_size: u64,
    /// Why the last attempt failed, as a token for the UI to word; `None` unless the phase is failed.
    pub failure: Option<FlashFailure>,
    /// Version for the current device or accepted flash target, when known.
    pub bundled_version: Option<String>,
    /// Whether the connected device should be updated, evaluated with the same selected image.
    pub advice: UpdateAdvice,
    /// Both names, set only with [`FlashFailure::WrongProduct`].
    pub wrong_product: Option<WrongProductInfo>,
    /// The product/board pairs to pick from, set only with [`FlashFailure::ChooseProduct`].
    pub restore_choices: Vec<RestoreChoice>,
}

/// The two names shown when an image was refused for a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrongProductInfo {
    /// What the connected device is.
    pub device: String,
    /// What the bundled image is for.
    pub image: String,
}

/// One product/board pair the user may pick for a restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreChoice {
    /// Machine id passed back with the restore request.
    pub id: String,
    /// Name shown to the user.
    pub name: String,
    /// Catalog board machine id.
    pub board: String,
    /// Board name shown to the user.
    pub board_name: String,
}

/// Result of a completed programming attempt, consumed by the device runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeTarget {
    /// Resume ordinary candidate discovery after a failed/cancelled flash.
    Discovery,
    /// Reopen only this exact previously-connected port while awaiting handshake verification.
    SamePort(String),
}

/// Small, testable flashing state machine. It retains native-only target data but exposes only a
/// [`FirmwareStatus`] projection to the IPC boundary.
#[derive(Debug, Clone)]
pub struct FlashWorkflow {
    status: FirmwareStatus,
    target: Option<FlashTarget>,
    recovered_hash: Option<String>,
    activity: Vec<SessionActivity>,
}

impl FlashWorkflow {
    /// Creates a workflow around the bundled image availability and its byte length.
    #[must_use]
    pub fn new(available: bool, image_size: u64) -> Self {
        let message = if available {
            "Firmware ready to flash.".to_string()
        } else {
            "Firmware is unavailable in this application build.".to_string()
        };
        let mut workflow = Self {
            status: FirmwareStatus {
                available,
                phase: FirmwarePhase::Idle,
                message,
                image_size: if available { image_size } else { 0 },
                failure: None,
                bundled_version: None,
                advice: UpdateAdvice::Unknown,
                wrong_product: None,
                restore_choices: Vec::new(),
            },
            target: None,
            recovered_hash: None,
            activity: Vec::new(),
        };
        workflow.observe(if available {
            ActivityEventKind::FirmwareAvailable
        } else {
            ActivityEventKind::FirmwareUnavailable
        });
        workflow
    }

    /// Refreshes target metadata while idle; an accepted operation keeps its exact image.
    pub fn select_image(&mut self, image: Option<&FirmwareImage<'_>>) {
        if !self.is_busy() {
            self.status.image_size = image.map_or(0, |image| image.bytes().len() as u64);
            self.status.bundled_version = image.map(|image| image.identity().version.clone());
            self.status.advice = UpdateAdvice::Unknown;
        }
    }

    /// Refreshes idle metadata without replacing an IPC reservation or workflow result.
    pub fn refresh_image(
        &mut self,
        image: Option<&FirmwareImage<'_>>,
        device_version: Option<&str>,
        published: &mut FirmwareStatus,
    ) {
        if self.is_busy()
            || matches!(
                published.phase,
                FirmwarePhase::Preparing | FirmwarePhase::Flashing | FirmwarePhase::Reconnecting
            )
        {
            return;
        }
        self.select_image(image);
        self.status.advice = update_advice(device_version, self.status.bundled_version.as_deref());
        published.advice = self.status.advice;
        published.image_size = self.status.image_size;
        published
            .bundled_version
            .clone_from(&self.status.bundled_version);
    }

    /// Returns the safe status projection.
    #[must_use]
    pub fn status(&self) -> &FirmwareStatus {
        &self.status
    }

    /// Whether this workflow owns the serial connection or is waiting to verify its return.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        matches!(
            self.status.phase,
            FirmwarePhase::Preparing | FirmwarePhase::Flashing | FirmwarePhase::Reconnecting
        )
    }

    /// The native-only port reserved for the reconnect check. It is never part of [`FirmwareStatus`].
    #[must_use]
    pub(crate) fn target_port(&self) -> Option<&str> {
        self.target.as_ref().map(FlashTarget::port)
    }

    /// The port to reopen for the post-flash handshake. `None` for a drive-only recovery (empty
    /// port): that board returns on whatever port the OS gives it, so discovery must find it.
    #[must_use]
    pub(crate) fn reconnect_port(&self) -> Option<&str> {
        self.target_port().filter(|port| !port.is_empty())
    }

    /// The short hash of the device accepted by the last recovery. Native-only.
    #[must_use]
    pub fn recovered_hash(&self) -> Option<&str> {
        self.recovered_hash.as_deref()
    }

    /// Drains closed firmware workflow observations for the device task to record and emit.
    pub fn drain_activity(&mut self) -> Vec<SessionActivity> {
        std::mem::take(&mut self.activity)
    }

    /// Validates and reserves the currently connected native target. Returns the port for the device
    /// thread only; callers must never expose it to IPC.
    pub fn request(
        &mut self,
        connected: bool,
        port: Option<&str>,
        device_hash: Option<&str>,
    ) -> Result<String, String> {
        if !self.status.available {
            return Err("Firmware is unavailable in this application build.".to_string());
        }
        if self.is_busy() {
            return Err("A firmware update is already in progress.".to_string());
        }
        if !connected {
            return Err("Connect a compatible device before flashing firmware.".to_string());
        }
        let port =
            port.ok_or_else(|| "The connected device is no longer available.".to_string())?;
        let device_hash = device_hash.ok_or_else(|| {
            "The connected device could not be verified for flashing.".to_string()
        })?;
        self.begin(FlashTarget::Verified {
            port: port.to_string(),
            hash: device_hash.to_string(),
        });
        Ok(port.to_string())
    }

    /// Reserves recovery of a device that cannot be verified (wrong-major firmware, a unit in ROM
    /// download mode). Requires exactly one USB-hinted port, or none while a board held in BOOTSEL
    /// shows its bootloader drive (then the port is empty); the caller must first accept its
    /// product/board recovery policy. Returns the port for the device thread only.
    pub fn request_recovery(&mut self, candidates: &[PortCandidate]) -> Result<String, String> {
        self.request_recovery_with(candidates, uf2::bootloader_drives().len())
    }

    /// [`Self::request_recovery`] with the number of mounted bootloader drives already counted.
    fn request_recovery_with(
        &mut self,
        candidates: &[PortCandidate],
        drives: usize,
    ) -> Result<String, String> {
        if !self.status.available {
            return Err("Firmware is unavailable in this application build.".to_string());
        }
        if self.is_busy() {
            return Err("A firmware update is already in progress.".to_string());
        }
        let found = filter_candidates(candidates, default_allowlist());
        let port = match found.as_slice() {
            [only] => only.port_name.clone(),
            [] if drives == 1 => String::new(),
            [] if drives > 1 => return Err(
                "More than one board is in firmware download mode; leave only the one to restore."
                    .to_string(),
            ),
            [] => return Err("No Kivori device was found to restore.".to_string()),
            _ => {
                return Err(
                    "More than one Kivori device is connected; leave only the one to restore."
                        .to_string(),
                )
            }
        };
        self.begin(FlashTarget::Recovery { port: port.clone() });
        Ok(port)
    }

    fn begin(&mut self, target: FlashTarget) {
        self.target = Some(target);
        self.recovered_hash = None;
        self.status.phase = FirmwarePhase::Preparing;
        self.status.message = "Preparing firmware update.".to_string();
        self.status.failure = None;
        self.status.wrong_product = None;
        self.status.restore_choices.clear();
        self.observe(ActivityEventKind::FirmwareFlashRequested);
        self.observe(ActivityEventKind::FirmwarePreparing);
    }

    /// Marks that the native owner released its serial session before starting the flasher.
    pub fn mark_serial_released(&mut self) {
        self.observe(ActivityEventKind::FirmwareSerialReleased);
    }

    /// Marks that the serial session has been released and the flasher now owns the port.
    pub fn mark_flashing(&mut self) {
        self.status.phase = FirmwarePhase::Flashing;
        self.status.message = "Flashing firmware.".to_string();
        self.observe(ActivityEventKind::FirmwareFlasherStarted);
    }

    /// Marks that a queued update lost its device before the flash process could acquire the port.
    pub fn fail_preparation(&mut self) {
        self.fail_preparation_with(FlashFailure::Unknown);
        self.status.message =
            "The connected device is no longer available for firmware flashing.".to_string();
    }

    /// Marks a refused or abandoned preparation with the failure class the UI should explain.
    pub fn fail_preparation_with(&mut self, failure: FlashFailure) {
        self.status.phase = FirmwarePhase::Failed;
        self.status.message = failure.message().to_string();
        self.status.failure = Some(failure);
        self.target = None;
        self.status.wrong_product = None;
        self.status.restore_choices.clear();
        self.observe(ActivityEventKind::FirmwarePreparationRejected);
    }

    /// Reports a recovery policy refusal without reserving a target or starting the flasher.
    pub fn refuse_recovery(&mut self, refusal: RecoveryRefusal) {
        match refusal {
            RecoveryRefusal::NoCandidate => self.fail_preparation_with(FlashFailure::Unknown),
        }
    }

    /// Refuses an image for a device it is not for (tier A). Nothing was written; the status
    /// carries both names so the UI can say what the device is and what the image is for.
    pub fn fail_wrong_product(&mut self, refusal: WrongProduct) {
        self.fail_preparation_with(FlashFailure::WrongProduct);
        self.status.wrong_product = Some(WrongProductInfo {
            device: refusal.device,
            image: refusal.image,
        });
    }

    /// Stops recovery until the user confirms the product/board pair on the port.
    pub fn ask_restore_choice(&mut self, options: Vec<RestoreOption>) {
        self.fail_preparation_with(FlashFailure::ChooseProduct);
        self.status.restore_choices = options
            .into_iter()
            .map(|o| RestoreChoice {
                id: o.id,
                name: o.name,
                board: o.board,
                board_name: o.board_name,
            })
            .collect();
    }

    /// Completes process execution and selects the safe connection-resume mode.
    pub fn finish(&mut self, result: Result<(), FlashFailure>) -> ResumeTarget {
        match result {
            Ok(()) => {
                self.status.phase = FirmwarePhase::Reconnecting;
                self.status.message =
                    "Firmware flashed. Reconnecting to device for verification.".to_string();
                self.observe(ActivityEventKind::FirmwareFlashSucceeded);
                self.observe(ActivityEventKind::FirmwareReconnectWaiting);
                ResumeTarget::SamePort(self.target_port().unwrap_or_default().to_string())
            }
            Err(failure) => {
                self.status.phase = FirmwarePhase::Failed;
                self.status.message = failure.message().to_string();
                self.status.failure = Some(failure);
                self.target = None;
                self.observe(ActivityEventKind::FirmwareUpdateFailed);
                ResumeTarget::Discovery
            }
        }
    }

    /// Accepts the post-flash handshake only if both its port and already-verified device identity
    /// match the original native target.
    pub fn handshake(&mut self, port: &str, device_hash: &str, compatible: bool) -> bool {
        if self.status.phase != FirmwarePhase::Reconnecting || !compatible {
            return false;
        }
        match &self.target {
            Some(FlashTarget::Verified { port: p, hash }) if p == port && hash == device_hash => {}
            // An empty port is a board that was in its bootloader: any port it comes back on is it.
            Some(FlashTarget::Recovery { port: p }) if p == port || p.is_empty() => {
                self.recovered_hash = Some(device_hash.to_string());
            }
            _ => return false,
        }
        self.status.phase = FirmwarePhase::Succeeded;
        self.status.message = "Firmware update verified.".to_string();
        self.target = None;
        self.observe(ActivityEventKind::FirmwarePostFlashVerified);
        true
    }

    /// Fails a pending post-flash reconnect without treating any later discovered device as success.
    pub fn reconnect_timed_out(&mut self) {
        if self.status.phase == FirmwarePhase::Reconnecting {
            self.status.phase = FirmwarePhase::Failed;
            self.status.message = FlashFailure::ReconnectTimedOut.message().to_string();
            self.status.failure = Some(FlashFailure::ReconnectTimedOut);
            self.target = None;
            self.observe(ActivityEventKind::FirmwareReconnectTimedOut);
        }
    }

    fn observe(&mut self, kind: ActivityEventKind) {
        self.activity.push(SessionActivity::new(kind, None));
    }
}

/// The complete immutable, build-validated image set.
#[must_use]
pub fn bundled_images() -> &'static FirmwareBundle<'static> {
    static IMAGES: std::sync::OnceLock<FirmwareBundle<'static>> = std::sync::OnceLock::new();
    IMAGES.get_or_init(|| {
        FirmwareBundle::parse(FIRMWARE_IMAGES).expect("build-validated firmware images")
    })
}

/// Exact product/board lookup, with no implicit product or first-entry default.
#[must_use]
pub fn image_for(product: u16, board: u16) -> Option<&'static FirmwareImage<'static>> {
    bundled_images().get(product, board)
}

/// Returns the initial status without assuming a device identity.
#[must_use]
pub fn initial_status() -> FirmwareStatus {
    FlashWorkflow::new(!bundled_images().images().is_empty(), 0)
        .status
        .clone()
}

/// Runs an explicitly selected, validated image through the native UF2 flasher: the board is put
/// in its ROM bootloader (or already is, for a recovery), the image is copied to its drive, and the
/// caller then verifies the reconnect handshake. Pass an empty `port` when no serial port exists
/// (a board held in BOOTSEL).
///
/// Cancellation and [`FLASH_TIMEOUT`] are polled; the copy itself cannot be interrupted, so a
/// cancelled or timed-out write is abandoned on its own thread.
///
/// # Errors
/// The [`FlashFailure`] class describing why the image was not flashed.
pub fn flash_image(
    image: &FirmwareImage<'_>,
    port: &str,
    cancel: &AtomicBool,
) -> Result<(), FlashFailure> {
    let started = Instant::now();
    let uf2 = uf2::elf_to_uf2(image.bytes()).ok_or(FlashFailure::ImageUnavailable)?;
    let drive = enter_bootloader(port, cancel)?;
    write_image(
        &drive.join("kivori-firmware.uf2"),
        uf2,
        cancel,
        started,
        FLASH_TIMEOUT,
    )
}

/// The one drive in `drives` that is not in `known`. `None` while there is none yet; `Unknown` when
/// several appeared at once, since which one is the board being flashed cannot be told.
fn single_new_drive(
    drives: Vec<PathBuf>,
    known: &[PathBuf],
) -> Result<Option<PathBuf>, FlashFailure> {
    let mut fresh = drives.into_iter().filter(|drive| !known.contains(drive));
    match (fresh.next(), fresh.next()) {
        (only, None) => Ok(only),
        _ => Err(FlashFailure::Unknown),
    }
}

/// Returns the bootloader drive of the board being flashed. With a `port` it is reached with the
/// Arduino-style 1200-baud touch and must be a drive that appeared since, so another board already
/// in BOOTSEL is never flashed by mistake. An empty `port` is a drive-only recovery: the board
/// already shows its (single) drive.
fn enter_bootloader(port: &str, cancel: &AtomicBool) -> Result<PathBuf, FlashFailure> {
    let known = if port.is_empty() {
        Vec::new()
    } else {
        uf2::bootloader_drives()
    };
    let mut touch_failed = false;
    if !port.is_empty() {
        // The firmware reboots into the ROM bootloader when the host sets 1200 baud and drops DTR.
        // On Windows the 1200-baud line coding set inside `open` already triggers it, and a later
        // call can fail although the board is rebooting, so only a missing drive means failure.
        match serialport::new(port, 1200).open() {
            Ok(mut serial) => {
                let _ = serial.write_data_terminal_ready(false);
            }
            Err(_) => touch_failed = true,
        }
    }
    await_new_drive(
        uf2::bootloader_drives,
        &known,
        touch_failed,
        cancel,
        BOOTLOADER_TIMEOUT,
    )
}

/// Polls `drives` until exactly one drive not in `known` shows up. Gives up after `timeout` with
/// `PortBusy` if the touch could not even open the port, else `NoDownloadMode`.
fn await_new_drive(
    mut drives: impl FnMut() -> Vec<PathBuf>,
    known: &[PathBuf],
    touch_failed: bool,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<PathBuf, FlashFailure> {
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(FlashFailure::Cancelled);
        }
        if let Some(drive) = single_new_drive(drives(), known)? {
            return Ok(drive);
        }
        if started.elapsed() >= timeout {
            return Err(if touch_failed {
                FlashFailure::PortBusy
            } else {
                FlashFailure::NoDownloadMode
            });
        }
        std::thread::sleep(POLL);
    }
}

fn write_image(
    path: &Path,
    uf2: Vec<u8>,
    cancel: &AtomicBool,
    started: Instant,
    timeout: Duration,
) -> Result<(), FlashFailure> {
    let target = path.to_path_buf();
    let writer = std::thread::spawn(move || {
        let mut file = std::fs::File::create(target)?;
        file.write_all(&uf2)?;
        // Best effort: the ROM reboots on the last block, so the drive may vanish before the sync.
        let _ = file.sync_all();
        Ok::<(), std::io::Error>(())
    });
    while !writer.is_finished() {
        if cancel.load(Ordering::SeqCst) {
            return Err(FlashFailure::Cancelled);
        }
        if started.elapsed() >= timeout {
            return Err(FlashFailure::Timeout);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // The ROM reboots as soon as the last block lands, so the drive can vanish under the write; a
    // gone drive therefore counts as written. The reconnect handshake is the real success check.
    let written = matches!(writer.join(), Ok(Ok(())));
    if written || !path.parent().is_some_and(uf2::is_bootloader_drive) {
        Ok(())
    } else {
        Err(FlashFailure::WriteFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive() -> tempfile::TempDir {
        let drive = tempfile::tempdir().unwrap();
        std::fs::write(drive.path().join("INFO_UF2.TXT"), "Board-ID: RPI-RP2\n").unwrap();
        drive
    }

    #[test]
    fn a_drive_only_recovery_has_no_port_to_reopen() {
        let mut flash = FlashWorkflow::new(true, 0);
        flash.begin(FlashTarget::Recovery {
            port: String::new(),
        });
        assert_eq!(flash.reconnect_port(), None);
        flash.begin(FlashTarget::Recovery {
            port: "COM7".into(),
        });
        assert_eq!(flash.reconnect_port(), Some("COM7"));
    }

    #[test]
    fn only_a_drive_that_appeared_after_the_touch_qualifies() {
        let (a, b, c) = (
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        );
        // Another board already in BOOTSEL (`a`) is never picked.
        assert_eq!(
            single_new_drive(vec![a.clone()], std::slice::from_ref(&a)),
            Ok(None)
        );
        assert_eq!(
            single_new_drive(vec![a.clone(), b.clone()], std::slice::from_ref(&a)),
            Ok(Some(b.clone()))
        );
        assert_eq!(
            single_new_drive(vec![a.clone(), b, c], &[a]),
            Err(FlashFailure::Unknown)
        );
        // A drive-only recovery (nothing known) takes its single drive and refuses several.
        assert_eq!(
            single_new_drive(vec![PathBuf::from("/x")], &[]),
            Ok(Some(PathBuf::from("/x")))
        );
        assert_eq!(
            single_new_drive(vec![PathBuf::from("/x"), PathBuf::from("/y")], &[]),
            Err(FlashFailure::Unknown)
        );
    }

    #[test]
    fn writes_the_image_and_treats_a_vanished_drive_as_written() {
        let cancel = AtomicBool::new(false);
        let drive = drive();
        let file = drive.path().join("kivori-firmware.uf2");
        assert_eq!(
            write_image(&file, vec![7; 512], &cancel, Instant::now(), FLASH_TIMEOUT),
            Ok(())
        );
        assert_eq!(std::fs::read(&file).unwrap(), vec![7; 512]);
        // A write error with the drive still mounted is a real failure; with the drive gone it is not.
        let blocked = drive.path().join("not-a-file");
        std::fs::create_dir(&blocked).unwrap();
        assert_eq!(
            write_image(&blocked, vec![1], &cancel, Instant::now(), FLASH_TIMEOUT),
            Err(FlashFailure::WriteFailed)
        );
        let gone = tempfile::tempdir()
            .unwrap()
            .path()
            .join("RPI-RP2")
            .join("x.uf2");
        assert_eq!(
            write_image(&gone, vec![1], &cancel, Instant::now(), FLASH_TIMEOUT),
            Ok(())
        );
    }

    const TIMEOUT: Duration = Duration::from_millis(250);

    /// A drive lister that answers each poll from `script` and then repeats the last answer.
    fn scripted(script: Vec<Vec<PathBuf>>) -> impl FnMut() -> Vec<PathBuf> {
        let mut polls = 0;
        move || {
            let answer = script[polls.min(script.len() - 1)].clone();
            polls += 1;
            answer
        }
    }

    fn drives(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn no_drive_after_the_touch_is_no_download_mode_and_a_failed_touch_is_port_busy() {
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        assert_eq!(
            await_new_drive(scripted(vec![vec![]]), &[], false, &cancel, TIMEOUT),
            Err(FlashFailure::NoDownloadMode)
        );
        assert!(started.elapsed() >= TIMEOUT);
        // A board already in BOOTSEL is not the one touched: still no download mode.
        assert_eq!(
            await_new_drive(
                scripted(vec![drives(&["/a"])]),
                &drives(&["/a"]),
                false,
                &cancel,
                TIMEOUT
            ),
            Err(FlashFailure::NoDownloadMode)
        );
        assert_eq!(
            await_new_drive(scripted(vec![vec![]]), &[], true, &cancel, TIMEOUT),
            Err(FlashFailure::PortBusy)
        );
    }

    #[test]
    fn a_drive_that_appears_without_an_opened_port_still_succeeds_on_windows_style_touches() {
        let cancel = AtomicBool::new(false);
        // The open "failed" but the line coding already rebooted the board: the drive wins.
        let late = scripted(vec![vec![], vec![], drives(&["/new"])]);
        assert_eq!(
            await_new_drive(late, &[], true, &cancel, Duration::from_secs(5)),
            Ok(PathBuf::from("/new"))
        );
    }

    #[test]
    fn the_new_drive_is_picked_over_a_foreign_one_and_two_new_drives_are_refused() {
        let cancel = AtomicBool::new(false);
        assert_eq!(
            await_new_drive(
                scripted(vec![drives(&["/foreign", "/new"])]),
                &drives(&["/foreign"]),
                false,
                &cancel,
                TIMEOUT
            ),
            Ok(PathBuf::from("/new"))
        );
        assert_eq!(
            await_new_drive(
                scripted(vec![drives(&["/foreign", "/n1", "/n2"])]),
                &drives(&["/foreign"]),
                false,
                &cancel,
                TIMEOUT
            ),
            Err(FlashFailure::Unknown)
        );
        // A drive-only recovery (nothing known): one drive is used, two are refused.
        assert_eq!(
            await_new_drive(
                scripted(vec![drives(&["/only"])]),
                &[],
                false,
                &cancel,
                TIMEOUT
            ),
            Ok(PathBuf::from("/only"))
        );
        assert_eq!(
            await_new_drive(
                scripted(vec![drives(&["/a", "/b"])]),
                &[],
                false,
                &cancel,
                TIMEOUT
            ),
            Err(FlashFailure::Unknown)
        );
    }

    #[test]
    fn cancelling_while_waiting_for_the_drive_returns_within_a_poll() {
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let trigger = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            flag.store(true, Ordering::SeqCst);
        });
        let started = Instant::now();
        assert_eq!(
            await_new_drive(
                scripted(vec![vec![]]),
                &[],
                false,
                &cancel,
                Duration::from_secs(30)
            ),
            Err(FlashFailure::Cancelled)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        trigger.join().unwrap();
        // Cancelled before the first poll never lists a drive at all.
        let listed = std::cell::Cell::new(false);
        assert_eq!(
            await_new_drive(
                || {
                    listed.set(true);
                    vec![]
                },
                &[],
                false,
                &cancel,
                TIMEOUT
            ),
            Err(FlashFailure::Cancelled)
        );
        assert!(!listed.get());
    }

    /// A path whose `File::create` blocks until something opens its other end: a stalled copy.
    #[cfg(unix)]
    fn stalled_target() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("kivori-firmware.uf2");
        assert!(std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        (dir, fifo)
    }

    #[cfg(unix)]
    #[test]
    fn a_stalled_copy_times_out_or_is_cancelled_without_waiting_for_it() {
        let (_dir, fifo) = stalled_target();
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        assert_eq!(
            write_image(&fifo, vec![1; 512], &cancel, started, TIMEOUT),
            Err(FlashFailure::Timeout)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        // The timeout counts from the start of the whole flash, not from the copy.
        let late = Instant::now() - Duration::from_secs(10);
        assert_eq!(
            write_image(&fifo, vec![1; 512], &cancel, late, TIMEOUT),
            Err(FlashFailure::Timeout)
        );
        cancel.store(true, Ordering::SeqCst);
        let started = Instant::now();
        assert_eq!(
            write_image(
                &fifo,
                vec![1; 512],
                &cancel,
                started,
                Duration::from_secs(60)
            ),
            Err(FlashFailure::Cancelled)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        // The abandoned writers stay blocked in `File::create` and die with the test process.
    }

    #[test]
    fn a_failed_write_is_write_failed_only_while_the_drive_is_still_mounted() {
        let cancel = AtomicBool::new(false);
        let mounted = drive();
        // The directory the image should land in is read-only in effect: the target is a directory.
        let blocked = mounted.path().join("kivori-firmware.uf2");
        std::fs::create_dir(&blocked).unwrap();
        assert_eq!(
            write_image(
                &blocked,
                vec![1; 512],
                &cancel,
                Instant::now(),
                FLASH_TIMEOUT
            ),
            Err(FlashFailure::WriteFailed)
        );
        // Same error, but the drive (its INFO_UF2.TXT) is gone: the board rebooted, so it counts as written.
        std::fs::remove_file(mounted.path().join("INFO_UF2.TXT")).unwrap();
        assert_eq!(
            write_image(
                &blocked,
                vec![1; 512],
                &cancel,
                Instant::now(),
                FLASH_TIMEOUT
            ),
            Ok(())
        );
    }

    #[test]
    fn a_drive_only_recovery_needs_exactly_one_drive_and_a_port_needs_exactly_one_kivori() {
        let port = |name: &str, vid, pid| PortCandidate::new(name, Some(vid), Some(pid));
        let (c3, rp) = (port("COM7", 0x303A, 0x1001), port("COM8", 0x2E8A, 0x000A));
        let fresh = || FlashWorkflow::new(true, 512);
        // No port: the single mounted drive is used (empty port), none or several are refused.
        let mut flash = fresh();
        assert_eq!(flash.request_recovery_with(&[], 1), Ok(String::new()));
        assert_eq!(flash.reconnect_port(), None);
        assert_eq!(flash.status().phase, FirmwarePhase::Preparing);
        for drives in [0, 2, 5] {
            let mut flash = fresh();
            assert!(flash.request_recovery_with(&[], drives).is_err());
            assert_eq!(flash.status().phase, FirmwarePhase::Idle);
        }
        // One v2 port wins whatever drives are mounted; two Kivori ports are ambiguous either way.
        for drives in [0, 1, 2] {
            assert_eq!(
                fresh().request_recovery_with(std::slice::from_ref(&rp), drives),
                Ok("COM8".to_string())
            );
            assert!(fresh()
                .request_recovery_with(&[c3.clone(), rp.clone()], drives)
                .is_err());
        }
        // A foreign serial port is not a candidate, so a lone drive is still a drive-only recovery.
        let foreign = port("COM1", 0x0403, 0x6001);
        assert_eq!(
            fresh().request_recovery_with(&[foreign], 1),
            Ok(String::new())
        );
        // Busy and unavailable are refused first.
        let mut busy = fresh();
        busy.request_recovery_with(&[], 1).unwrap();
        assert!(busy.request_recovery_with(&[], 1).is_err());
        assert!(FlashWorkflow::new(false, 0)
            .request_recovery_with(&[], 1)
            .is_err());
    }

    fn kinds(flash: &mut FlashWorkflow) -> Vec<ActivityEventKind> {
        flash.drain_activity().into_iter().map(|a| a.kind).collect()
    }

    #[test]
    fn a_drive_only_recovery_walks_the_phases_in_order_and_accepts_the_board_on_any_port() {
        use ActivityEventKind as K;
        let mut flash = FlashWorkflow::new(true, 512);
        flash.drain_activity();
        assert_eq!(flash.request_recovery_with(&[], 1), Ok(String::new()));
        assert_eq!(flash.status().phase, FirmwarePhase::Preparing);
        flash.mark_serial_released();
        flash.mark_flashing();
        assert_eq!(flash.status().phase, FirmwarePhase::Flashing);
        assert_eq!(flash.finish(Ok(())), ResumeTarget::SamePort(String::new()));
        assert_eq!(flash.status().phase, FirmwarePhase::Reconnecting);
        assert_eq!(
            flash.reconnect_port(),
            None,
            "discovery must find the board"
        );
        assert!(flash.is_busy());
        // An incompatible device is no recovery; a compatible one on whichever port is.
        assert!(!flash.handshake("/dev/cu.usbmodem1", "0badf00d", false));
        assert_eq!(flash.status().phase, FirmwarePhase::Reconnecting);
        assert!(flash.handshake("/dev/cu.usbmodem9", "0badf00d", true));
        assert_eq!(flash.status().phase, FirmwarePhase::Succeeded);
        assert_eq!(flash.recovered_hash(), Some("0badf00d"));
        assert!(!flash.is_busy());
        assert_eq!(
            kinds(&mut flash),
            [
                K::FirmwareFlashRequested,
                K::FirmwarePreparing,
                K::FirmwareSerialReleased,
                K::FirmwareFlasherStarted,
                K::FirmwareFlashSucceeded,
                K::FirmwareReconnectWaiting,
                K::FirmwarePostFlashVerified,
            ]
        );
        // The success is spent: a late second handshake, or a timeout, changes nothing.
        assert!(!flash.handshake("/dev/cu.usbmodem9", "0badf00d", true));
        flash.reconnect_timed_out();
        assert_eq!(flash.status().phase, FirmwarePhase::Succeeded);
    }

    #[test]
    fn a_cancelled_or_unreconnected_drive_only_recovery_fails_and_never_accepts_a_later_device() {
        for end in ["cancelled", "write failed", "no reconnect"] {
            let mut flash = FlashWorkflow::new(true, 512);
            flash.request_recovery_with(&[], 1).unwrap();
            flash.mark_serial_released();
            flash.mark_flashing();
            let expected = match end {
                "cancelled" => {
                    assert_eq!(
                        flash.finish(Err(FlashFailure::Cancelled)),
                        ResumeTarget::Discovery
                    );
                    FlashFailure::Cancelled
                }
                "write failed" => {
                    assert_eq!(
                        flash.finish(Err(FlashFailure::WriteFailed)),
                        ResumeTarget::Discovery
                    );
                    FlashFailure::WriteFailed
                }
                _ => {
                    flash.finish(Ok(()));
                    flash.reconnect_timed_out();
                    FlashFailure::ReconnectTimedOut
                }
            };
            assert_eq!(flash.status().phase, FirmwarePhase::Failed, "{end}");
            assert_eq!(flash.status().failure, Some(expected), "{end}");
            assert!(!flash.is_busy());
            assert!(
                !flash.handshake("/dev/cu.usbmodem9", "0badf00d", true),
                "{end}"
            );
            assert_eq!(flash.recovered_hash(), None);
            assert_eq!(flash.status().phase, FirmwarePhase::Failed);
            // The next attempt starts clean.
            assert_eq!(flash.request_recovery_with(&[], 1), Ok(String::new()));
            assert_eq!(flash.status().failure, None);
        }
    }

    #[test]
    fn a_verified_update_waits_on_its_own_port_and_a_board_swapped_on_the_way_back_fails() {
        let mut flash = FlashWorkflow::new(true, 512);
        flash
            .request(true, Some("COM7"), Some("pip-rp2040"))
            .unwrap();
        assert_eq!(
            flash.finish(Ok(())),
            ResumeTarget::SamePort("COM7".to_string())
        );
        assert_eq!(flash.reconnect_port(), Some("COM7"));
        // Same unit on another port, or another unit on this one: neither is the flashed board.
        assert!(!flash.handshake("COM8", "pip-rp2040", true));
        assert!(!flash.handshake("COM7", "other-unit", true));
        assert_eq!(flash.status().phase, FirmwarePhase::Reconnecting);
        flash.reconnect_timed_out();
        assert_eq!(
            flash.status().failure,
            Some(FlashFailure::ReconnectTimedOut)
        );
        assert!(!flash.handshake("COM7", "pip-rp2040", true));
    }
}
