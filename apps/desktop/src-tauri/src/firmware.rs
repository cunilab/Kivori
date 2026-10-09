//! Native-only firmware flashing workflow.
//!
//! The webview can only request a flash of the fixed firmware bundled into this binary. Port names,
//! filesystem paths, and `espflash` output remain inside the native process.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::activity::{ActivityEventKind, SessionActivity};
use crate::device::discovery::{filter_candidates, PortCandidate, DEFAULT_ALLOWLIST};
use serde::Serialize;

/// Maximum time allowed for one `espflash` process before it is terminated.
pub const FLASH_TIMEOUT: Duration = Duration::from_secs(90);
/// Maximum time to wait for the flashed device to reconnect and complete a handshake.
pub const RECONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// How much of `espflash`'s stderr is kept for classification (the tail, where errors are reported).
pub const STDERR_LIMIT: usize = 8 * 1024;

/// The firmware version stamped into this build by `build.rs` (empty when no firmware is embedded).
pub const BUNDLED_FIRMWARE_VERSION: &str = env!("KIVORI_BUNDLED_FIRMWARE_VERSION");

/// The fixed firmware artifact embedded by `build.rs`; an empty file means this build cannot flash.
pub static BUNDLED_FIRMWARE: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/kivori-firmware.elf"));

/// User-visible firmware flashing phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FirmwarePhase {
    /// No flash has been requested.
    Idle,
    /// Native validation has accepted a queued flash request.
    Preparing,
    /// `espflash` owns the port and is programming the fixed artifact.
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
    /// The flashing tool is not part of this installation (or the OS refused to run it).
    ToolMissing,
    /// Another program holds the serial port, or the OS denied access to it.
    PortBusy,
    /// The chip did not answer as a bootloader; it is not in download mode.
    NoDownloadMode,
    /// The flash did not finish in time.
    Timeout,
    /// The user (or an application shutdown) cancelled the flash.
    Cancelled,
    /// The bundled firmware image is missing or could not be prepared.
    ImageUnavailable,
    /// Flashing finished but the device did not come back and verify.
    ReconnectTimedOut,
    /// Anything not recognized.
    Unknown,
}

impl FlashFailure {
    /// Every class, in declaration order.
    pub const ALL: [Self; 8] = [
        Self::ToolMissing,
        Self::PortBusy,
        Self::NoDownloadMode,
        Self::Timeout,
        Self::Cancelled,
        Self::ImageUnavailable,
        Self::ReconnectTimedOut,
        Self::Unknown,
    ];

    /// The token the webview receives (matches the serialized form).
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::ToolMissing => "toolMissing",
            Self::PortBusy => "portBusy",
            Self::NoDownloadMode => "noDownloadMode",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::ImageUnavailable => "imageUnavailable",
            Self::ReconnectTimedOut => "reconnectTimedOut",
            Self::Unknown => "unknown",
        }
    }

    /// Safe native status text. It carries no port, path or process output.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::ToolMissing => "The firmware tool is missing from this installation.",
            Self::PortBusy => "The device port is in use by another program.",
            Self::NoDownloadMode => "The device did not enter firmware download mode.",
            Self::Timeout => "Firmware flashing timed out.",
            Self::Cancelled => "Firmware flashing was cancelled.",
            Self::ImageUnavailable => "Firmware is unavailable in this application build.",
            Self::ReconnectTimedOut => {
                "Firmware flashed, but the device could not be verified after reconnecting."
            }
            Self::Unknown => "Firmware flashing failed.",
        }
    }
}

/// How an `espflash` run ended, as far as the process boundary can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspflashExit {
    /// The sidecar could not be found or started.
    ToolMissing,
    /// The image could not be prepared for the tool.
    ImageUnavailable,
    /// The process was killed after exceeding [`FLASH_TIMEOUT`].
    TimedOut,
    /// The process was killed on request.
    Cancelled,
    /// The process exited by itself; `None` when it was ended by a signal.
    Code(Option<i32>),
}

