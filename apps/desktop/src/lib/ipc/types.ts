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

/** Closed activity-event vocabulary: runtime validation rejects any other token. */
export const ACTIVITY_EVENT_TYPES = [
  'connectionAttempted',
  'connectionOpened',
  'handshakeStarted',
  'handshakeSucceeded',
  'connectionRetryScheduled',
  'incompatibleFirmware',
  'connectionIoFailure',
  'handshakeTimedOut',
  'heartbeatTimedOut',
  'connectionRecovered',
  'connectionDisconnected',
  'connectionStateChanged',
  'deviceNegotiated',
  'personalityConfigured',
  'selfPlayConfigured',
  'stateRequested',
  'mirroredStateRequested',
  'manualSocialActionRequested',
  'autonomousSocialActionRequested',
  'socialActionApplied',
  'stateSynchronized',
  'deviceStateObserved',
  'deviceDiagnosticFraming',
  'deviceDiagnosticChecksum',
  'deviceDiagnosticVersion',
  'deviceDiagnosticPayload',
  'deviceSequenceGap',
  'deviceDisplayFault',
  'deviceLinkLost',
  'deviceDiagnosticUnknown',
  'deviceError',
  'deviceBusy',
  'deviceTimedOut',
  'protocolMalformedFrame',
  'protocolSequenceGap',
  'actionRequested',
  'actionCompleted',
  'actionFailed',
  'deviceDiscovered',
  'deviceRejected',
  'protocolMessageRejected',
  'protocolFailed',
  'firmwareUpdateStarted',
  'firmwareUpdateCompleted',
  'firmwareUpdateFailed',
  'firmwareAvailable',
  'firmwareUnavailable',
  'firmwareFlashRequested',
  'firmwarePreparing',
  'firmwareSerialReleased',
  'firmwareFlasherStarted',
  'firmwareFlashSucceeded',
  'firmwareReconnectWaiting',
  'firmwareReconnectTimedOut',
  'firmwarePostFlashVerified',
  'firmwarePreparationRejected',
  'sessionNonceUnavailable',
  'inputStaleSessionRejected',
  'inputUnstartedGestureRejected',
  'volumeWriteFailed',
  'audioEndpointChanged',
  'audioEndpointLost',
  'displayModeChanged',
  'deskActionRequested',
  'deskActionConfirmed',
  'deskActionUnverified',
  'deskActionFailed',
  'deskActionPermissionRequired',
] as const;
export type ActivityEventType = (typeof ACTIVITY_EVENT_TYPES)[number];

export const MASCOT_ACTIONS: readonly MascotAction[] = [
  'greet',
  'pet',
  'tickle',
  'surprise',
  'comfort',
];

export const DISPLAY_MODES = ['buddy', 'clock', 'volume', 'media', 'system'] as const;
export type DisplayMode = (typeof DISPLAY_MODES)[number];
export const MEDIA_STATUSES = ['playing', 'paused', 'stopped'] as const;
export type MediaStatus = (typeof MEDIA_STATUSES)[number];
export const DESK_ACTIONS = [
  'volume',
  'playPause',
  'mute',
  'shortcut',
  'launch',
  'previousTrack',
  'nextTrack',
] as const;
export type DeskActionToken = (typeof DESK_ACTIONS)[number];
export const DESK_RESULTS = [
  'processing',
  'stateConfirmed',
  'executionConfirmed',
  'unverified',
  'error',
] as const;
export type DeskResult = (typeof DESK_RESULTS)[number];

/** What a double press does. Fixed in M1: show the next display mode. */
export const DOUBLE_PRESS_ACTIONS = ['nextView'] as const;
export type DoublePressAction = (typeof DOUBLE_PRESS_ACTIONS)[number];

/** Actions the dev-only Device Studio can trigger (`volume` is device-driven only). */
export type TestActionKind = 'playPause' | 'mute' | 'shortcut' | 'launch';

export interface DeskActionDto {
  action: DeskActionToken;
  result: DeskResult;
  /** The OS needs a permission first (macOS Accessibility). */
  permissionRequired: boolean;
}

/** Desk projection. Every unknown value is `null` (never guessed). */
export interface DeskStatusDto {
  mode: DisplayMode;
  volumePercent: number | null;
  muted: boolean | null;
  media: MediaStatus | null;
  cpuPercent: number | null;
  ramPercent: number | null;
  highLoad: boolean;
  pressAction: DeskActionToken;
  holdAction: DeskActionToken;
  doublePressAction: DoublePressAction;
  /** The three contextual buttons' Press, left to right; `null` = unbound. */
  buttonActions: [DeskActionToken | null, DeskActionToken | null, DeskActionToken | null];
  /** Now playing, when observable: `null` = unknown, `''` = the player shared none. */
  mediaTitle: string | null;
  mediaArtist: string | null;
  lastAction: DeskActionDto | null;
}

export interface TestActionRequest {
  action: TestActionKind;
  shortcut?: string;
  target?: string;
}

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
  /** A mascot action name, or (for desk events) a desk action token. */
  action?: MascotAction | DeskActionToken;
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
