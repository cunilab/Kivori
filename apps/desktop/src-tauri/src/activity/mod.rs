//! Session-only, typed activity history for the desktop UI.
//!
//! The log stores only closed event kinds and typed, allowlisted metadata. It has no persistence
//! path and does not accept caller-provided text, so UI-facing summaries are always generated from
//! trusted event data.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use kivori_model::{ConnectionState, MascotAction, MascotPersonality, SendableState};

static NEXT_ACTIVITY_ID: AtomicU64 = AtomicU64::new(1);

/// The closed set of native activity event kinds currently emitted by the desktop core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityEventKind {
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
    /// The device connection manager entered a new lifecycle state.
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
    /// A desktop-owned mascot action was requested.
    ActionRequested,
    /// A mascot action completed successfully.
    ActionCompleted,
    /// A mascot action could not be completed.
    ActionFailed,
    /// A device was observed by native discovery.
    DeviceDiscovered,
    /// A device-side operation was rejected.
    DeviceRejected,
    /// A protocol message was rejected before it could be applied.
    ProtocolMessageRejected,
    /// A protocol operation failed.
    ProtocolFailed,
    /// A firmware update began.
    FirmwareUpdateStarted,
    /// A firmware update completed successfully.
    FirmwareUpdateCompleted,
    /// A firmware update failed.
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
    /// The OS could not supply a fresh session nonce; this connection attempt was aborted.
    SessionNonceUnavailable,
    /// A rotary `InputEvent` arrived outside the current session and was dropped unexecuted.
    InputStaleSessionRejected,
    /// A rotary `InputEvent` referenced a gesture never started in this session and was dropped.
    InputUnstartedGestureRejected,
    /// A master-volume write was attempted and is known to have failed.
    VolumeWriteFailed,
    /// The default audio render endpoint changed; volume was re-read from the new endpoint.
    AudioEndpointChanged,
    /// No default audio render endpoint is available any more.
    AudioEndpointLost,
}

/// Closed severity vocabulary for native activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivitySeverity {
    Info,
    Warning,
    Error,
}

/// Closed source vocabulary for native activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivitySource {
    Connection,
    Action,
    Device,
    Protocol,
    Firmware,
}

/// Closed outcome vocabulary for native activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityOutcome {
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

#[derive(Debug, Clone, Copy)]
struct ActivityClassification {
    severity: ActivitySeverity,
    source: ActivitySource,
    outcome: ActivityOutcome,
}

