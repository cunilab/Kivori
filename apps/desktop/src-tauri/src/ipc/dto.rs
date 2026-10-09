//! IPC DTOs and pure projections (contracts/ipc.md §4).
//!
//! These are the ONLY shapes that cross to the webview. They carry no raw device id, payload bytes,
//! serial handles, or paths. All enum tokens are the lowercase wire strings the frontend expects; the
//! projections are pure functions of the internal state, so they are unit-testable without Tauri.

use std::time::Instant;

use kivori_model::{
    Capabilities, CompanionState, ConnectionState, MascotAction, MascotPersonality,
    ProtocolVersion, SendableState,
};
use kivori_protocol::{MascotActionApplied, PROTOCOL_MAJOR, PROTOCOL_MINOR};
use serde::Serialize;

use crate::activity::{
    ActivityEvent, ActivityEventKind, ActivityMetadata, ActivityOutcome, ActivitySeverity,
    ActivitySource,
};
use crate::device::fsm::ConnectionManager;
use crate::device::session::SessionDiagnostics;
use crate::orchestrator::Orchestrator;
use crate::platform::host_events::{HostPresence, HOST_PRESENCE_TOKENS};

/// Lowercase wire token for a connection state (ipc.md §4).
#[must_use]
pub fn connection_token(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Connecting => "connecting",
        ConnectionState::Connected => "connected",
        ConnectionState::Incompatible => "incompatible",
        ConnectionState::Disconnected => "disconnected",
        ConnectionState::Error => "error",
    }
}

/// Lowercase wire token for a companion state.
#[must_use]
pub fn companion_token(state: CompanionState) -> &'static str {
    match state {
        CompanionState::Booting => "booting",
        CompanionState::Idle => "idle",
        CompanionState::Happy => "happy",
        CompanionState::Busy => "busy",
        CompanionState::Sleeping => "sleeping",
        CompanionState::Offline => "offline",
    }
}

#[must_use]
pub fn mascot_action_token(action: MascotAction) -> &'static str {
    match action {
        MascotAction::Greet => "greet",
        MascotAction::Pet => "pet",
        MascotAction::Tickle => "tickle",
        MascotAction::Surprise => "surprise",
        MascotAction::Comfort => "comfort",
    }
}

#[must_use]
pub fn mascot_personality_token(personality: MascotPersonality) -> &'static str {
    match personality {
        MascotPersonality::Cozy => "cozy",
        MascotPersonality::Playful => "playful",
        MascotPersonality::Calm => "calm",
    }
}

/// Lowercase wire token for a sendable state.
#[must_use]
pub fn sendable_token(state: SendableState) -> &'static str {
    match state {
        SendableState::Idle => "idle",
        SendableState::Happy => "happy",
        SendableState::Busy => "busy",
        SendableState::Sleeping => "sleeping",
    }
}

/// `major.minor` protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProtocolVersionDto {
    /// Major version.
    pub major: u16,
    /// Minor version.
    pub minor: u16,
}

impl From<ProtocolVersion> for ProtocolVersionDto {
    fn from(v: ProtocolVersion) -> Self {
        Self {
            major: v.major,
            minor: v.minor,
        }
    }
}

/// Application/build info (all builds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    /// Desktop application version.
    pub app_version: String,
    /// Desktop protocol version.
    pub protocol_version: ProtocolVersionDto,
    /// Protocol major versions the desktop supports.
    pub supported_majors: Vec<u16>,
    /// Whether Device Studio is present in this build (false in release).
    pub device_studio_enabled: bool,
}

/// Whether Kivori starts when the user signs in (read from the OS entry, never stored in config).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupSettingsDto {
    /// Whether the OS login entry exists.
    pub launch_at_login: bool,
    /// Which OS this build runs on: `windows`, `macos` or `other` (for the switch's wording).
    pub platform: &'static str,
}

/// The platform tokens `StartupSettingsDto.platform` can carry (shared with the webview).
pub const STARTUP_PLATFORM_TOKENS: [&str; 3] = ["windows", "macos", "other"];

impl StartupSettingsDto {
    /// Builds the DTO for this OS.
    #[must_use]
    pub const fn new(launch_at_login: bool) -> Self {
        Self {
            launch_at_login,
            platform: if cfg!(windows) {
                "windows"
            } else if cfg!(target_os = "macos") {
                "macos"
            } else {
                "other"
            },
        }
    }
}

/// Safe connected-device summary (never the raw identity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfoDto {
    /// Device firmware version (`major.minor.patch`).
    pub firmware_version: String,
    /// Device protocol version.
    pub protocol_version: ProtocolVersionDto,
    /// Short, non-reversible hash of the device identity.
    pub device_id_hash_short: String,
}

/// The three-axis connection snapshot surfaced to the UI (ipc.md §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatusDto {
    /// Connection lifecycle state.
    pub connection: String,
    /// The desktop's desired sendable state (always present; default `idle`).
    pub desired: String,
    /// The device's reported state; `null` until the first `StateReport`.
    pub reported: Option<String>,
    /// Connected-device summary; present only when connected.
    pub device: Option<DeviceInfoDto>,
    /// Human-readable reason when `connection == "incompatible"`.
    pub incompatible_reason: Option<String>,
    /// Consecutive reconnect attempts.
    pub retry_count: u32,
    /// Within-process port-session identity. Changes whenever device uptime may reset.
    pub connection_generation: u32,
    /// Whether current device session supports transient mascot interactions.
    pub mascot_interaction: bool,
    /// Most recent correlated device acknowledgment for a social action in this session.
    pub mascot_action: Option<MascotActionAppliedDto>,
    /// What the computer is doing: `active`, `locked` or `sleeping`.
    pub host: &'static str,
}

/// Acknowledged physical action cue, safe to replay in Device Studio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MascotActionAppliedDto {
    pub action: String,
    pub personality: String,
    pub seed: u32,
    pub applied_at_ms: u32,
}

/// One typed session activity event safe to send to the webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEventDto {
    /// Process-monotonic event identifier.
    pub id: u64,
    /// Native-generated ISO-8601 timestamp.
    pub at: String,
    /// Closed event-kind token.
    #[serde(rename = "type")]
    pub event_type: ActivityEventTypeDto,
    /// Native-generated human-readable summary.
    pub summary: String,
    /// Closed severity token.
    pub severity: ActivitySeverityDto,
    /// Closed source token.
    pub source: ActivitySourceDto,
    /// Closed outcome token.
    pub outcome: ActivityOutcomeDto,
    /// Optional fixed-shape, allowlisted event details.
    pub metadata: Option<ActivityMetadataDto>,
}

/// Fixed-shape activity metadata; there is deliberately no arbitrary details map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityMetadataDto {
    /// Connection state for a lifecycle transition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ActivityConnectionStateDto>,
    /// Consecutive reconnect attempts.
    pub retry_count: u32,
    /// Monotonic elapsed-ms marker.
    pub elapsed_ms: u32,
    /// Safe device diagnostic category, when the event originated on the device.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_category: Option<ActivityDiagnosticCategoryDto>,
    /// Stable, safe device diagnostic code, when supplied by the device protocol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<ProtocolVersionDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id_hash_short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_play: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_at_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autonomous: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_category: Option<ProtocolMalformedCategoryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_len: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reported: Option<String>,
}

