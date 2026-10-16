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

/** Where a device's identity came from: `legacy` means it reported none (the original Kivori Pip). */
export const IDENTITY_SOURCES = ['legacy', 'build', 'chip'] as const;
export type IdentitySource = (typeof IDENTITY_SOURCES)[number];

/** Which product and board a connected device is. Names are display names, never ids. */
export interface DeviceIdentityDto {
  product: string;
  board: string;
  /** `null` until provisioning burned a revision. */
  hwRev: number | null;
  source: IdentitySource;
  /** Whether this app knows both the product and the board. */
  known: boolean;
}

/** Customer metadata from the compiled catalog. It never authorizes live features. */
export interface ProductPresentationDto {
  displayName: string;
  model: string;
  /** Opaque compiled artwork key, never a filesystem path or URL. */
  art: string | null;
}

export interface DeviceInfoDto {
  presentation: ProductPresentationDto | null;
  firmwareVersion: string;
  protocolVersion: ProtocolVersion;
  deviceIdHashShort: string;
  /** `null` while a device that reports its identity has not yet. */
  identity: DeviceIdentityDto | null;
}

export const FIRMWARE_PHASES = [
  'idle',
  'preparing',
  'flashing',
  'reconnecting',
  'succeeded',
  'failed',
] as const;
/** Why the last firmware attempt failed; the UI owns the wording. */
export const FLASH_FAILURES = [
  'toolMissing',
  'portBusy',
  'noDownloadMode',
  'timeout',
  'cancelled',
  'imageUnavailable',
  'reconnectTimedOut',
  'wrongProduct',
  'chooseProduct',
  'unknown',
] as const;
export type FlashFailure = (typeof FLASH_FAILURES)[number];
/** Whether the connected device should be updated to the firmware bundled with this app. */
export const UPDATE_ADVICE = ['upToDate', 'updateAvailable', 'deviceNewer', 'unknown'] as const;
export type UpdateAdvice = (typeof UPDATE_ADVICE)[number];

/** Which OS this build runs on, for the wording of the start-at-login switch. */
/** Whether first-run setup is finished (or was skipped). */
export interface OnboardingDto {
  completed: boolean;
}

/** The macOS Accessibility permission; `notApplicable` on every other OS. */
export const ACCESSIBILITY_STATES = ['granted', 'missing', 'notApplicable'] as const;
export type AccessibilityState = (typeof ACCESSIBILITY_STATES)[number];

export const STARTUP_PLATFORMS = ['windows', 'macos', 'other'] as const;
export type StartupPlatform = (typeof STARTUP_PLATFORMS)[number];

/** Whether Kivori starts at login; read from the OS entry, never stored in the config file. */
export interface StartupSettingsDto {
  launchAtLogin: boolean;
  platform: StartupPlatform;
}

/** The app updater's states (ADR-0015). `disabled` and `unsupported` never make a request. */
export const UPDATE_STATES = [
  'disabled',
  'unsupported',
  'idle',
  'checking',
  'available',
  'downloading',
  'readyToRestart',
  'failed',
] as const;
export type UpdateState = (typeof UPDATE_STATES)[number];
/** Why the updater is off: a development build, no public key yet, or a wrong endpoint. */
export const UPDATE_DISABLED_REASONS = ['devBuild', 'noKey', 'badEndpoint'] as const;
export type UpdateDisabledReason = (typeof UPDATE_DISABLED_REASONS)[number];

/** An offered release. `notes` is plain text: render it as text, never as HTML. */
export interface UpdateReleaseDto {
  version: string;
  notes: string;
  date: string | null;
}

export interface UpdateStatusDto {
  state: UpdateState;
  /** Set only while `state` is `disabled`. */
  reason: UpdateDisabledReason | null;
  currentVersion: string;
  /** The last check that got an answer this session (UTC ISO-8601). */
  lastChecked: string | null;
  autoCheck: boolean;
  /** Set while `available`, `downloading`, `readyToRestart` or `failed`. */
  update: UpdateReleaseDto | null;
  /** Set while `downloading`; `total` is `null` when the size is unknown. */
  progress: { downloaded: number; total: number | null } | null;
}

/**
 * What the app says about the connected device's firmware (ADR-0015, step 6): an optional update,
 * a required one, "update the app", waiting for an app update first, or nothing.
 */
export const FIRMWARE_OFFER_KINDS = [
  'none',
  'offerOptional',
  'required',
  'updateApp',
  'deferredForAppUpdate',
] as const;
export type FirmwareOfferKind = (typeof FIRMWARE_OFFER_KINDS)[number];
/** Why an update is required: an older protocol, or a capability the product needs is missing. */
export const FIRMWARE_REQUIRED_REASONS = ['protocolTooOld', 'missingCapabilities'] as const;
export type FirmwareRequiredReason = (typeof FIRMWARE_REQUIRED_REASONS)[number];