impl ActivityEventKind {
    const fn classification(self) -> ActivityClassification {
        match self {
            Self::ConnectionAttempted | Self::ConnectionOpened | Self::HandshakeStarted => {
                ActivityClassification {
                    severity: ActivitySeverity::Info,
                    source: ActivitySource::Connection,
                    outcome: ActivityOutcome::Started,
                }
            }
            Self::HandshakeSucceeded | Self::ConnectionRecovered => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Connection,
                outcome: ActivityOutcome::Succeeded,
            },
            Self::ConnectionRetryScheduled => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Connection,
                outcome: ActivityOutcome::Retrying,
            },
            Self::IncompatibleFirmware => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Connection,
                outcome: ActivityOutcome::Rejected,
            },
            Self::ConnectionIoFailure | Self::ConnectionDisconnected => ActivityClassification {
                severity: ActivitySeverity::Error,
                source: ActivitySource::Connection,
                outcome: ActivityOutcome::Failed,
            },
            Self::HandshakeTimedOut | Self::HeartbeatTimedOut => ActivityClassification {
                severity: ActivitySeverity::Error,
                source: ActivitySource::Connection,
                outcome: ActivityOutcome::TimedOut,
            },
            Self::ConnectionStateChanged => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Connection,
                outcome: ActivityOutcome::Observed,
            },
            Self::DeviceNegotiated => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Succeeded,
            },
            Self::PersonalityConfigured
            | Self::SelfPlayConfigured
            | Self::StateRequested
            | Self::MirroredStateRequested
            | Self::ManualSocialActionRequested
            | Self::AutonomousSocialActionRequested => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Action,
                outcome: ActivityOutcome::Started,
            },
            Self::SocialActionApplied => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Applied,
            },
            Self::StateSynchronized => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Synchronized,
            },
            Self::DeviceStateObserved => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Observed,
            },
            Self::DeviceDiagnosticFraming
            | Self::DeviceDiagnosticChecksum
            | Self::DeviceDiagnosticVersion
            | Self::DeviceDiagnosticPayload
            | Self::DeviceSequenceGap
            | Self::DeviceDisplayFault
            | Self::DeviceLinkLost
            | Self::DeviceDiagnosticUnknown => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Observed,
            },
            Self::DeviceError => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Rejected,
            },
            Self::DeviceBusy => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Busy,
            },
            Self::DeviceTimedOut => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::TimedOut,
            },
            Self::ProtocolMalformedFrame | Self::ProtocolSequenceGap => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Protocol,
                outcome: ActivityOutcome::Rejected,
            },
            Self::ActionRequested => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Action,
                outcome: ActivityOutcome::Started,
            },
            Self::ActionCompleted => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Action,
                outcome: ActivityOutcome::Succeeded,
            },
            Self::ActionFailed => ActivityClassification {
                severity: ActivitySeverity::Error,
                source: ActivitySource::Action,
                outcome: ActivityOutcome::Failed,
            },
            Self::DeviceDiscovered => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Observed,
            },
            Self::DeviceRejected => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Device,
                outcome: ActivityOutcome::Rejected,
            },
            Self::ProtocolMessageRejected => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Protocol,
                outcome: ActivityOutcome::Rejected,
            },
            Self::ProtocolFailed => ActivityClassification {
                severity: ActivitySeverity::Error,
                source: ActivitySource::Protocol,
                outcome: ActivityOutcome::Failed,
            },
            Self::FirmwareUpdateStarted => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Firmware,
                outcome: ActivityOutcome::Started,
            },
            Self::FirmwareUpdateCompleted => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Firmware,
                outcome: ActivityOutcome::Succeeded,
            },
            Self::FirmwareUpdateFailed => ActivityClassification {
                severity: ActivitySeverity::Error,
                source: ActivitySource::Firmware,
                outcome: ActivityOutcome::Failed,
            },
            Self::FirmwareAvailable => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Firmware,
                outcome: ActivityOutcome::Available,
            },
            Self::FirmwareUnavailable => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Firmware,
                outcome: ActivityOutcome::Unavailable,
            },
            Self::FirmwareFlashRequested
            | Self::FirmwarePreparing
            | Self::FirmwareSerialReleased
            | Self::FirmwareFlasherStarted
            | Self::FirmwareReconnectWaiting => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Firmware,
                outcome: ActivityOutcome::Started,
            },
            Self::FirmwareFlashSucceeded | Self::FirmwarePostFlashVerified => {
                ActivityClassification {
                    severity: ActivitySeverity::Info,
                    source: ActivitySource::Firmware,
                    outcome: ActivityOutcome::Succeeded,
                }
            }
            Self::FirmwareReconnectTimedOut => ActivityClassification {
                severity: ActivitySeverity::Error,
                source: ActivitySource::Firmware,
                outcome: ActivityOutcome::TimedOut,
            },
            Self::FirmwarePreparationRejected => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Firmware,
                outcome: ActivityOutcome::Rejected,
            },
            Self::SessionNonceUnavailable => ActivityClassification {
                severity: ActivitySeverity::Error,
                source: ActivitySource::Connection,
                outcome: ActivityOutcome::Failed,
            },
            Self::InputStaleSessionRejected | Self::InputUnstartedGestureRejected => {
                ActivityClassification {
                    severity: ActivitySeverity::Warning,
                    source: ActivitySource::Protocol,
                    outcome: ActivityOutcome::Rejected,
                }
            }
            Self::VolumeWriteFailed => ActivityClassification {
                severity: ActivitySeverity::Error,
                source: ActivitySource::Action,
                outcome: ActivityOutcome::Failed,
            },
            Self::AudioEndpointChanged => ActivityClassification {
                severity: ActivitySeverity::Info,
                source: ActivitySource::Action,
                outcome: ActivityOutcome::Observed,
            },
            Self::AudioEndpointLost => ActivityClassification {
                severity: ActivitySeverity::Warning,
                source: ActivitySource::Action,
                outcome: ActivityOutcome::Unavailable,
            },
        }
    }
}