/// Closed activity-event type token serialized to the webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityEventTypeDto {
    ConnectionAttempted,
    ConnectionOpened,
    HandshakeStarted,
    HandshakeSucceeded,
    ConnectionRetryScheduled,
    IncompatibleFirmware,
    ConnectionIoFailure,
    HandshakeTimedOut,
    HeartbeatTimedOut,
    ConnectionRecovered,
    ConnectionDisconnected,
    /// A device connection lifecycle state changed.
    ConnectionStateChanged,
    DeviceNegotiated,
    PersonalityConfigured,
    SelfPlayConfigured,
    StateRequested,
    MirroredStateRequested,
    ManualSocialActionRequested,
    AutonomousSocialActionRequested,
    SocialActionApplied,
    StateSynchronized,
    DeviceStateObserved,
    DeviceDiagnosticFraming,
    DeviceDiagnosticChecksum,
    DeviceDiagnosticVersion,
    DeviceDiagnosticPayload,
    DeviceSequenceGap,
    DeviceDisplayFault,
    DeviceLinkLost,
    DeviceDiagnosticUnknown,
    DeviceError,
    DeviceBusy,
    DeviceTimedOut,
    ProtocolMalformedFrame,
    ProtocolSequenceGap,
    ActionRequested,
    ActionCompleted,
    ActionFailed,
    DeviceDiscovered,
    DeviceRejected,
    ProtocolMessageRejected,
    ProtocolFailed,
    FirmwareUpdateStarted,
    FirmwareUpdateCompleted,
    FirmwareUpdateFailed,
    FirmwareAvailable,
    FirmwareUnavailable,
    FirmwareFlashRequested,
    FirmwarePreparing,
    FirmwareSerialReleased,
    FirmwareFlasherStarted,
    FirmwareFlashSucceeded,
    FirmwareReconnectWaiting,
    FirmwareReconnectTimedOut,
    FirmwarePostFlashVerified,
    FirmwarePreparationRejected,
    SessionNonceUnavailable,
    InputStaleSessionRejected,
    InputUnstartedGestureRejected,
    InputStale,
    VolumeWriteFailed,
    AudioEndpointChanged,
    AudioEndpointLost,
    DisplayModeChanged,
    DeskActionRequested,
    DeskActionConfirmed,
    DeskActionUnverified,
    DeskActionFailed,
    DeskActionPermissionRequired,
    ConfigSaved,
    ConfigRecovered,
    ConfigReset,
    ConfigSaveFailed,
    HostSuspending,
    HostResumed,
    HostLocked,
    HostUnlocked,
}

/// Closed connection-state token serialized in activity metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityConnectionStateDto {
    Connecting,
    Connected,
    Incompatible,
    Disconnected,
    Error,
}

/// Closed severity token serialized to the webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivitySeverityDto {
    Info,
    Warning,
    Error,
}

/// Closed source token serialized to the webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivitySourceDto {
    Connection,
    Action,
    Device,
    Protocol,
    Firmware,
    Config,
}

/// Closed outcome token serialized to the webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityOutcomeDto {
    Observed,
    Started,
    Succeeded,
    Failed,
    Rejected,
    Retrying,
    Applied,
    Synchronized,
    Busy,
    TimedOut,
    Available,
    Unavailable,
}

/// Closed safe diagnostic categories projected from the wire protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityDiagnosticCategoryDto {
    Io,
    Handshake,
    Version,
    Framing,
    Checksum,
    Timeout,
    Busy,
    BadPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtocolMalformedCategoryDto {
    Framing,
    Checksum,
    Version,
    Payload,
}

/// Projects application info. `device_studio_enabled` reflects the compiled-in Device Studio feature.
#[must_use]
pub fn app_info(device_studio_enabled: bool) -> AppInfoDto {
    AppInfoDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR).into(),
        supported_majors: vec![PROTOCOL_MAJOR],
        device_studio_enabled,
    }
}

/// Projects the connection snapshot from the manager, orchestrator, and last reported state.
#[must_use]
pub fn connection_status(
    manager: &ConnectionManager,
    orchestrator: &Orchestrator,
    reported: Option<CompanionState>,
    mascot_interaction: bool,
    mascot_action: Option<MascotActionApplied>,
    connection_generation: u32,
    host: HostPresence,
) -> ConnectionStatusDto {
    let device = manager.device().map(|d| DeviceInfoDto {
        firmware_version: format!(
            "{}.{}.{}",
            d.firmware_version.major, d.firmware_version.minor, d.firmware_version.patch
        ),
        protocol_version: d.protocol_version.into(),
        device_id_hash_short: d.device_id_hash_short.clone(),
    });
    ConnectionStatusDto {
        connection: connection_token(manager.state()).to_string(),
        // The user's choice, not the lock-screen override: the UI should not flip to Sleeping.
        desired: sendable_token(orchestrator.user_desired()).to_string(),
        reported: reported.map(|r| companion_token(r).to_string()),
        device,
        incompatible_reason: manager.incompatible_reason().map(str::to_string),
        retry_count: manager.retry_count(),
        connection_generation,
        mascot_interaction: manager.state().can_drive_device() && mascot_interaction,
        mascot_action: mascot_action.map(mascot_action_applied),
        host: host.token(),
    }
}

#[must_use]
pub fn mascot_action_applied(value: MascotActionApplied) -> MascotActionAppliedDto {
    MascotActionAppliedDto {
        action: mascot_action_token(value.action).to_string(),
        personality: mascot_personality_token(value.personality).to_string(),
        seed: value.seed,
        applied_at_ms: value.applied_at_ms,
    }
}

/// Projects a typed activity record to its wire DTO.
#[must_use]
pub fn activity_event(event: &ActivityEvent) -> ActivityEventDto {
    ActivityEventDto {
        id: event.id(),
        at: event.at().to_string(),
        event_type: activity_kind_token(event.kind()),
        summary: event.summary().to_string(),
        severity: activity_severity(event.severity()),
        source: activity_source(event.source()),
        outcome: activity_outcome(event.outcome()),
        metadata: event.metadata().map(activity_metadata),
    }
}