export interface FirmwareOfferDto {
  kind: FirmwareOfferKind;
  /** The device's product name, when known (only with `offerOptional` or `required`). */
  product: string | null;
  /** The device's firmware version, with `offerOptional`. */
  from: string | null;
  /** The bundled firmware version, with `offerOptional` and `required`. */
  to: string | null;
  /** Set only with `required`. */
  reason: FirmwareRequiredReason | null;
  /** Changes per device, bundled version and connection: the dialog opens by itself once per key. */
  key: string | null;
}

export interface FirmwareStatusDto {
  available: boolean;
  phase: (typeof FIRMWARE_PHASES)[number];
  message: string;
  imageSize: number;
  /** Set only while `phase` is `failed`. */
  failure: FlashFailure | null;
  /** Firmware version bundled in this app, when known. */
  bundledVersion: string | null;
  advice: UpdateAdvice;
  /** Both names; set only with the `wrongProduct` failure. */
  wrongProduct: { device: string; image: string } | null;
  /** The products to pick from; set only with the `chooseProduct` failure. */
  restoreChoices: { id: string; name: string }[];
}

export const CAPABILITY_NAMES = [
  'mascotInteraction',
  'physicalInputV1',
  'presentationV1',
  'buttonInputV1',
  'deskStatusV1',
  'actionFeedbackV1',
  'doublePressV1',
  'mediaInfoV1',
  'controlLabelsV1',
  'contextButtonsV1',
  'hostTakeoversV1',
  'quietAckV1',
  'deviceInternalsV1',
  'idleLifeV1',
  'micStateV1',
  'productIdentityV1',
] as const;
export type CapabilityName = (typeof CAPABILITY_NAMES)[number];

export const HOST_AVAILABILITIES = ['available', 'unsupported', 'unavailable'] as const;
export const HOST_MEDIA = ['observable', 'notObservable'] as const;
export const HOST_FOCUS = ['detecting', 'unknown'] as const;
/** What the computer is doing, as `ConnectionStatusDto.host` reports it. */
export const HOST_PRESENCES = ['active', 'locked', 'sleeping'] as const;
export type HostPresence = (typeof HOST_PRESENCES)[number];

export const HOST_INPUT_PERMISSIONS = ['required', 'granted', 'notNeeded', 'unknown'] as const;

/**
 * Everything the Diagnostics card shows and "Copy diagnostics" copies. Unknown values are `null`.
 * Carries no port, path or raw device id; the device is only its short hash.
 */
export interface DiagnosticsDto {
  versions: {
    app: string;
    firmware: string | null;
    /** This app's protocol version, `major.minor`. */
    protocol: string;
    negotiatedMinor: number | null;
    capabilities: CapabilityName[];
    deviceHash: string | null;
    /** Which product and board the connected device is. */
    identity: DeviceIdentityDto | null;
  };
  connection: {
    generation: number;
    state: ConnectionState;
    connectedForSecs: number | null;
    reconnects: number;
    retryCount: number;
    lastPongAgeMs: number | null;
    rttMs: number | null;
  };
  health: {
    deviceUptimeMs: number | null;
    freeBytes: number | null;
    malformedFrames: number;
    sequenceGaps: number;
    deviceErrors: number;
  };
  host: {
    systemVolume: (typeof HOST_AVAILABILITIES)[number];
    appVolume: (typeof HOST_AVAILABILITIES)[number];
    media: (typeof HOST_MEDIA)[number];
    focus: (typeof HOST_FOCUS)[number];
    inputPermission: (typeof HOST_INPUT_PERMISSIONS)[number];
  };
  config: {
    status: 'ok' | ConfigNotice;
    schemaVersion: number;
    customBindings: number;
    macros: number;
  };
}

export const CHIP_MODELS = ['esp32c3'] as const;
export const RESET_REASONS = [
  'powerOn',
  'software',
  'watchdog',
  'brownout',
  'deepSleep',
  'other',
] as const;

/**
 * The developer-mode Device internals card (ADR-0013). `snapshot` is `null` until the device has
 * answered on this connection; inside it, `null` means the device cannot measure that value, never 0.
 */