/// Typed, optional metadata for an activity event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityMetadata {
    /// Allowlisted details recorded with a connection lifecycle transition.
    Connection {
        /// The new connection state.
        state: ConnectionState,
        /// Consecutive reconnect attempts at this transition.
        retry_count: u32,
        /// Monotonic elapsed time since the device task started.
        elapsed_ms: u32,
    },
    /// Safe device diagnostic category and stable firmware code.
    DeviceDiagnostic {
        category: kivori_protocol::ErrorCategory,
        code: u16,
    },
    /// Safe identity and compatibility facts established by a successful handshake.
    Negotiated {
        firmware_major: u16,
        firmware_minor: u16,
        firmware_patch: u16,
        protocol_major: u16,
        protocol_minor: u16,
        device_id_hash_short: String,
        capabilities: u32,
    },
    Action {
        state: Option<SendableState>,
        personality: Option<MascotPersonality>,
        self_play: Option<bool>,
        action: Option<MascotAction>,
        seed: Option<u32>,
        applied_at_ms: Option<u32>,
        autonomous: Option<bool>,
    },
    ProtocolMalformed {
        category: ProtocolMalformedCategory,
        payload_len: Option<u16>,
        sequence: Option<u16>,
    },
    ProtocolSequenceGap {
        skipped: u16,
    },
    DeviceState {
        reported: kivori_model::CompanionState,
    },
    /// A safe category the desktop itself assigned to a host-side fault (no device code, no raw
    /// payload).
    HostDiagnostic {
        category: kivori_protocol::ErrorCategory,
    },
}

/// Maps a session-nonce failure to its safe [`kivori_protocol::ErrorCategory`]. The OS entropy
/// source is an I/O-class host dependency, so its unavailability is categorized the same as any
/// other I/O fault (the nonce is a freshness token, not a security credential — this is not a
/// `Handshake` failure).
#[must_use]
pub const fn category_for_nonce_error(
    _error: &crate::device::nonce::NonceError,
) -> kivori_protocol::ErrorCategory {
    kivori_protocol::ErrorCategory::Io
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolMalformedCategory {
    Framing,
    Checksum,
    Version,
    Payload,
}

/// A closed observation produced by the synchronous session parser.
///
/// The device task drains this queue and remains the sole recorder and Tauri-emission owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionActivity {
    /// Closed activity kind.
    pub kind: ActivityEventKind,
    /// Optional, typed safe metadata.
    pub metadata: Option<ActivityMetadata>,
}

impl SessionActivity {
    /// Creates a closed session observation.
    #[must_use]
    pub const fn new(kind: ActivityEventKind, metadata: Option<ActivityMetadata>) -> Self {
        Self { kind, metadata }
    }
}

/// Pure activity planner shared by the device loop and host-side producer tests.
#[derive(Debug, Default)]
pub struct RuntimeActivityPlanner {
    pending_recovery: bool,
}

/// One closed runtime request for which the device loop needs an activity observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeActivityRequest {
    State {
        state: SendableState,
        mirrored: bool,
    },
    CompanionConfiguration {
        personality: MascotPersonality,
        self_play: bool,
    },
    SocialAction {
        action: MascotAction,
        personality: MascotPersonality,
        seed: u32,
        autonomous: bool,
    },
}