fn activity_kind_token(kind: ActivityEventKind) -> ActivityEventTypeDto {
    match kind {
        ActivityEventKind::ConnectionAttempted => ActivityEventTypeDto::ConnectionAttempted,
        ActivityEventKind::ConnectionOpened => ActivityEventTypeDto::ConnectionOpened,
        ActivityEventKind::HandshakeStarted => ActivityEventTypeDto::HandshakeStarted,
        ActivityEventKind::HandshakeSucceeded => ActivityEventTypeDto::HandshakeSucceeded,
        ActivityEventKind::ConnectionRetryScheduled => {
            ActivityEventTypeDto::ConnectionRetryScheduled
        }
        ActivityEventKind::IncompatibleFirmware => ActivityEventTypeDto::IncompatibleFirmware,
        ActivityEventKind::ConnectionIoFailure => ActivityEventTypeDto::ConnectionIoFailure,
        ActivityEventKind::HandshakeTimedOut => ActivityEventTypeDto::HandshakeTimedOut,
        ActivityEventKind::HeartbeatTimedOut => ActivityEventTypeDto::HeartbeatTimedOut,
        ActivityEventKind::ConnectionRecovered => ActivityEventTypeDto::ConnectionRecovered,
        ActivityEventKind::ConnectionDisconnected => ActivityEventTypeDto::ConnectionDisconnected,
        ActivityEventKind::ConnectionStateChanged => ActivityEventTypeDto::ConnectionStateChanged,
        ActivityEventKind::DeviceNegotiated => ActivityEventTypeDto::DeviceNegotiated,
        ActivityEventKind::PersonalityConfigured => ActivityEventTypeDto::PersonalityConfigured,
        ActivityEventKind::SelfPlayConfigured => ActivityEventTypeDto::SelfPlayConfigured,
        ActivityEventKind::StateRequested => ActivityEventTypeDto::StateRequested,
        ActivityEventKind::MirroredStateRequested => ActivityEventTypeDto::MirroredStateRequested,
        ActivityEventKind::ManualSocialActionRequested => {
            ActivityEventTypeDto::ManualSocialActionRequested
        }
        ActivityEventKind::AutonomousSocialActionRequested => {
            ActivityEventTypeDto::AutonomousSocialActionRequested
        }
        ActivityEventKind::SocialActionApplied => ActivityEventTypeDto::SocialActionApplied,
        ActivityEventKind::StateSynchronized => ActivityEventTypeDto::StateSynchronized,
        ActivityEventKind::DeviceStateObserved => ActivityEventTypeDto::DeviceStateObserved,
        ActivityEventKind::DeviceDiagnosticFraming => ActivityEventTypeDto::DeviceDiagnosticFraming,
        ActivityEventKind::DeviceDiagnosticChecksum => {
            ActivityEventTypeDto::DeviceDiagnosticChecksum
        }
        ActivityEventKind::DeviceDiagnosticVersion => ActivityEventTypeDto::DeviceDiagnosticVersion,
        ActivityEventKind::DeviceDiagnosticPayload => ActivityEventTypeDto::DeviceDiagnosticPayload,
        ActivityEventKind::DeviceSequenceGap => ActivityEventTypeDto::DeviceSequenceGap,
        ActivityEventKind::DeviceDisplayFault => ActivityEventTypeDto::DeviceDisplayFault,
        ActivityEventKind::DeviceLinkLost => ActivityEventTypeDto::DeviceLinkLost,
        ActivityEventKind::DeviceDiagnosticUnknown => ActivityEventTypeDto::DeviceDiagnosticUnknown,
        ActivityEventKind::DeviceError => ActivityEventTypeDto::DeviceError,
        ActivityEventKind::DeviceBusy => ActivityEventTypeDto::DeviceBusy,
        ActivityEventKind::DeviceTimedOut => ActivityEventTypeDto::DeviceTimedOut,
        ActivityEventKind::ProtocolMalformedFrame => ActivityEventTypeDto::ProtocolMalformedFrame,
        ActivityEventKind::ProtocolSequenceGap => ActivityEventTypeDto::ProtocolSequenceGap,
        ActivityEventKind::ActionRequested => ActivityEventTypeDto::ActionRequested,
        ActivityEventKind::ActionCompleted => ActivityEventTypeDto::ActionCompleted,
        ActivityEventKind::ActionFailed => ActivityEventTypeDto::ActionFailed,
        ActivityEventKind::DeviceDiscovered => ActivityEventTypeDto::DeviceDiscovered,
        ActivityEventKind::DeviceRejected => ActivityEventTypeDto::DeviceRejected,
        ActivityEventKind::ProtocolMessageRejected => ActivityEventTypeDto::ProtocolMessageRejected,
        ActivityEventKind::ProtocolFailed => ActivityEventTypeDto::ProtocolFailed,
        ActivityEventKind::FirmwareUpdateStarted => ActivityEventTypeDto::FirmwareUpdateStarted,
        ActivityEventKind::FirmwareUpdateCompleted => ActivityEventTypeDto::FirmwareUpdateCompleted,
        ActivityEventKind::FirmwareUpdateFailed => ActivityEventTypeDto::FirmwareUpdateFailed,
        ActivityEventKind::FirmwareAvailable => ActivityEventTypeDto::FirmwareAvailable,
        ActivityEventKind::FirmwareUnavailable => ActivityEventTypeDto::FirmwareUnavailable,
        ActivityEventKind::FirmwareFlashRequested => ActivityEventTypeDto::FirmwareFlashRequested,
        ActivityEventKind::FirmwarePreparing => ActivityEventTypeDto::FirmwarePreparing,
        ActivityEventKind::FirmwareSerialReleased => ActivityEventTypeDto::FirmwareSerialReleased,
        ActivityEventKind::FirmwareFlasherStarted => ActivityEventTypeDto::FirmwareFlasherStarted,
        ActivityEventKind::FirmwareFlashSucceeded => ActivityEventTypeDto::FirmwareFlashSucceeded,
        ActivityEventKind::FirmwareReconnectWaiting => {
            ActivityEventTypeDto::FirmwareReconnectWaiting
        }
        ActivityEventKind::FirmwareReconnectTimedOut => {
            ActivityEventTypeDto::FirmwareReconnectTimedOut
        }
        ActivityEventKind::FirmwarePostFlashVerified => {
            ActivityEventTypeDto::FirmwarePostFlashVerified
        }
        ActivityEventKind::FirmwarePreparationRejected => {
            ActivityEventTypeDto::FirmwarePreparationRejected
        }
        ActivityEventKind::SessionNonceUnavailable => ActivityEventTypeDto::SessionNonceUnavailable,
        ActivityEventKind::InputStaleSessionRejected => {
            ActivityEventTypeDto::InputStaleSessionRejected
        }
        ActivityEventKind::InputUnstartedGestureRejected => {
            ActivityEventTypeDto::InputUnstartedGestureRejected
        }
        ActivityEventKind::InputStale => ActivityEventTypeDto::InputStale,
        ActivityEventKind::VolumeWriteFailed => ActivityEventTypeDto::VolumeWriteFailed,
        ActivityEventKind::AudioEndpointChanged => ActivityEventTypeDto::AudioEndpointChanged,
        ActivityEventKind::AudioEndpointLost => ActivityEventTypeDto::AudioEndpointLost,
        ActivityEventKind::DisplayModeChanged => ActivityEventTypeDto::DisplayModeChanged,
        ActivityEventKind::DeskActionRequested => ActivityEventTypeDto::DeskActionRequested,
        ActivityEventKind::DeskActionConfirmed => ActivityEventTypeDto::DeskActionConfirmed,
        ActivityEventKind::DeskActionUnverified => ActivityEventTypeDto::DeskActionUnverified,
        ActivityEventKind::DeskActionFailed => ActivityEventTypeDto::DeskActionFailed,
        ActivityEventKind::DeskActionPermissionRequired => {
            ActivityEventTypeDto::DeskActionPermissionRequired
        }
        ActivityEventKind::ConfigSaved => ActivityEventTypeDto::ConfigSaved,
        ActivityEventKind::ConfigRecovered => ActivityEventTypeDto::ConfigRecovered,
        ActivityEventKind::ConfigReset => ActivityEventTypeDto::ConfigReset,
        ActivityEventKind::ConfigSaveFailed => ActivityEventTypeDto::ConfigSaveFailed,
        ActivityEventKind::HostSuspending => ActivityEventTypeDto::HostSuspending,
        ActivityEventKind::HostResumed => ActivityEventTypeDto::HostResumed,
        ActivityEventKind::HostLocked => ActivityEventTypeDto::HostLocked,
        ActivityEventKind::HostUnlocked => ActivityEventTypeDto::HostUnlocked,
    }
}

fn activity_severity(severity: ActivitySeverity) -> ActivitySeverityDto {
    match severity {
        ActivitySeverity::Info => ActivitySeverityDto::Info,
        ActivitySeverity::Warning => ActivitySeverityDto::Warning,
        ActivitySeverity::Error => ActivitySeverityDto::Error,
    }
}

fn activity_source(source: ActivitySource) -> ActivitySourceDto {
    match source {
        ActivitySource::Connection => ActivitySourceDto::Connection,
        ActivitySource::Action => ActivitySourceDto::Action,
        ActivitySource::Device => ActivitySourceDto::Device,
        ActivitySource::Protocol => ActivitySourceDto::Protocol,
        ActivitySource::Firmware => ActivitySourceDto::Firmware,
        ActivitySource::Config => ActivitySourceDto::Config,
    }
}

