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
  'inputStale',
  'volumeWriteFailed',
  'audioEndpointChanged',
  'audioEndpointLost',
  'displayModeChanged',
  'deskActionRequested',
  'deskActionConfirmed',
  'deskActionUnverified',
  'deskActionFailed',
  'deskActionPermissionRequired',
  'configSaved',
  'configRecovered',
  'configReset',
  'configSaveFailed',
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

export const INTENSITIES = ['low', 'normal', 'high'] as const;
export type Intensity = (typeof INTENSITIES)[number];
/** Low, Normal and High are the calm, cozy and playful personalities. */
export const INTENSITY_PERSONALITY: Record<Intensity, MascotPersonality> = {
  low: 'calm',
  normal: 'cozy',
  high: 'playful',
};
export const CONFIG_NOTICES = ['recoveredCorrupt', 'recoveredNewerVersion', 'migrated'] as const;
export type ConfigNotice = (typeof CONFIG_NOTICES)[number];

export const PROFILE_IDS = ['general', 'browser', 'code', 'media', 'zoom', 'teams'] as const;
export type ProfileId = (typeof PROFILE_IDS)[number];

/** Controls `setBinding` targets. The middle button's Hold is reserved for the profile pin. */
export const CONTROL_REFS = [
  'press',
  'hold',
  'button1Press',
  'button1Hold',
  'button2Press',
  'button3Press',
  'button3Hold',
] as const;
export type ControlRef = (typeof CONTROL_REFS)[number];

export type ActionSpec =
  | { kind: 'playPause' }
  | { kind: 'previousTrack' }
  | { kind: 'nextTrack' }
  | { kind: 'systemMute' }
  /** `app` is the lowercase executable name on Windows (`spotify.exe`), at most 128 characters. */
  | { kind: 'appMute'; app: string }
  | { kind: 'shortcut'; keys: string }
  | { kind: 'launch'; target: string }
  /** Runs a saved macro by its id. Never valid as a macro's own step. */
  | { kind: 'macro'; id: string };
export const ACTION_SPEC_KINDS = [
  'playPause',
  'previousTrack',
  'nextTrack',
  'systemMute',
  'appMute',
  'shortcut',
  'launch',
  'macro',
] as const;

export const STEP_SPEC_KINDS = ['action', 'delay'] as const;
/** One step of a macro: run an action (never a macro), or wait. */
export type StepSpec =
  | { kind: 'action'; action: Exclude<ActionSpec, { kind: 'macro' }> }
  | { kind: 'delay'; ms: number };
/** A user macro: its steps run in order. `id` is stable (a-z, 0-9, - and _); bindings name it. */
export interface MacroSpec {
  id: string;
  name: string;
  steps: StepSpec[];
}
/** The native limits, checked again there. */
export const MACRO_LIMITS = { macros: 32, steps: 8, delayMin: 50, delayMax: 2000 } as const;

export const ROTATE_SPEC_KINDS = ['systemVolume', 'shortcuts', 'appVolume'] as const;
export type RotateSpec =
  | { kind: 'systemVolume' }
  | { kind: 'shortcuts'; cw: string; ccw: string; label: string }
  /** One app's volume. `label` defaults to the app's name without `.exe`. */
  | { kind: 'appVolume'; app: string; label?: string };

/** A replacement for one slot: `action: null` = explicitly unbound. */
export interface SlotSpec {
  action: ActionSpec | null;
  label?: string | null;
}

export interface SlotDto {
  /** `null` = unbound. */
  action: ActionSpec | null;
  /** The custom label; `null` = the action's own. */
  label: string | null;
  /** What the device shows (`''` = unbound). */
  deviceLabel: string;
  /** Changed from the built-in. */
  overridden: boolean;
}

export interface ProfileConfigDto {
  id: ProfileId;
  name: string;
  /** Foreground app ids that select this profile (empty for General). */
  apps: string[];
  rotate: { spec: RotateSpec; deviceLabel: string; overridden: boolean };
  press: SlotDto;
  hold: SlotDto;
  /** A button's Hold is `'pin'` for the middle one (reserved for profile pin). */
  buttons: [ButtonDto, ButtonDto, ButtonDto];
}
export interface ButtonDto {
  press: SlotDto;
  hold: SlotDto | 'pin';
}

/** The saved settings. `secondaryView: 'cycle'` = a double press steps through every view. */
export interface ConfigDto {
  version: number;
  /** Counts saves and resets in this run: a lower revision is a stale copy. */
  revision: number;
  notice: ConfigNotice | null;
  /** Every built-in profile as resolved (overrides over built-ins), General first. */
  profiles: ProfileConfigDto[];
  /** The user's macros, in the order they were created. */
  macros: MacroSpec[];
  display: { defaultView: DisplayMode; secondaryView: DisplayMode | 'cycle' };
  buddy: { reactions: boolean; intensity: Intensity };
}
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
  'appVolume',
  'appMute',
  'macro',
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
  /** Press and Hold; `null` = unbound. */
  pressAction: DeskActionToken | null;
  holdAction: DeskActionToken | null;
  doublePressAction: DoublePressAction;
  /** The three contextual buttons' Press, left to right; `null` = unbound. */
  buttonActions: [DeskActionToken | null, DeskActionToken | null, DeskActionToken | null];
  /** The three contextual buttons' Hold; `null` = unbound (the middle one always is: it pins). */
  buttonHoldActions: [DeskActionToken | null, DeskActionToken | null, DeskActionToken | null];
  /** The active profile (the config UI keys its tabs by it). */
  profileId: ProfileId;
  /** The active profile's name as the device shows it; `null` = the General fallback. */
  profile: string | null;
  /** The profile was pinned on the device instead of following the app in front. */
  pinned: boolean;
  /** What the knob does, as the device labels it; `''` = suspended here. */
  rotateLabel: string;
  /** Each contextual button's device label; `''` = unbound or suspended here. */
  buttonLabels: [string, string, string];
  /** Now playing, when observable: `null` = unknown, `''` = the player shared none. */
  mediaTitle: string | null;
  mediaArtist: string | null;
  lastAction: DeskActionDto | null;
}

export const CATALOG_SLOTS = ['rotate', 'discrete'] as const;
export const CATALOG_SCOPES = ['system', 'app', 'media', 'keyboard', 'launch', 'macro'] as const;
export const CATALOG_VERIFICATIONS = [
  'confirmed',
  'started',
  'unverified',
  'leastOfSteps',
] as const;
export const CATALOG_PARAMS = [
  'none',
  'app',
  'shortcut',
  'shortcutPair',
  'target',
  'macro',
] as const;
export const CATALOG_AVAILABILITIES = ['available', 'unsupported', 'unavailable'] as const;

/**
 * One bindable action. `unsupported` = this OS or build cannot (shown disabled, with `reason`);
 * `unavailable` = not right now.
 */
export interface ActionCatalogEntryDto {
  id: string;
  slot: (typeof CATALOG_SLOTS)[number];
  scope: (typeof CATALOG_SCOPES)[number];
  verification: (typeof CATALOG_VERIFICATIONS)[number];
  params: (typeof CATALOG_PARAMS)[number];
  availability: (typeof CATALOG_AVAILABILITIES)[number];
  reason: string | null;
  /** Runs while the foreground is a protected surface. */
  runsWhenProtected: boolean;
}

export type ActivitySeverity = 'info' | 'warning' | 'error';
export type ActivitySource =
  'connection' | 'action' | 'device' | 'protocol' | 'firmware' | 'config';
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