impl RuntimeActivityPlanner {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pending_recovery: false,
        }
    }
    #[must_use]
    pub const fn attempt(&self) -> SessionActivity {
        SessionActivity::new(ActivityEventKind::ConnectionAttempted, None)
    }
    #[must_use]
    pub fn failure(
        &mut self,
        kind: ActivityEventKind,
        retry_scheduled: bool,
    ) -> Vec<SessionActivity> {
        self.pending_recovery = true;
        let mut observations = vec![SessionActivity::new(kind, None)];
        if retry_scheduled {
            observations.push(SessionActivity::new(
                ActivityEventKind::ConnectionRetryScheduled,
                None,
            ));
        }
        observations
    }
    pub fn recovered(&mut self) -> Option<SessionActivity> {
        self.pending_recovery.then(|| {
            self.pending_recovery = false;
            SessionActivity::new(ActivityEventKind::ConnectionRecovered, None)
        })
    }
    #[must_use]
    pub fn requests(&self, request: RuntimeActivityRequest) -> Vec<SessionActivity> {
        match request {
            RuntimeActivityRequest::State { state, mirrored } => vec![SessionActivity::new(
                if mirrored {
                    ActivityEventKind::MirroredStateRequested
                } else {
                    ActivityEventKind::StateRequested
                },
                Some(ActivityMetadata::Action {
                    state: Some(state),
                    personality: None,
                    self_play: None,
                    action: None,
                    seed: None,
                    applied_at_ms: None,
                    autonomous: Some(false),
                }),
            )],
            RuntimeActivityRequest::CompanionConfiguration {
                personality,
                self_play,
            } => vec![
                SessionActivity::new(
                    ActivityEventKind::PersonalityConfigured,
                    Some(ActivityMetadata::Action {
                        state: None,
                        personality: Some(personality),
                        self_play: None,
                        action: None,
                        seed: None,
                        applied_at_ms: None,
                        autonomous: None,
                    }),
                ),
                SessionActivity::new(
                    ActivityEventKind::SelfPlayConfigured,
                    Some(ActivityMetadata::Action {
                        state: None,
                        personality: None,
                        self_play: Some(self_play),
                        action: None,
                        seed: None,
                        applied_at_ms: None,
                        autonomous: None,
                    }),
                ),
            ],
            RuntimeActivityRequest::SocialAction {
                action,
                personality,
                seed,
                autonomous,
            } => vec![SessionActivity::new(
                if autonomous {
                    ActivityEventKind::AutonomousSocialActionRequested
                } else {
                    ActivityEventKind::ManualSocialActionRequested
                },
                Some(ActivityMetadata::Action {
                    state: None,
                    personality: Some(personality),
                    self_play: None,
                    action: Some(action),
                    seed: Some(seed),
                    applied_at_ms: None,
                    autonomous: Some(autonomous),
                }),
            )],
        }
    }
}

/// One session activity record with a process-monotonic identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEvent {
    /// Process-monotonic event identifier.
    id: u64,
    /// Native-generated UTC ISO-8601 timestamp.
    at: String,
    /// Closed activity event kind.
    kind: ActivityEventKind,
    /// Closed severity generated from the event kind.
    severity: ActivitySeverity,
    /// Closed source generated from the event kind.
    source: ActivitySource,
    /// Closed outcome generated from the event kind.
    outcome: ActivityOutcome,
    /// Optional typed, allowlisted details.
    metadata: Option<ActivityMetadata>,
    /// Native-generated summary from the event kind and allowlisted metadata.
    summary: String,
}

impl ActivityEvent {
    /// Process-monotonic event identifier.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Native-generated UTC ISO-8601 timestamp.
    #[must_use]
    pub fn at(&self) -> &str {
        &self.at
    }

    /// Closed activity event kind.
    #[must_use]
    pub const fn kind(&self) -> ActivityEventKind {
        self.kind
    }

    /// Closed severity generated from the event kind.
    #[must_use]
    pub const fn severity(&self) -> ActivitySeverity {
        self.severity
    }

    /// Closed source generated from the event kind.
    #[must_use]
    pub const fn source(&self) -> ActivitySource {
        self.source
    }

    /// Closed outcome generated from the event kind.
    #[must_use]
    pub const fn outcome(&self) -> ActivityOutcome {
        self.outcome
    }

    /// Optional typed, allowlisted details.
    #[must_use]
    pub const fn metadata(&self) -> Option<&ActivityMetadata> {
        self.metadata.as_ref()
    }