fn activity_outcome(outcome: ActivityOutcome) -> ActivityOutcomeDto {
    match outcome {
        ActivityOutcome::Observed => ActivityOutcomeDto::Observed,
        ActivityOutcome::Started => ActivityOutcomeDto::Started,
        ActivityOutcome::Succeeded => ActivityOutcomeDto::Succeeded,
        ActivityOutcome::Failed => ActivityOutcomeDto::Failed,
        ActivityOutcome::Rejected => ActivityOutcomeDto::Rejected,
        ActivityOutcome::Retrying => ActivityOutcomeDto::Retrying,
        ActivityOutcome::Applied => ActivityOutcomeDto::Applied,
        ActivityOutcome::Synchronized => ActivityOutcomeDto::Synchronized,
        ActivityOutcome::Busy => ActivityOutcomeDto::Busy,
        ActivityOutcome::TimedOut => ActivityOutcomeDto::TimedOut,
        ActivityOutcome::Available => ActivityOutcomeDto::Available,
        ActivityOutcome::Unavailable => ActivityOutcomeDto::Unavailable,
    }
}

fn activity_metadata(metadata: &ActivityMetadata) -> ActivityMetadataDto {
    match metadata {
        ActivityMetadata::Connection {
            state,
            retry_count,
            elapsed_ms,
        } => ActivityMetadataDto {
            connection: Some(activity_connection_state(*state)),
            retry_count: *retry_count,
            elapsed_ms: *elapsed_ms,
            diagnostic_category: None,
            diagnostic_code: None,
            firmware_version: None,
            protocol_version: None,
            device_id_hash_short: None,
            capabilities: None,
            state: None,
            personality: None,
            self_play: None,
            action: None,
            seed: None,
            applied_at_ms: None,
            autonomous: None,
            protocol_category: None,
            payload_len: None,
            sequence: None,
            skipped: None,
            reported: None,
        },
        ActivityMetadata::DeviceDiagnostic { category, code } => {
            diagnostic_metadata(*category, Some(*code))
        }
        ActivityMetadata::HostDiagnostic { category } => diagnostic_metadata(*category, None),
        ActivityMetadata::DeskAction { action } => ActivityMetadataDto {
            action: Some(desk_action_token(*action).to_string()),
            ..diagnostic_metadata_empty()
        },
        ActivityMetadata::Negotiated {
            firmware_major,
            firmware_minor,
            firmware_patch,
            protocol_major,
            protocol_minor,
            device_id_hash_short,
            capabilities,
        } => ActivityMetadataDto {
            connection: None,
            retry_count: 0,
            elapsed_ms: 0,
            diagnostic_category: None,
            diagnostic_code: None,
            firmware_version: Some(format!(
                "{firmware_major}.{firmware_minor}.{firmware_patch}"
            )),
            protocol_version: Some(ProtocolVersionDto {
                major: *protocol_major,
                minor: *protocol_minor,
            }),
            device_id_hash_short: Some(device_id_hash_short.clone()),
            capabilities: Some(*capabilities),
            state: None,
            personality: None,
            self_play: None,
            action: None,
            seed: None,
            applied_at_ms: None,
            autonomous: None,
            protocol_category: None,
            payload_len: None,
            sequence: None,
            skipped: None,
            reported: None,
        },
        ActivityMetadata::Action {
            state,
            personality,
            self_play,
            action,
            seed,
            applied_at_ms,
            autonomous,
        } => ActivityMetadataDto {
            connection: None,
            retry_count: 0,
            elapsed_ms: 0,
            diagnostic_category: None,
            diagnostic_code: None,
            firmware_version: None,
            protocol_version: None,
            device_id_hash_short: None,
            capabilities: None,
            state: state.map(sendable_token).map(str::to_string),
            personality: personality
                .map(mascot_personality_token)
                .map(str::to_string),
            self_play: *self_play,
            action: action.map(mascot_action_token).map(str::to_string),
            seed: *seed,
            applied_at_ms: *applied_at_ms,
            autonomous: *autonomous,
            protocol_category: None,
            payload_len: None,
            sequence: None,
            skipped: None,
            reported: None,
        },
        ActivityMetadata::ProtocolMalformed {
            category,
            payload_len,
            sequence,
        } => ActivityMetadataDto {
            connection: None,
            retry_count: 0,
            elapsed_ms: 0,
            diagnostic_category: None,
            diagnostic_code: None,
            firmware_version: None,
            protocol_version: None,
            device_id_hash_short: None,
            capabilities: None,
            state: None,
            personality: None,
            self_play: None,
            action: None,
            seed: None,
            applied_at_ms: None,
            autonomous: None,
            protocol_category: Some(match category {
                crate::activity::ProtocolMalformedCategory::Framing => {
                    ProtocolMalformedCategoryDto::Framing
                }
                crate::activity::ProtocolMalformedCategory::Checksum => {
                    ProtocolMalformedCategoryDto::Checksum
                }
                crate::activity::ProtocolMalformedCategory::Version => {
                    ProtocolMalformedCategoryDto::Version
                }
                crate::activity::ProtocolMalformedCategory::Payload => {
                    ProtocolMalformedCategoryDto::Payload
                }
            }),
            payload_len: *payload_len,
            sequence: *sequence,
            skipped: None,
            reported: None,
        },
        ActivityMetadata::ProtocolSequenceGap { skipped } => ActivityMetadataDto {
            connection: None,
            retry_count: 0,
            elapsed_ms: 0,
            diagnostic_category: None,
            diagnostic_code: None,
            firmware_version: None,
            protocol_version: None,
            device_id_hash_short: None,
            capabilities: None,
            state: None,
            personality: None,
            self_play: None,
            action: None,
            seed: None,
            applied_at_ms: None,
            autonomous: None,
            protocol_category: None,
            payload_len: None,
            sequence: None,
            skipped: Some(*skipped),
            reported: None,
        },
        ActivityMetadata::DeviceState { reported } => ActivityMetadataDto {
            connection: None,
            retry_count: 0,
            elapsed_ms: 0,
            diagnostic_category: None,
            diagnostic_code: None,
            firmware_version: None,
            protocol_version: None,
            device_id_hash_short: None,
            capabilities: None,
            state: None,
            personality: None,
            self_play: None,
            action: None,
            seed: None,
            applied_at_ms: None,
            autonomous: None,
            protocol_category: None,
            payload_len: None,
            sequence: None,
            skipped: None,
            reported: Some(companion_token(*reported).to_string()),
        },
    }
}

/// The closed webview token for a desk action.
#[must_use]
pub const fn desk_action_token(action: crate::desk::ActionToken) -> &'static str {
    action.token()
}

/// One catalog entry for the config UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionCatalogEntryDto {
    pub id: &'static str,
    /// `rotate` or `discrete`.
    pub slot: &'static str,
    /// `system`, `app`, `media`, `keyboard`, `launch` or `macro`.
    pub scope: &'static str,
    /// `confirmed`, `started`, `unverified` or `leastOfSteps`.
    pub verification: &'static str,
    /// `none`, `app`, `shortcut`, `shortcutPair`, `target` or `macro`.
    pub params: &'static str,
    /// `available`, `unsupported` (this OS or build cannot) or `unavailable` (not right now).
    pub availability: &'static str,
    /// Why it is not available; `null` when it is.
    pub reason: Option<String>,
    pub runs_when_protected: bool,
}