/// Maps how `espflash` ended plus its (bounded) stderr to a failure class.
///
/// Pure and conservative: only well-known `espflash` 4.x / OS message fragments are recognized and
/// anything else is [`FlashFailure::Unknown`]. The stderr text is inspected here and then dropped;
/// it is never logged or sent anywhere.
#[must_use]
pub fn classify_espflash(exit: EspflashExit, stderr: &str) -> FlashFailure {
    match exit {
        EspflashExit::ToolMissing => return FlashFailure::ToolMissing,
        EspflashExit::ImageUnavailable => return FlashFailure::ImageUnavailable,
        EspflashExit::TimedOut => return FlashFailure::Timeout,
        EspflashExit::Cancelled => return FlashFailure::Cancelled,
        EspflashExit::Code(_) => {}
    }
    let text = stderr.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));
    // Port ownership first: a busy port also makes the connect attempt fail.
    if has(&[
        "access is denied",
        "permission denied",
        "device or resource busy",
        "resource busy",
        "could not open port",
        "failed to open serial port",
        "the serial port could not be opened",
    ]) {
        FlashFailure::PortBusy
    } else if has(&[
        "wrong boot mode",
        "download mode",
        "failed to connect",
        "error while connecting",
        "no serial data received",
        "invalid response",
        "serial port not found",
    ]) {
        FlashFailure::NoDownloadMode
    } else if has(&["timeout while running", "timed out"]) {
        FlashFailure::Timeout
    } else if has(&[
        "failed to parse elf",
        "not a valid elf",
        "no such file or directory",
    ]) {
        FlashFailure::ImageUnavailable
    } else {
        FlashFailure::Unknown
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
    /// Whether this app build contains a valid fixed firmware image.
    pub available: bool,
    /// Current native workflow phase.
    pub phase: FirmwarePhase,
    /// Human-readable status with no port, path, or process output.
    pub message: String,
    /// Bundled image byte length, or zero when unavailable.
    pub image_size: u64,
    /// Why the last attempt failed, as a token for the UI to word; `None` unless the phase is failed.
    pub failure: Option<FlashFailure>,
    /// Version of the firmware bundled in this build, when known.
    pub bundled_version: Option<String>,
    /// Whether the connected device should be updated. Filled when the status is read.
    pub advice: UpdateAdvice,
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
                bundled_version: (available && !BUNDLED_FIRMWARE_VERSION.is_empty())
                    .then(|| BUNDLED_FIRMWARE_VERSION.to_string()),
                advice: UpdateAdvice::Unknown,
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
    /// download mode). Allowed only when exactly one allowlisted port is present, so the flasher can
    /// never be pointed at the wrong board. Returns the port for the device thread only.
    pub fn request_recovery(&mut self, candidates: &[PortCandidate]) -> Result<String, String> {
        if !self.status.available {
            return Err("Firmware is unavailable in this application build.".to_string());
        }
        if self.is_busy() {
            return Err("A firmware update is already in progress.".to_string());
        }
        let found = filter_candidates(candidates, DEFAULT_ALLOWLIST);
        let [only] = found.as_slice() else {
            return Err(if found.is_empty() {
                "No Kivori device was found to restore.".to_string()
            } else {
                "More than one Kivori device is connected; leave only the one to restore."
                    .to_string()
            });
        };
        let port = only.port_name.clone();
        self.begin(FlashTarget::Recovery { port: port.clone() });
        Ok(port)
    }

    fn begin(&mut self, target: FlashTarget) {
        self.target = Some(target);
        self.recovered_hash = None;
        self.status.phase = FirmwarePhase::Preparing;
        self.status.message = "Preparing firmware update.".to_string();
        self.status.failure = None;
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
        self.observe(ActivityEventKind::FirmwarePreparationRejected);
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
            Some(FlashTarget::Recovery { port: p }) if p == port => {
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

/// Returns whether the bundled artifact is an ELF32 little-endian RISC-V executable.
#[must_use]
pub fn bundled_image_available() -> bool {
    is_elf32_riscv(BUNDLED_FIRMWARE)
}

/// Returns the initial safe status for the currently embedded artifact.
#[must_use]
pub fn initial_status() -> FirmwareStatus {
    FlashWorkflow::new(bundled_image_available(), BUNDLED_FIRMWARE.len() as u64)
        .status
        .clone()
}

/// Runs the fixed bundled image through `espflash` for the given native-owned port.
///
/// `espflash` stderr is captured in-process (bounded) only to classify a failure; it is never logged
/// or exposed. The child is killed and reaped on timeout or application shutdown.
///
/// # Errors
/// The [`FlashFailure`] class describing why the image was not flashed.
pub fn flash_bundled(port: &str, cancel: &AtomicBool) -> Result<(), FlashFailure> {
    if !bundled_image_available() {
        return Err(FlashFailure::ImageUnavailable);
    }
    let tool = find_espflash().ok_or(FlashFailure::ToolMissing)?;
    let temp = write_temp_image(BUNDLED_FIRMWARE).ok_or(FlashFailure::ImageUnavailable)?;
    let result = run_espflash(&tool, port, &temp, cancel);
    let _ = std::fs::remove_file(&temp);
    result
}

fn is_elf32_riscv(bytes: &[u8]) -> bool {
    bytes.len() >= 20
        && bytes.starts_with(b"\x7fELF")
        && bytes[4] == 1
        && bytes[5] == 1
        && bytes[18] == 0xF3
        && bytes[19] == 0
}

/// The sidecar's file name on this platform.
#[must_use]
pub const fn espflash_file_name() -> &'static str {
    if cfg!(windows) {
        "espflash.exe"
    } else {
        "espflash"
    }
}

/// Looks for the bundled `espflash` sidecar directly inside `dir`.
#[must_use]
pub fn find_espflash_in(dir: &Path) -> Option<PathBuf> {
    Some(dir.join(espflash_file_name())).filter(|path| path.is_file())
}

/// Resolves the flashing tool: the sidecar next to the application executable first (where the
/// installer puts it). Only developer builds (`device-studio`) fall back to `PATH` and
/// `~/.cargo/bin`; a release build never runs whatever happens to be on `PATH`.
fn find_espflash() -> Option<PathBuf> {
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().and_then(find_espflash_in));
    #[cfg(feature = "device-studio")]
    {
        beside_exe.or_else(find_espflash_on_dev_path)
    }
    #[cfg(not(feature = "device-studio"))]
    {
        beside_exe
    }
}

#[cfg(feature = "device-studio")]
fn find_espflash_on_dev_path() -> Option<PathBuf> {
    let exe = espflash_file_name();
    let mut candidates = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(exe))
        .collect::<Vec<_>>();
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        candidates.push(PathBuf::from(home).join(".cargo").join("bin").join(exe));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn write_temp_image(image: &[u8]) -> Option<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!(
        "kivori-firmware-{}-{stamp}.elf",
        std::process::id()
    ));
    std::fs::write(&path, image).ok()?;
    Some(path)
}