    /// Native-generated safe summary.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

/// A bounded, thread-safe, in-memory activity ring.
pub struct ActivityLog {
    entries: Mutex<VecDeque<ActivityEvent>>,
    capacity: usize,
}

impl ActivityLog {
    /// Creates a session-only activity ring holding between 1 and 256 entries.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            capacity: capacity.clamp(1, 256),
        }
    }

    /// Adds one typed event and returns the stored record.
    pub fn record(
        &self,
        kind: ActivityEventKind,
        metadata: Option<ActivityMetadata>,
    ) -> ActivityEvent {
        let mut entries = self.entries.lock().expect("activity log lock");
        let classification = kind.classification();
        let event = ActivityEvent {
            id: next_activity_id(),
            at: now_iso(),
            kind,
            severity: classification.severity,
            source: classification.source,
            outcome: classification.outcome,
            summary: summary_for(kind, metadata.as_ref()),
            metadata,
        };
        if entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(event.clone());
        event
    }

    /// Returns up to `limit` recent records, ordered from oldest to newest.
    #[must_use]
    pub fn recent(&self, limit: usize) -> Vec<ActivityEvent> {
        let entries = self.entries.lock().expect("activity log lock");
        let start = entries.len().saturating_sub(limit);
        entries.iter().skip(start).cloned().collect()
    }
}

fn next_activity_id() -> u64 {
    NEXT_ACTIVITY_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .expect("activity event id exhausted")
}