/// The action catalog for `os`, with availability from `services`.
#[must_use]
pub fn action_catalog(
    os: crate::desk::catalog::Os,
    services: &crate::desk::catalog::Services,
) -> Vec<ActionCatalogEntryDto> {
    use crate::desk::catalog::{Params, Scope, Slot, Verification};
    use crate::platform::ActionAvailability;
    crate::desk::catalog::catalog(os, services)
        .into_iter()
        .map(|(entry, availability)| {
            let (availability, reason) = match availability {
                ActionAvailability::Available { .. } => ("available", None),
                ActionAvailability::Unsupported { reason } => ("unsupported", Some(reason)),
                ActionAvailability::NotImplementedYet { target } => (
                    "unsupported",
                    Some(format!("Not available on {target} yet")),
                ),
                ActionAvailability::RuntimeUnavailable { reason } => ("unavailable", Some(reason)),
                ActionAvailability::Unknown => {
                    ("unavailable", Some("Not available right now".to_string()))
                }
            };
            ActionCatalogEntryDto {
                id: entry.id,
                slot: match entry.slot {
                    Slot::Rotate => "rotate",
                    Slot::Discrete => "discrete",
                },
                scope: match entry.scope {
                    Scope::System => "system",
                    Scope::App => "app",
                    Scope::Media => "media",
                    Scope::Keyboard => "keyboard",
                    Scope::Launch => "launch",
                    Scope::Macro => "macro",
                },
                verification: match entry.verification {
                    Verification::Confirmed => "confirmed",
                    Verification::Started => "started",
                    Verification::Unverified => "unverified",
                    Verification::LeastOfSteps => "leastOfSteps",
                },
                params: match entry.params {
                    Params::None => "none",
                    Params::App => "app",
                    Params::Shortcut => "shortcut",
                    Params::ShortcutPair => "shortcutPair",
                    Params::Target => "target",
                    Params::Macro => "macro",
                },
                availability,
                reason,
                runs_when_protected: entry.runs_when_protected,
            }
        })
        .collect()
}

fn diagnostic_metadata_empty() -> ActivityMetadataDto {
    ActivityMetadataDto {
        diagnostic_category: None,
        ..diagnostic_metadata(kivori_protocol::ErrorCategory::BadPayload, None)
    }
}

fn diagnostic_metadata(
    category: kivori_protocol::ErrorCategory,
    code: Option<u16>,
) -> ActivityMetadataDto {
    ActivityMetadataDto {
        connection: None,
        retry_count: 0,
        elapsed_ms: 0,
        diagnostic_category: Some(activity_diagnostic_category(category)),
        diagnostic_code: code,
        firmware_version: None,
        protocol_version: None,
        device_id_hash_short: None,
        capabilities: None,
        state: None,
        personality: None,
        self_play: None,
        action: None,
        seed: None,
        applied_at_ms: None,
        autonomous: None,
        protocol_category: None,
        payload_len: None,
        sequence: None,
        skipped: None,
        reported: None,
    }
}

fn activity_diagnostic_category(
    category: kivori_protocol::ErrorCategory,
) -> ActivityDiagnosticCategoryDto {
    match category {
        kivori_protocol::ErrorCategory::Io => ActivityDiagnosticCategoryDto::Io,
        kivori_protocol::ErrorCategory::Handshake => ActivityDiagnosticCategoryDto::Handshake,
        kivori_protocol::ErrorCategory::Version => ActivityDiagnosticCategoryDto::Version,
        kivori_protocol::ErrorCategory::Framing => ActivityDiagnosticCategoryDto::Framing,
        kivori_protocol::ErrorCategory::Checksum => ActivityDiagnosticCategoryDto::Checksum,
        kivori_protocol::ErrorCategory::Timeout => ActivityDiagnosticCategoryDto::Timeout,
        kivori_protocol::ErrorCategory::Busy => ActivityDiagnosticCategoryDto::Busy,
        kivori_protocol::ErrorCategory::BadPayload => ActivityDiagnosticCategoryDto::BadPayload,
    }
}

fn activity_connection_state(state: ConnectionState) -> ActivityConnectionStateDto {
    match state {
        ConnectionState::Connecting => ActivityConnectionStateDto::Connecting,
        ConnectionState::Connected => ActivityConnectionStateDto::Connected,
        ConnectionState::Incompatible => ActivityConnectionStateDto::Incompatible,
        ConnectionState::Disconnected => ActivityConnectionStateDto::Disconnected,
        ConnectionState::Error => ActivityConnectionStateDto::Error,
    }
}

/// Parses a lowercase sendable token into a [`SendableState`]. Rejects `booting`/`offline` and any
/// unknown token, enforcing the sendable boundary at the IPC edge (FR-014/015).
#[must_use]
pub fn sendable_from_token(token: &str) -> Option<SendableState> {
    match token {
        "idle" => Some(SendableState::Idle),
        "happy" => Some(SendableState::Happy),
        "busy" => Some(SendableState::Busy),
        "sleeping" => Some(SendableState::Sleeping),
        _ => None,
    }
}

/// Parses a lowercase companion token into a [`CompanionState`] (all six; preview use only).
#[must_use]
pub fn companion_from_token(token: &str) -> Option<CompanionState> {
    match token {
        "booting" => Some(CompanionState::Booting),
        "idle" => Some(CompanionState::Idle),
        "happy" => Some(CompanionState::Happy),
        "busy" => Some(CompanionState::Busy),
        "sleeping" => Some(CompanionState::Sleeping),
        "offline" => Some(CompanionState::Offline),
        _ => None,
    }
}

/// Parses a lowercase direct social-action token.
#[must_use]
pub fn mascot_action_from_token(token: &str) -> Option<MascotAction> {
    match token {
        "greet" => Some(MascotAction::Greet),
        "pet" => Some(MascotAction::Pet),
        "tickle" => Some(MascotAction::Tickle),
        "surprise" => Some(MascotAction::Surprise),
        "comfort" => Some(MascotAction::Comfort),
        _ => None,
    }
}

/// Parses a lowercase mascot-personality token.
#[must_use]
pub fn mascot_personality_from_token(token: &str) -> Option<MascotPersonality> {
    match token {
        "cozy" => Some(MascotPersonality::Cozy),
        "playful" => Some(MascotPersonality::Playful),
        "calm" => Some(MascotPersonality::Calm),
        _ => None,
    }
}

/// The initial (pre-connection) snapshot: `disconnected` / desired `idle`, nothing reported.
#[must_use]
pub fn initial_status() -> ConnectionStatusDto {
    connection_status(
        &ConnectionManager::new(),
        &Orchestrator::new(),
        None,
        false,
        None,
        0,
        HostPresence::Active,
    )
}