/// Drains `reader` to the end, keeping only the last [`STDERR_LIMIT`] bytes.
fn read_tail(mut reader: impl Read) -> Vec<u8> {
    let mut tail: Vec<u8> = Vec::new();
    let mut chunk = [0_u8; 1024];
    while let Ok(read) = reader.read(&mut chunk) {
        if read == 0 {
            break;
        }
        tail.extend_from_slice(&chunk[..read]);
        if tail.len() > STDERR_LIMIT {
            tail.drain(..tail.len() - STDERR_LIMIT);
        }
    }
    tail
}

fn run_espflash(
    tool: &Path,
    port: &str,
    image: &Path,
    cancel: &AtomicBool,
) -> Result<(), FlashFailure> {
    let mut command = Command::new(tool);
    command
        .arg("flash")
        .arg("--port")
        .arg(port)
        .arg("--chip")
        .arg("esp32c3")
        .arg("--non-interactive")
        .arg("--skip-update-check")
        .arg(image)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|_| classify_espflash(EspflashExit::ToolMissing, ""))?;
    // A reader thread keeps the pipe drained so a chatty child can never block on a full buffer.
    let stderr = child
        .stderr
        .take()
        .map(|pipe| std::thread::spawn(move || read_tail(pipe)));
    let started = Instant::now();
    let exit = loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            break EspflashExit::Cancelled;
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break EspflashExit::Code(Some(0)),
            Ok(Some(status)) => break EspflashExit::Code(status.code()),
            Ok(None) if started.elapsed() >= FLASH_TIMEOUT => {
                let _ = child.kill();
                break EspflashExit::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => {
                // A failed status probe does not reap the child. Terminate it before the unconditional
                // `wait` below so a broken process handle cannot strand the device runtime.
                let _ = child.kill();
                break EspflashExit::Code(None);
            }
        }
    };
    let _ = child.wait();
    let captured = stderr
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    if exit == EspflashExit::Code(Some(0)) {
        return Ok(());
    }
    // The text is only inspected for a class; it is dropped here and never logged or emitted.
    Err(classify_espflash(exit, &String::from_utf8_lossy(&captured)))
}