fn summary_for(kind: ActivityEventKind, metadata: Option<&ActivityMetadata>) -> String {
    match (kind, metadata) {
        (
            ActivityEventKind::ConnectionStateChanged,
            Some(ActivityMetadata::Connection { state, .. }),
        ) => {
            format!("Connection changed to {}.", connection_token(*state))
        }
        (ActivityEventKind::ConnectionStateChanged, None) => {
            "Connection state changed.".to_string()
        }
        (ActivityEventKind::ConnectionStateChanged, Some(_)) => {
            "Connection state changed.".to_string()
        }
        (ActivityEventKind::ConnectionAttempted, _) => "Connection attempted.".to_string(),
        (ActivityEventKind::ConnectionOpened, _) => "Connection opened.".to_string(),
        (ActivityEventKind::HandshakeStarted, _) => "Handshake started.".to_string(),
        (ActivityEventKind::HandshakeSucceeded, _) => "Handshake succeeded.".to_string(),
        (ActivityEventKind::ConnectionRetryScheduled, _) => {
            "Connection retry scheduled.".to_string()
        }
        (ActivityEventKind::IncompatibleFirmware, _) => "Incompatible firmware.".to_string(),
        (ActivityEventKind::ConnectionIoFailure, _) => "Connection I/O failed.".to_string(),
        (ActivityEventKind::HandshakeTimedOut, _) => "Handshake timed out.".to_string(),
        (ActivityEventKind::HeartbeatTimedOut, _) => "Heartbeat timed out.".to_string(),
        (ActivityEventKind::ConnectionRecovered, _) => "Connection recovered.".to_string(),
        (ActivityEventKind::ConnectionDisconnected, _) => "Connection disconnected.".to_string(),
        (ActivityEventKind::DeviceNegotiated, _) => "Device negotiated.".to_string(),
        (ActivityEventKind::PersonalityConfigured, _) => "Personality configured.".to_string(),
        (ActivityEventKind::SelfPlayConfigured, _) => "Self-play configured.".to_string(),
        (ActivityEventKind::StateRequested, _) => "State requested.".to_string(),
        (ActivityEventKind::MirroredStateRequested, _) => "Mirrored state requested.".to_string(),
        (ActivityEventKind::ManualSocialActionRequested, _) => {
            "Social reaction requested.".to_string()
        }
        (ActivityEventKind::AutonomousSocialActionRequested, _) => {
            "Autonomous social reaction requested.".to_string()
        }
        (ActivityEventKind::SocialActionApplied, _) => "Social reaction applied.".to_string(),
        (ActivityEventKind::StateSynchronized, _) => "Device state synchronized.".to_string(),
        (ActivityEventKind::DeviceStateObserved, _) => "Device state observed.".to_string(),
        (ActivityEventKind::DeviceDiagnosticFraming, _) => {
            "Device rejected a framing error.".to_string()
        }
        (ActivityEventKind::DeviceDiagnosticChecksum, _) => {
            "Device rejected a checksum error.".to_string()
        }
        (ActivityEventKind::DeviceDiagnosticVersion, _) => {
            "Device rejected a version error.".to_string()
        }
        (ActivityEventKind::DeviceDiagnosticPayload, _) => {
            "Device rejected a payload error.".to_string()
        }
        (ActivityEventKind::DeviceSequenceGap, _) => "Device observed a sequence gap.".to_string(),
        (ActivityEventKind::DeviceDisplayFault, _) => "Device display fault observed.".to_string(),
        (ActivityEventKind::DeviceLinkLost, _) => "Device link loss observed.".to_string(),
        (ActivityEventKind::DeviceDiagnosticUnknown, _) => {
            "Device diagnostic observed.".to_string()
        }
        (ActivityEventKind::DeviceError, _) => "Device rejected an operation.".to_string(),
        (ActivityEventKind::DeviceBusy, _) => "Device is busy.".to_string(),
        (ActivityEventKind::DeviceTimedOut, _) => "Device operation timed out.".to_string(),
        (ActivityEventKind::ProtocolMalformedFrame, _) => {
            "Malformed protocol frame dropped.".to_string()
        }
        (ActivityEventKind::ProtocolSequenceGap, _) => {
            "Protocol sequence gap observed.".to_string()
        }
        (ActivityEventKind::ActionRequested, _) => "Action requested.".to_string(),
        (ActivityEventKind::ActionCompleted, _) => "Action completed.".to_string(),
        (ActivityEventKind::ActionFailed, _) => "Action failed.".to_string(),
        (ActivityEventKind::DeviceDiscovered, _) => "Device discovered.".to_string(),
        (ActivityEventKind::DeviceRejected, _) => "Device rejected an operation.".to_string(),
        (ActivityEventKind::ProtocolMessageRejected, _) => "Protocol message rejected.".to_string(),
        (ActivityEventKind::ProtocolFailed, _) => "Protocol operation failed.".to_string(),
        (ActivityEventKind::FirmwareUpdateStarted, _) => "Firmware update started.".to_string(),
        (ActivityEventKind::FirmwareUpdateCompleted, _) => "Firmware update completed.".to_string(),
        (ActivityEventKind::FirmwareUpdateFailed, _) => "Firmware update failed.".to_string(),
        (ActivityEventKind::FirmwareAvailable, _) => "Bundled firmware is available.".to_string(),
        (ActivityEventKind::FirmwareUnavailable, _) => {
            "Bundled firmware is unavailable.".to_string()
        }
        (ActivityEventKind::FirmwareFlashRequested, _) => "Firmware flash requested.".to_string(),
        (ActivityEventKind::FirmwarePreparing, _) => "Preparing firmware update.".to_string(),
        (ActivityEventKind::FirmwareSerialReleased, _) => {
            "Serial connection released for firmware update.".to_string()
        }
        (ActivityEventKind::FirmwareFlasherStarted, _) => "Firmware flasher started.".to_string(),
        (ActivityEventKind::FirmwareFlashSucceeded, _) => {
            "Firmware flashed successfully.".to_string()
        }
        (ActivityEventKind::FirmwareReconnectWaiting, _) => {
            "Waiting for firmware reconnect.".to_string()
        }
        (ActivityEventKind::FirmwareReconnectTimedOut, _) => {
            "Firmware reconnect timed out.".to_string()
        }
        (ActivityEventKind::FirmwarePostFlashVerified, _) => {
            "Firmware update verified.".to_string()
        }
        (ActivityEventKind::FirmwarePreparationRejected, _) => {
            "Firmware preparation was rejected.".to_string()
        }
        (ActivityEventKind::SessionNonceUnavailable, _) => {
            "Session nonce unavailable; connection attempt aborted.".to_string()
        }
        (ActivityEventKind::InputStaleSessionRejected, _) => {
            "Stale-session input rejected.".to_string()
        }
        (ActivityEventKind::InputUnstartedGestureRejected, _) => {
            "Input for an unstarted gesture rejected.".to_string()
        }
        (ActivityEventKind::VolumeWriteFailed, _) => "Volume change failed.".to_string(),
        (ActivityEventKind::AudioEndpointChanged, _) => "Audio output device changed.".to_string(),
        (ActivityEventKind::AudioEndpointLost, _) => "Audio output device unavailable.".to_string(),
    }
}

/// Lowercase token shared by safe summaries and the IPC projection.
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

/// The current UTC time as an ISO-8601 second-precision string (`YYYY-MM-DDTHH:MM:SSZ`).
#[must_use]
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}