/// The desk projection the UI shows: display mode, monitored values and the last action outcome.
/// Unknown values are `null`, never guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeskStatusDto {
    /// `buddy`, `clock`, `volume`, `media` or `system`.
    pub mode: &'static str,
    pub volume_percent: Option<u8>,
    pub muted: Option<bool>,
    /// `playing`, `paused` or `stopped`; `null` when playback cannot be observed on this OS.
    pub media: Option<&'static str>,
    pub cpu_percent: Option<u8>,
    pub ram_percent: Option<u8>,
    pub high_load: bool,
    /// Desk action tokens bound to Press and Hold (`null` = unbound).
    pub press_action: Option<&'static str>,
    pub hold_action: Option<&'static str>,
    /// Always `nextView` in M1: a double press shows the next display mode.
    pub double_press_action: &'static str,
    /// Desk action tokens bound to the three contextual buttons' Press (`null` = unbound).
    pub button_actions: [Option<&'static str>; 3],
    /// Desk action tokens bound to the three contextual buttons' Hold (`null` = unbound; the
    /// middle one is always `null`, it pins the profile).
    pub button_hold_actions: [Option<&'static str>; 3],
    /// The active profile's id token (what the config UI keys its tabs by).
    pub profile_id: &'static str,
    /// The active profile's name, exactly as the device shows it (`null` = the General fallback).
    pub profile: Option<String>,
    /// The profile was pinned from the device instead of following the focused app.
    pub pinned: bool,
    /// What the knob does (empty = suspended).
    pub rotate_label: String,
    /// What each contextual button does, as the device labels it (empty = unbound or suspended).
    pub button_labels: [String; 3],
    /// Now playing, when observable (`null` = unknown; empty string = the player gave none).
    pub media_title: Option<String>,
    pub media_artist: Option<String>,
    pub last_action: Option<DeskActionDto>,
}

/// One action outcome for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeskActionDto {
    /// A desk action token (see [`desk_action_token`]).
    pub action: &'static str,
    /// `processing`, `stateConfirmed`, `executionConfirmed`, `unverified` or `error`.
    pub result: &'static str,
    /// The OS needs a permission first (macOS Accessibility).
    pub permission_required: bool,
}

/// The webview token for a display mode.
#[must_use]
pub const fn display_mode_token(mode: kivori_model::desk::DisplayMode) -> &'static str {
    use kivori_model::desk::DisplayMode;
    match mode {
        DisplayMode::Buddy => "buddy",
        DisplayMode::Clock => "clock",
        DisplayMode::Volume => "volume",
        DisplayMode::Media => "media",
        DisplayMode::System => "system",
    }
}

/// Parses a display-mode token from the webview.
#[must_use]
pub fn display_mode_from_token(token: &str) -> Option<kivori_model::desk::DisplayMode> {
    kivori_model::desk::DisplayMode::ALL
        .into_iter()
        .find(|mode| display_mode_token(*mode) == token)
}

const fn feedback_token(kind: kivori_model::desk::FeedbackKind) -> &'static str {
    use kivori_model::desk::FeedbackKind;
    match kind {
        FeedbackKind::Processing => "processing",
        FeedbackKind::StateConfirmed => "stateConfirmed",
        FeedbackKind::ExecutionConfirmed => "executionConfirmed",
        FeedbackKind::Unverified => "unverified",
        FeedbackKind::Error => "error",
    }
}

const fn media_token(media: kivori_model::desk::MediaStatus) -> &'static str {
    use kivori_model::desk::MediaStatus;
    match media {
        MediaStatus::Playing => "playing",
        MediaStatus::Paused => "paused",
        MediaStatus::Stopped => "stopped",
    }
}

fn slot_token(slot: &crate::desk::Slot) -> Option<&'static str> {
    slot.action.as_ref().map(|a| desk_action_token(a.token()))
}

fn button_tokens(bindings: &crate::desk::Bindings) -> [Option<&'static str>; 3] {
    bindings.buttons.each_ref().map(|b| slot_token(&b.press))
}

fn button_hold_tokens(bindings: &crate::desk::Bindings) -> [Option<&'static str>; 3] {
    bindings.buttons.each_ref().map(|b| slot_token(&b.hold))
}

fn text(text: kivori_model::desk::MediaText) -> String {
    text.as_latin1().iter().map(|&b| char::from(b)).collect()
}

/// The desk projection before the device thread has observed anything.
#[must_use]
pub fn initial_desk_status(config: &crate::config::ResolvedConfig) -> DeskStatusDto {
    let context = crate::desk::profile::Context::new(config.profiles.clone());
    let bindings = &context.profile().bindings;
    let labels = context.labels();
    DeskStatusDto {
        mode: display_mode_token(config.display.default_view.mode()),
        volume_percent: None,
        muted: None,
        media: None,
        cpu_percent: None,
        ram_percent: None,
        high_load: false,
        press_action: slot_token(&bindings.press),
        hold_action: slot_token(&bindings.hold),
        double_press_action: "nextView",
        button_actions: button_tokens(bindings),
        button_hold_actions: button_hold_tokens(bindings),
        profile_id: context.profile().id.token(),
        profile: None,
        pinned: false,
        rotate_label: text(labels.rotate),
        button_labels: labels.buttons.map(text),
        media_title: None,
        media_artist: None,
        last_action: None,
    }
}

/// Projects the device task's desk runtime for the UI.
#[must_use]
pub fn desk_status_dto(desk: &crate::desk::DeskRuntime) -> DeskStatusDto {
    let observed = desk.observed();
    let labels = desk.labels();
    DeskStatusDto {
        mode: display_mode_token(desk.mode()),
        volume_percent: observed.volume_percent,
        muted: observed.muted,
        media: observed.media.map(media_token),
        cpu_percent: observed.system.cpu_percent,
        ram_percent: observed.system.ram_percent,
        high_load: observed.system.high_load,
        press_action: slot_token(&desk.bindings().press),
        hold_action: slot_token(&desk.bindings().hold),
        double_press_action: "nextView",
        button_actions: button_tokens(desk.bindings()),
        button_hold_actions: button_hold_tokens(desk.bindings()),
        profile_id: desk.context().profile().id.token(),
        profile: Some(text(labels.profile)).filter(|name| !name.is_empty()),
        pinned: labels.pinned,
        rotate_label: text(labels.rotate),
        button_labels: labels.buttons.map(text),
        media_title: desk.now_playing().map(|np| np.title.clone()),
        media_artist: desk.now_playing().map(|np| np.artist.clone()),
        last_action: desk.last_action().map(|last| DeskActionDto {
            action: desk_action_token(last.action),
            result: feedback_token(last.kind),
            permission_required: last.permission_required,
        }),
    }
}

/// The user's settings as the UI shows them. Never carries a file path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDto {
    pub version: u32,
    /// Counts saves and resets in this run, so the UI can tell a stale copy from a fresh one.
    pub revision: u64,
    /// `recoveredCorrupt`, `recoveredNewerVersion` or `migrated`; `null` = nothing to report.
    pub notice: Option<&'static str>,
    /// Every built-in profile as resolved (built-ins with the user's overrides), General first.
    pub profiles: Vec<ProfileConfigDto>,
    /// The user's macros, in the order they were created.
    pub macros: Vec<crate::config::MacroSpec>,
    pub display: DisplaySettingsDto,
    pub buddy: BuddySettingsDto,
}

