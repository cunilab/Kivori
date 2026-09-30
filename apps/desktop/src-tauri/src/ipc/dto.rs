//! IPC DTOs and pure projections (contracts/ipc.md §4).
//!
//! These are the ONLY shapes that cross to the webview. They carry no raw device id, payload bytes,
//! serial handles, or paths. All enum tokens are the lowercase wire strings the frontend expects; the
//! projections are pure functions of the internal state, so they are unit-testable without Tauri.

use kivori_model::{
    CompanionState, ConnectionState, MascotAction, MascotPersonality, ProtocolVersion,
    SendableState,
};
use kivori_protocol::{MascotActionApplied, PROTOCOL_MAJOR, PROTOCOL_MINOR};
use serde::Serialize;

use crate::activity::{
    ActivityEvent, ActivityEventKind, ActivityMetadata, ActivityOutcome, ActivitySeverity,
    ActivitySource,
};
use crate::device::fsm::ConnectionManager;
use crate::orchestrator::Orchestrator;

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
    VolumeWriteFailed,
    AudioEndpointChanged,
    AudioEndpointLost,
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
        desired: sendable_token(orchestrator.desired()).to_string(),
        reported: reported.map(|r| companion_token(r).to_string()),
        device,
        incompatible_reason: manager.incompatible_reason().map(str::to_string),
        retry_count: manager.retry_count(),
        connection_generation,
        mascot_interaction: manager.state().can_drive_device() && mascot_interaction,
        mascot_action: mascot_action.map(mascot_action_applied),
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
        ActivityEventKind::VolumeWriteFailed => ActivityEventTypeDto::VolumeWriteFailed,
        ActivityEventKind::AudioEndpointChanged => ActivityEventTypeDto::AudioEndpointChanged,
        ActivityEventKind::AudioEndpointLost => ActivityEventTypeDto::AudioEndpointLost,
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
    )
}
