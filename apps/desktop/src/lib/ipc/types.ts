// DTOs crossing the desktop-core ↔ webview IPC boundary. Mirrors contracts/ipc.md §4 exactly; these
// are the ONLY shapes the webview sees (no raw device ids, payload bytes, serial handles, or paths).

export type SendableState = 'idle' | 'happy' | 'busy' | 'sleeping';
export type CompanionState = 'booting' | 'idle' | 'happy' | 'busy' | 'sleeping' | 'offline';
export type MascotPersonality = 'cozy' | 'playful' | 'calm';
export type MascotAction = 'greet' | 'pet' | 'tickle' | 'surprise' | 'comfort';
export type ConnectionState =
  'connecting' | 'connected' | 'incompatible' | 'disconnected' | 'error';

export interface ProtocolVersion {
  major: number;
  minor: number;
}

export interface AppInfoDto {
  appVersion: string;
  protocolVersion: ProtocolVersion;
  supportedMajors: number[];
  deviceStudioEnabled: boolean;
}

export interface DeviceInfoDto {
  firmwareVersion: string;
  protocolVersion: ProtocolVersion;
  deviceIdHashShort: string;
}

export interface FirmwareStatusDto {
  available: boolean;
  phase: 'idle' | 'preparing' | 'flashing' | 'reconnecting' | 'succeeded' | 'failed';
  message: string;
  imageSize: number;
}

export interface ConnectionStatusDto {
  connection: ConnectionState;
  desired: SendableState;
  reported: CompanionState | null;
  device: DeviceInfoDto | null;
  incompatibleReason: string | null;
  retryCount: number;
  /** Within-process device-session identity; changes when device uptime may reset. */
  connectionGeneration: number;
  mascotInteraction: boolean;
  mascotAction: MascotActionAppliedDto | null;
}

export interface MascotActionAppliedDto {
  action: MascotAction;
  personality: MascotPersonality;
  seed: number;
  appliedAtMs: number;
}

export type ActivityEventType =
  | 'connectionAttempted'
  | 'connectionOpened'
  | 'handshakeStarted'
  | 'handshakeSucceeded'
  | 'connectionRetryScheduled'
  | 'incompatibleFirmware'
  | 'connectionIoFailure'
  | 'handshakeTimedOut'
  | 'heartbeatTimedOut'
  | 'connectionRecovered'
  | 'connectionDisconnected'
  | 'connectionStateChanged'
  | 'deviceNegotiated'
  | 'personalityConfigured'
  | 'selfPlayConfigured'
  | 'stateRequested'
  | 'mirroredStateRequested'
  | 'manualSocialActionRequested'
  | 'autonomousSocialActionRequested'
  | 'socialActionApplied'
  | 'stateSynchronized'
  | 'deviceStateObserved'
  | 'deviceDiagnosticFraming'
  | 'deviceDiagnosticChecksum'
  | 'deviceDiagnosticVersion'
  | 'deviceDiagnosticPayload'
  | 'deviceSequenceGap'
  | 'deviceDisplayFault'
  | 'deviceLinkLost'
  | 'deviceDiagnosticUnknown'
  | 'deviceError'
  | 'deviceBusy'
  | 'deviceTimedOut'
  | 'protocolMalformedFrame'
  | 'protocolSequenceGap'
  | 'actionRequested'
  | 'actionCompleted'
  | 'actionFailed'
  | 'deviceDiscovered'
  | 'deviceRejected'
  | 'protocolMessageRejected'
  | 'protocolFailed'
  | 'firmwareUpdateStarted'
  | 'firmwareUpdateCompleted'
  | 'firmwareUpdateFailed'
  | 'firmwareAvailable'
  | 'firmwareUnavailable'
  | 'firmwareFlashRequested'
  | 'firmwarePreparing'
  | 'firmwareSerialReleased'
  | 'firmwareFlasherStarted'
  | 'firmwareFlashSucceeded'
  | 'firmwareReconnectWaiting'
  | 'firmwareReconnectTimedOut'
  | 'firmwarePostFlashVerified'
  | 'firmwarePreparationRejected'
  | 'sessionNonceUnavailable'
  | 'inputStaleSessionRejected'
  | 'inputUnstartedGestureRejected'
  | 'volumeWriteFailed'
  | 'audioEndpointChanged'
  | 'audioEndpointLost';

export type ActivitySeverity = 'info' | 'warning' | 'error';
export type ActivitySource = 'connection' | 'action' | 'device' | 'protocol' | 'firmware';
export type ActivityOutcome =
  | 'observed'
  | 'started'
  | 'succeeded'
  | 'failed'
  | 'rejected'
  | 'retrying'
  | 'applied'
  | 'synchronized'
  | 'busy'
  | 'timedOut'
  | 'available'
  | 'unavailable';
export type ActivityConnectionState = ConnectionState;
export type ActivityDiagnosticCategory =
  'io' | 'handshake' | 'version' | 'framing' | 'checksum' | 'timeout' | 'busy' | 'badPayload';
export type ProtocolMalformedCategory = 'framing' | 'checksum' | 'version' | 'payload';

/** Fixed activity metadata allowlist. Optional fields are omitted by Rust when unavailable. */
export interface ActivityMetadataDto {
  connection?: ActivityConnectionState;
  retryCount: number;
  elapsedMs: number;
  diagnosticCategory?: ActivityDiagnosticCategory;
  diagnosticCode?: number;
  firmwareVersion?: string;
  protocolVersion?: ProtocolVersion;
  deviceIdHashShort?: string;
  capabilities?: number;
  state?: SendableState;
  personality?: MascotPersonality;
  selfPlay?: boolean;
  action?: MascotAction;
  seed?: number;
  appliedAtMs?: number;
  autonomous?: boolean;
  protocolCategory?: ProtocolMalformedCategory;
  payloadLen?: number;
  sequence?: number;
  skipped?: number;
  reported?: CompanionState;
}

/** One native-issued, typed and allowlisted activity event. */
export interface ActivityEventDto {
  id: number;
  at: string;
  type: ActivityEventType;
  summary: string;
  severity: ActivitySeverity;
  source: ActivitySource;
  outcome: ActivityOutcome;
  metadata: ActivityMetadataDto | null;
}

/** Every companion state, in canonical display order. */
export const COMPANION_STATES: readonly CompanionState[] = [
  'booting',
  'idle',
  'happy',
  'busy',
  'sleeping',
  'offline',
];

/** States a host may request the device enter (a subset of {@link COMPANION_STATES}). */
export const SENDABLE_STATES: readonly SendableState[] = ['idle', 'happy', 'busy', 'sleeping'];

/** Panel edge length in pixels (square). Matches `DeviceProfile::KIVORI_240`. */
export const PREVIEW_DIM = 240;

/** Canonical preview stream rate (frames per second); the native side caps to this. */
export const PREVIEW_FPS = 30;
/** Semantic changes replayed by the shared Rust animator; no animation math in React. */
export interface AnimationEvent {
  atMs: number;
  state: CompanionState;
}

/** One deterministic social reaction replayed by the native mascot animator. */
export interface MascotActionEvent {
  atMs: number;
  action: MascotAction;
  personality: MascotPersonality;
  seed: number;
  /** Original device uptime from an applied-action acknowledgment. */
  deviceAppliedAtMs?: number;
  /** Device session in which the action was applied. */
  connectionGeneration?: number;
}

export interface AnimationTimeline {
  initialState: CompanionState;
  events: AnimationEvent[];
  actionEvents: MascotActionEvent[];
}
export const MAX_ANIMATION_EVENTS = 256;