export interface DeviceInternalsDto {
  connectionGeneration: number;
  /** Age since the native session accepted this report, not since the IPC read. */
  snapshotAgeMs: number | null;
  connected: boolean;
  /** The connected device's firmware reports internals. */
  supported: boolean;
  snapshot: {
    appImageBytes: number | null;
    appPartitionBytes: number | null;
    assetBlobBytes: number | null;
    assetBlobCapBytes: number | null;
    flashTotalBytes: number | null;
    ramTotalBytes: number | null;
    ramStaticBytes: number | null;
    ramFreeBytes: number | null;
    stackFreeMinBytes: number | null;
    busyPercent: number | null;
    composeMs: number | null;
    flushMs: number | null;
    fps: number | null;
    chipModel: (typeof CHIP_MODELS)[number] | null;
    chipRevision: { major: number; minor: number } | null;
    /** Tenths of a degree Celsius. */
    temperatureDeciCelsius: number | null;
    uptimeMs: number | null;
    resetReason: (typeof RESET_REASONS)[number] | null;
    edgeQueueHighWater: number | null;
    droppedEdges: number | null;
    invalidTransitions: number | null;
    framesIn: number | null;
    framesOut: number | null;
    framesRejected: number | null;
  } | null;
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
  capabilities: CapabilityName[];
  mascotInteraction: boolean;
  mascotAction: MascotActionAppliedDto | null;
  /** `locked` shows the sleeping buddy; `sleeping` means the port was released for a suspend. */
  host: HostPresence;
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
  'hostSuspending',
  'hostResumed',
  'hostLocked',
  'hostUnlocked',
  'appUpdateCheckFailed',
  'appUpdateAvailable',
  'appUpdateInstallStarted',
  'appUpdateInstallFailed',
  'firmwareOfferShown',
  'firmwareOfferAccepted',
  'firmwareOfferLater',
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

/** The built-in profiles; a custom profile's id is `custom-<n>`. */
export const PROFILE_IDS = ['general', 'browser', 'code', 'media', 'zoom', 'teams'] as const;
export type BuiltinProfileId = (typeof PROFILE_IDS)[number];
export type ProfileId = BuiltinProfileId | `custom-${number}`;
/** A custom profile's id: `custom-` and 1 to 4 digits. */
export const CUSTOM_PROFILE_ID = /^custom-\d{1,4}$/;
/** The native limits, checked again there. */
export const PROFILE_LIMITS = { customProfiles: 16, apps: 32, nameMax: 32 } as const;

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
  /** Mutes every microphone the OS has, not an app's own mute button. */
  | { kind: 'micMute' }
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
  'micMute',
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
  /** The user's own profile: it can be renamed and deleted. Built-ins cannot. */
  custom: boolean;
  /** The middle button's Hold cycle visits it (always true for a built-in). */
  inPinCycle: boolean;
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

/** Backlight steps, 1 (dimmest, still clearly on) to 5 (full). */
export const BRIGHTNESS_STEPS = 5;

/** The saved settings. `secondaryView: 'cycle'` = a double press steps through every view. */
export interface ConfigDto {
  version: number;
  /** Counts saves and resets in this run: a lower revision is a stale copy. */
  revision: number;
  notice: ConfigNotice | null;
  /** Every profile as resolved: the built-ins with the user's overrides (General first), then the
   *  user's own profiles in list order. */
  profiles: ProfileConfigDto[];
  /** The user's macros, in the order they were created. */
  macros: MacroSpec[];
  display: { defaultView: DisplayMode; secondaryView: DisplayMode | 'cycle'; brightness: number };
  buddy: { reactions: boolean; intensity: Intensity };
}
export const MEDIA_STATUSES = ['playing', 'paused', 'stopped'] as const;
export type MediaStatus = (typeof MEDIA_STATUSES)[number];
export const DESK_ACTIONS = [
  'volume',
  'playPause',
  'mute',
  'micMute',
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

/** An app by foreground id (Windows: `code.exe`; macOS: bundle id) and display name. */
export interface AppRefDto {
  id: string;
  name: string;
}

/** Desk projection. Every unknown value is `null` (never guessed). */
/** Whether the microphones are muted: `yes` only when every one reads muted, never a guess. */
export const MIC_MUTED_STATES = ['yes', 'partial', 'no', 'unknown'] as const;
export type MicMutedState = (typeof MIC_MUTED_STATES)[number];
export const MIC_DEVICE_STATES = [
  'muted',
  'live',
  'unknown',
  'cannotMuteIdle',
  'cannotMuteLive',
  'cannotMuteUnknown',
] as const;
export type MicDeviceState = (typeof MIC_DEVICE_STATES)[number];

export interface MicDeviceDto {
  name: string;
  state: MicDeviceState;
}

/** The computer's microphones. Device names are for this window only; the activity log has none. */
export interface MicDto {
  muted: MicMutedState;
  /** An app is using a microphone; `null` where the OS does not say (never read as "no"). */
  inUse: boolean | null;
  /** The "Live" cue: in use AND unmuted. Unknown on either side is `false`. */
  live: boolean;
  devices: MicDeviceDto[];
}

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
  mic: MicDto;
  /** The last app in front that is not Kivori itself (`null` = none yet). */
  recentApp: AppRefDto | null;
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
  'connection' | 'action' | 'device' | 'protocol' | 'firmware' | 'config' | 'update';
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
  /** The buddy's own idle moments, replayed by the same code the firmware runs (fixed seed). */
  idleLife?: { personality: MascotPersonality; selfPlay: boolean };
}
export const MAX_ANIMATION_EVENTS = 256;