/// One profile as the config UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileConfigDto {
    /// `general`, `browser`, `code`, `media`, `zoom` or `teams`.
    pub id: &'static str,
    pub name: String,
    /// The foreground app ids that select this profile (empty for General).
    pub apps: Vec<String>,
    pub rotate: RotateDto,
    pub press: SlotDto,
    pub hold: SlotDto,
    pub buttons: [ButtonDto; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotateDto {
    pub spec: crate::config::RotateSpec,
    /// What the device shows for the knob.
    pub device_label: String,
    /// The user changed it from the built-in.
    pub overridden: bool,
}

/// One bindable gesture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotDto {
    /// `null` = unbound.
    pub action: Option<crate::config::ActionSpec>,
    /// The custom label, `null` = the action's own.
    pub label: Option<String>,
    /// What the device shows (empty = unbound).
    pub device_label: String,
    pub overridden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonDto {
    pub press: SlotDto,
    pub hold: HoldDto,
}

/// A button's Hold: a slot, or `"pin"` for the middle button (reserved for profile pin).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum HoldDto {
    Pin(&'static str),
    Slot(SlotDto),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettingsDto {
    /// A display-mode token.
    pub default_view: &'static str,
    /// A display-mode token, or `cycle` for the M1 behaviour (every view in turn).
    pub secondary_view: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuddySettingsDto {
    pub reactions: bool,
    /// `low`, `normal` or `high`.
    pub intensity: &'static str,
}

fn config_notice_token(notice: crate::config::ConfigNotice) -> &'static str {
    use crate::config::ConfigNotice;
    match notice {
        ConfigNotice::RecoveredCorrupt => "recoveredCorrupt",
        ConfigNotice::RecoveredNewerVersion => "recoveredNewerVersion",
        ConfigNotice::Migrated => "migrated",
    }
}

/// The Rust-side vocabulary for [`crate::config::Intensity`].
#[must_use]
pub const fn intensity_token(intensity: crate::config::Intensity) -> &'static str {
    use crate::config::Intensity;
    match intensity {
        Intensity::Low => "low",
        Intensity::Normal => "normal",
        Intensity::High => "high",
    }
}

/// Parses an intensity token from the webview.
#[must_use]
pub fn intensity_from_token(token: &str) -> Option<crate::config::Intensity> {
    use crate::config::Intensity;
    [Intensity::Low, Intensity::Normal, Intensity::High]
        .into_iter()
        .find(|intensity| intensity_token(*intensity) == token)
}

/// Parses a `defaultView`/`secondaryView` pair from the webview. `secondary` may be `cycle`.
///
/// # Errors
/// A short reason naming the bad field or rule; never echoes the input.
pub fn display_settings_from_tokens(
    default_view: &str,
    secondary_view: &str,
) -> Result<crate::config::DisplaySettings, &'static str> {
    use crate::config::{DisplaySettings, SecondaryView, View};
    let default_view = display_mode_from_token(default_view)
        .map(View::from_mode)
        .ok_or("unknown default view")?;
    let secondary_view = if secondary_view == "cycle" {
        SecondaryView::Cycle
    } else {
        SecondaryView::View(
            display_mode_from_token(secondary_view)
                .map(View::from_mode)
                .ok_or("unknown double-press view")?,
        )
    };
    let settings = DisplaySettings {
        default_view,
        secondary_view,
    };
    settings.validate()?;
    Ok(settings)
}

/// Projects the stored config for the UI.
#[must_use]
pub fn config_dto(store: &crate::config::ConfigStore) -> ConfigDto {
    use crate::config::SecondaryView;
    let file = store.file();
    ConfigDto {
        version: file.version,
        revision: store.revision(),
        notice: store.notice().map(config_notice_token),
        profiles: profile_dtos(store),
        macros: file.macros.clone(),
        display: DisplaySettingsDto {
            default_view: display_mode_token(file.display.default_view.mode()),
            secondary_view: match file.display.secondary_view {
                SecondaryView::Cycle => "cycle",
                SecondaryView::View(view) => display_mode_token(view.mode()),
            },
        },
        buddy: BuddySettingsDto {
            reactions: file.buddy.reactions,
            intensity: intensity_token(file.buddy.intensity),
        },
    }
}

fn slot_dto(slot: &crate::desk::Slot, overridden: bool) -> SlotDto {
    SlotDto {
        action: slot
            .action
            .as_ref()
            .and_then(crate::config::resolve::action_spec),
        label: slot.label.clone(),
        device_label: slot.device_label(),
        overridden,
    }
}

fn profile_dtos(store: &crate::config::ConfigStore) -> Vec<ProfileConfigDto> {
    let resolved = store.resolved();
    resolved
        .profiles
        .iter()
        .map(|profile| {
            let over = store
                .file()
                .profiles
                .get(&profile.id)
                .cloned()
                .unwrap_or_default();
            let b = &profile.bindings;
            let button = |i: usize| ButtonDto {
                press: slot_dto(&b.buttons[i].press, over.buttons[i].press.is_some()),
                hold: if i == usize::from(crate::desk::profile::PIN_BUTTON) {
                    HoldDto::Pin("pin")
                } else {
                    HoldDto::Slot(slot_dto(&b.buttons[i].hold, over.buttons[i].hold.is_some()))
                },
            };
            ProfileConfigDto {
                id: profile.id.token(),
                name: profile.name.clone(),
                apps: profile.ids.clone(),
                rotate: RotateDto {
                    spec: crate::config::resolve::rotate_spec(&profile.rotate),
                    device_label: profile.rotate.label().to_string(),
                    overridden: over.rotate.is_some(),
                },
                press: slot_dto(&b.press, over.press.is_some()),
                hold: slot_dto(&b.hold, over.hold.is_some()),
                buttons: [button(0), button(1), button(2)],
            }
        })
        .collect()
}

/// `available`, `unsupported` or `unavailable` for a host service's availability.
#[must_use]
pub const fn availability_token(
    availability: &crate::platform::ActionAvailability,
) -> &'static str {
    use crate::platform::ActionAvailability;
    match availability {
        ActionAvailability::Available { .. } => "available",
        ActionAvailability::Unsupported { .. } | ActionAvailability::NotImplementedYet { .. } => {
            "unsupported"
        }
        ActionAvailability::RuntimeUnavailable { .. } | ActionAvailability::Unknown => {
            "unavailable"
        }
    }
}

/// Every capability flag with the name the diagnostics page lists it by, in bit order.
const CAPABILITY_TABLE: [(Capabilities, &str); 11] = [
    (Capabilities::MASCOT_INTERACTION, "mascotInteraction"),
    (Capabilities::PHYSICAL_INPUT_V1, "physicalInputV1"),
    (Capabilities::PRESENTATION_V1, "presentationV1"),
    (Capabilities::BUTTON_INPUT_V1, "buttonInputV1"),
    (Capabilities::DESK_STATUS_V1, "deskStatusV1"),
    (Capabilities::ACTION_FEEDBACK_V1, "actionFeedbackV1"),
    (Capabilities::DOUBLE_PRESS_V1, "doublePressV1"),
    (Capabilities::MEDIA_INFO_V1, "mediaInfoV1"),
    (Capabilities::CONTROL_LABELS_V1, "controlLabelsV1"),
    (Capabilities::CONTEXT_BUTTONS_V1, "contextButtonsV1"),
    (Capabilities::HOST_TAKEOVERS_V1, "hostTakeoversV1"),
];

/// The capability names the diagnostics page lists, in bit order.
#[must_use]
pub fn capability_names(caps: Capabilities) -> Vec<&'static str> {
    CAPABILITY_TABLE
        .into_iter()
        .filter(|(flag, _)| caps.contains(*flag))
        .map(|(_, name)| name)
        .collect()
}

/// Host-service tokens in the diagnostics DTO (closed lists, shared with the webview).
pub const HOST_AVAILABILITY_TOKENS: [&str; 3] = ["available", "unsupported", "unavailable"];
pub const HOST_MEDIA_TOKENS: [&str; 2] = ["observable", "notObservable"];
pub const HOST_FOCUS_TOKENS: [&str; 2] = ["detecting", "unknown"];
pub const HOST_INPUT_PERMISSION_TOKENS: [&str; 3] = ["required", "notNeeded", "unknown"];

/// What the host services report, as short tokens. Written by the device thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostServicesSnapshot {
    /// `available`, `unsupported` or `unavailable`.
    pub system_volume: &'static str,
    /// `available`, `unsupported` or `unavailable`.
    pub app_volume: &'static str,
    /// `observable` or `notObservable`.
    pub media: &'static str,
    /// `detecting` (a focused window is seen) or `unknown`.
    pub focus: &'static str,
    /// `required` (the OS refused synthesized input), `notNeeded` or `unknown`.
    pub input_permission: &'static str,
}

/// The device thread's raw figures for the diagnostics page. Ages are worked out when the page
/// asks, so the snapshot only has to be refreshed about once a second.
#[derive(Debug, Clone)]
pub struct DiagnosticsSnapshot {
    /// The connection state token.
    pub connection: &'static str,
    /// The device's firmware version, while connected.
    pub firmware_version: Option<String>,
    /// The short, non-reversible device hash, while connected.
    pub device_hash: Option<String>,
    /// The capability set negotiated for this connection.
    pub negotiated_caps: Capabilities,
    /// The protocol minor both sides settled on, while connected.
    pub negotiated_minor: Option<u16>,
    /// When the current connection came up.
    pub connected_since: Option<Instant>,
    /// Times the device came back after the first connection of this run.
    pub reconnects: u32,
    /// Consecutive reconnect attempts.
    pub retry_count: u32,
    /// Health and link-quality figures from the session.
    pub session: SessionDiagnostics,
    /// Host service status.
    pub host: HostServicesSnapshot,
}

impl DiagnosticsSnapshot {
    /// Before the device thread has published anything.
    #[must_use]
    pub fn initial() -> Self {
        Self {
            connection: connection_token(ConnectionState::Disconnected),
            firmware_version: None,
            device_hash: None,
            negotiated_caps: Capabilities::NONE,
            negotiated_minor: None,
            connected_since: None,
            reconnects: 0,
            retry_count: 0,
            session: SessionDiagnostics::default(),
            host: HostServicesSnapshot {
                system_volume: "unavailable",
                app_volume: "unavailable",
                media: "notObservable",
                focus: "unknown",
                input_permission: "unknown",
            },
        }
    }
}

/// Everything the diagnostics page shows, and what "Copy diagnostics" copies. Unknown values are
/// `null`, never guessed. Carries no port, path or raw device id: the device appears only as its
/// short hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsDto {
    pub versions: VersionsDto,
    pub connection: ConnectionDiagnosticsDto,
    pub health: HealthDto,
    pub host: HostServicesDto,
    pub config: ConfigDiagnosticsDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionsDto {
    pub app: String,
    /// The connected device's firmware version.
    pub firmware: Option<String>,
    /// This build's protocol version, `major.minor`.
    pub protocol: String,
    pub negotiated_minor: Option<u16>,
    pub capabilities: Vec<&'static str>,
    pub device_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDiagnosticsDto {
    pub state: &'static str,
    pub connected_for_secs: Option<u64>,
    pub reconnects: u32,
    pub retry_count: u32,
    pub last_pong_age_ms: Option<u64>,
    pub rtt_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDto {
    /// The device's uptime, estimated forward from its last `Pong`.
    pub device_uptime_ms: Option<u64>,
    pub free_bytes: Option<u32>,
    pub malformed_frames: u32,
    pub sequence_gaps: u32,
    pub device_errors: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostServicesDto {
    pub system_volume: &'static str,
    pub app_volume: &'static str,
    pub media: &'static str,
    pub focus: &'static str,
    pub input_permission: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDiagnosticsDto {
    /// `ok`, or the config notice token (`recoveredCorrupt`, `recoveredNewerVersion`, `migrated`).
    pub status: &'static str,
    pub schema_version: u32,
    /// Controls the user rebound (rotate, press, hold and button overrides across profiles).
    pub custom_bindings: u32,
    pub macros: u32,
}

fn custom_bindings(file: &crate::config::ConfigFile) -> u32 {
    let count = file
        .profiles
        .values()
        .map(|over| {
            usize::from(over.rotate.is_some())
                + usize::from(over.press.is_some())
                + usize::from(over.hold.is_some())
                + over
                    .buttons
                    .iter()
                    .map(|b| usize::from(b.press.is_some()) + usize::from(b.hold.is_some()))
                    .sum::<usize>()
        })
        .sum::<usize>();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Projects the diagnostics page at time `now`.
#[must_use]
pub fn diagnostics_dto(
    snapshot: &DiagnosticsSnapshot,
    store: &crate::config::ConfigStore,
    now: Instant,
) -> DiagnosticsDto {
    let session = snapshot.session;
    let pong_age = session
        .last_pong
        .map(|(at, _)| now.saturating_duration_since(at));
    let file = store.file();
    DiagnosticsDto {
        versions: VersionsDto {
            app: env!("CARGO_PKG_VERSION").to_string(),
            firmware: snapshot.firmware_version.clone(),
            protocol: format!("{PROTOCOL_MAJOR}.{PROTOCOL_MINOR}"),
            negotiated_minor: snapshot.negotiated_minor,
            capabilities: capability_names(snapshot.negotiated_caps),
            device_hash: snapshot.device_hash.clone(),
        },
        connection: ConnectionDiagnosticsDto {
            state: snapshot.connection,
            connected_for_secs: snapshot
                .connected_since
                .map(|since| now.saturating_duration_since(since).as_secs()),
            reconnects: snapshot.reconnects,
            retry_count: snapshot.retry_count,
            last_pong_age_ms: pong_age
                .map(|age| u64::try_from(age.as_millis()).unwrap_or(u64::MAX)),
            rtt_ms: session.rtt_ms,
        },
        health: HealthDto {
            device_uptime_ms: session.last_pong.zip(pong_age).map(|((_, uptime), age)| {
                u64::from(uptime).saturating_add(u64::try_from(age.as_millis()).unwrap_or(0))
            }),
            free_bytes: session.free_bytes,
            malformed_frames: session.malformed_frames,
            sequence_gaps: session.sequence_gaps,
            device_errors: session.device_errors,
        },
        host: HostServicesDto {
            system_volume: snapshot.host.system_volume,
            app_volume: snapshot.host.app_volume,
            media: snapshot.host.media,
            focus: snapshot.host.focus,
            input_permission: snapshot.host.input_permission,
        },
        config: ConfigDiagnosticsDto {
            status: store.notice().map_or("ok", config_notice_token),
            schema_version: file.version,
            custom_bindings: custom_bindings(file),
            macros: u32::try_from(file.macros.len()).unwrap_or(u32::MAX),
        },
    }
}

/// The closed `ProfileId` and `Control` vocabularies, for the Rust/TS drift check.
#[must_use]
pub fn vocabulary_json() -> serde_json::Value {
    serde_json::json!({
        "profileIds": crate::config::ProfileId::ALL.map(crate::config::ProfileId::token),
        "controls": crate::config::resolve::CONTROL_TOKENS,
        "actionSpecKinds": crate::config::resolve::ACTION_SPEC_KINDS,
        "stepSpecKinds": crate::config::resolve::STEP_SPEC_KINDS,
        "rotateSpecKinds": crate::config::resolve::ROTATE_SPEC_KINDS,
        "deskActions": crate::desk::ActionToken::ALL.map(crate::desk::ActionToken::token),
        "capabilityNames": CAPABILITY_TABLE.map(|(_, name)| name),
        "hostAvailability": HOST_AVAILABILITY_TOKENS,
        "hostMedia": HOST_MEDIA_TOKENS,
        "hostFocus": HOST_FOCUS_TOKENS,
        "hostInputPermission": HOST_INPUT_PERMISSION_TOKENS,
        "hostPresence": HOST_PRESENCE_TOKENS,
        "flashFailures": crate::firmware::FlashFailure::ALL.map(crate::firmware::FlashFailure::token),
        "updateAdvice": crate::firmware::UpdateAdvice::ALL.map(crate::firmware::UpdateAdvice::token),
        "startupPlatform": STARTUP_PLATFORM_TOKENS,
    })
}
