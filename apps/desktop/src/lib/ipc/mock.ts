// Browser development IPC mock — DEV builds ONLY.
//
// This module is imported lazily behind a static `import.meta.env.DEV` guard (see `./index.ts`), so
// Vite/Rollup drop it entirely from production Tauri bundles (T124). The sentinel below is asserted
// absent from the production build by `scripts/check-mock-excluded.mjs`.

import type {
  ActionCatalogEntryDto,
  ActionSpec,
  ActivityEventDto,
  AppInfoDto,
  CompanionState,
  ConfigDto,
  ConnectionStatusDto,
  ControlRef,
  DeskActionToken,
  DeskStatusDto,
  DisplayMode,
  Intensity,
  ProfileConfigDto,
  ProfileId,
  RotateSpec,
  SlotDto,
  SlotSpec,
} from './types';
import { COMPANION_STATES, PREVIEW_DIM } from './types';

/// A unique marker string; `scripts/check-mock-excluded.mjs` fails if it appears in a prod bundle.
export const MOCK_BUILD_SENTINEL = 'kivori-ipc-browser-mock-must-not-ship';

export function mockAppInfo(): AppInfoDto {
  return {
    appVersion: '0.0.0-dev',
    protocolVersion: { major: 1, minor: 0 },
    supportedMajors: [1],
    deviceStudioEnabled: true,
  };
}

// Browser preview scenarios for design review: `?mock=connected`, `?mock=incompatible`,
// `?mock=connecting`. Without the parameter the mock is the plain disconnected default.
function scenario(): string {
  return typeof location === 'undefined'
    ? ''
    : (new URLSearchParams(location.search).get('mock') ?? '');
}

export function mockConnectionStatus(): ConnectionStatusDto {
  const base: ConnectionStatusDto = {
    connection: 'disconnected',
    desired: 'idle',
    reported: null,
    device: null,
    incompatibleReason: null,
    retryCount: 0,
    connectionGeneration: 0,
    mascotInteraction: false,
    mascotAction: null,
  };
  switch (scenario()) {
    case 'connected':
      return {
        ...base,
        connection: 'connected',
        reported: 'idle',
        device: {
          firmwareVersion: '0.4.0',
          protocolVersion: { major: 1, minor: 3 },
          deviceIdHashShort: '3fa9c1d2',
        },
        connectionGeneration: 1,
        mascotInteraction: true,
      };
    case 'incompatible':
      return {
        ...base,
        connection: 'incompatible',
        incompatibleReason: 'This Kivori runs protocol v2, which this app does not support yet.',
      };
    case 'connecting':
      return { ...base, connection: 'connecting', retryCount: 2 };
    default:
      return base;
  }
}

export function mockListStates(): CompanionState[] {
  return [...COMPANION_STATES];
}

export function mockActivityLog(): ActivityEventDto[] {
  if (scenario() !== 'connected') return [];
  const at = (s: number): string => new Date(Date.now() - s * 1000).toISOString();
  const meta = { retryCount: 0, elapsedMs: 0 };
  return [
    {
      id: 1,
      at: at(320),
      type: 'connectionAttempted',
      summary: 'Looking for a Kivori.',
      severity: 'info',
      source: 'connection',
      outcome: 'started',
      metadata: meta,
    },
    {
      id: 2,
      at: at(318),
      type: 'handshakeSucceeded',
      summary: 'Handshake complete.',
      severity: 'info',
      source: 'connection',
      outcome: 'succeeded',
      metadata: { ...meta, firmwareVersion: '0.4.0', protocolVersion: { major: 1, minor: 3 } },
    },
    {
      id: 3,
      at: at(240),
      type: 'displayModeChanged',
      summary: 'Display view changed.',
      severity: 'info',
      source: 'action',
      outcome: 'applied',
      metadata: meta,
    },
    {
      id: 4,
      at: at(200),
      type: 'deskActionUnverified',
      summary: 'Desk action sent; result unknown.',
      severity: 'warning',
      source: 'action',
      outcome: 'observed',
      metadata: { ...meta, action: 'playPause' },
    },
    {
      id: 5,
      at: at(120),
      type: 'deskActionConfirmed',
      summary: 'Desk action confirmed.',
      severity: 'info',
      source: 'action',
      outcome: 'succeeded',
      metadata: { ...meta, action: 'mute' },
    },
    {
      id: 6,
      at: at(60),
      type: 'heartbeatTimedOut',
      summary: 'Heartbeat timed out.',
      severity: 'error',
      source: 'device',
      outcome: 'timedOut',
      metadata: { ...meta, retryCount: 1 },
    },
    {
      id: 7,
      at: at(55),
      type: 'connectionRecovered',
      summary: 'Connection recovered.',
      severity: 'info',
      source: 'connection',
      outcome: 'succeeded',
      metadata: meta,
    },
  ];
}

const connectedDesk: Partial<DeskStatusDto> =
  scenario() === 'connected'
    ? {
        media: 'playing',
        mediaTitle: 'Weightless',
        mediaArtist: 'Marconi Union',
        cpuPercent: 87,
        highLoad: true,
        lastAction: { action: 'playPause', result: 'unverified', permissionRequired: false },
      }
    : {};

let deskStatus: DeskStatusDto = {
  mode: 'buddy',
  volumePercent: 42,
  muted: false,
  media: null,
  cpuPercent: 23,
  ramPercent: 61,
  highLoad: false,
  pressAction: 'playPause',
  holdAction: 'mute',
  doublePressAction: 'nextView',
  buttonActions: ['previousTrack', 'playPause', 'nextTrack'],
  buttonHoldActions: [null, null, null],
  profileId: 'general',
  profile: null,
  pinned: false,
  rotateLabel: 'Volume',
  buttonLabels: ['Previous', 'Play/Pause', 'Next'],
  mediaTitle: null,
  mediaArtist: null,
  lastAction: null,
  ...connectedDesk,
};
const deskListeners = new Set<(status: DeskStatusDto) => void>();

function updateDesk(next: Partial<DeskStatusDto>): void {
  deskStatus = { ...deskStatus, ...next };
  for (const listener of deskListeners) listener(deskStatus);
}

export function mockDeskStatus(): DeskStatusDto {
  return deskStatus;
}

export function mockSetDisplayMode(mode: DisplayMode): void {
  updateDesk({ mode });
}

const ACTION_TOKENS: Record<ActionSpec['kind'], DeskActionToken> = {
  playPause: 'playPause',
  previousTrack: 'previousTrack',
  nextTrack: 'nextTrack',
  systemMute: 'mute',
  shortcut: 'shortcut',
  launch: 'launch',
};

/** Mirrors native rules loosely: a shortcut/launch needs its text; the outcome is "unverified". */
export function mockTestAction(action: ActionSpec): void {
  if (action.kind === 'shortcut' && !action.keys.trim()) {
    throw new Error('not a valid shortcut');
  }
  if (action.kind === 'launch' && !action.target.trim()) {
    throw new Error('not a valid application');
  }
  const result =
    action.kind === 'systemMute'
      ? 'stateConfirmed'
      : action.kind === 'launch'
        ? 'executionConfirmed'
        : 'unverified';
  updateDesk({
    muted: action.kind === 'systemMute' ? !deskStatus.muted : deskStatus.muted,
    lastAction: { action: ACTION_TOKENS[action.kind], result, permissionRequired: false },
  });
}

const catalogEntry = (
  id: string,
  slot: ActionCatalogEntryDto['slot'],
  scope: ActionCatalogEntryDto['scope'],
  verification: ActionCatalogEntryDto['verification'],
  params: ActionCatalogEntryDto['params'],
  runsWhenProtected: boolean,
  comingSoon = false,
): ActionCatalogEntryDto => ({
  id,
  slot,
  scope,
  verification,
  params,
  availability: comingSoon ? 'unsupported' : 'available',
  reason: comingSoon ? 'Coming soon' : null,
  runsWhenProtected,
});

/** The native catalog: App Volume, App Mute and macros are listed but not built yet. */
export function mockListActionCatalog(): ActionCatalogEntryDto[] {
  return [
    catalogEntry('systemVolume', 'rotate', 'system', 'confirmed', 'none', true),
    catalogEntry('appVolume', 'rotate', 'app', 'confirmed', 'app', true, true),
    catalogEntry('knobShortcuts', 'rotate', 'keyboard', 'unverified', 'shortcutPair', false),
    catalogEntry('playPause', 'discrete', 'media', 'unverified', 'none', true),
    catalogEntry('previousTrack', 'discrete', 'media', 'unverified', 'none', true),
    catalogEntry('nextTrack', 'discrete', 'media', 'unverified', 'none', true),
    catalogEntry('systemMute', 'discrete', 'system', 'confirmed', 'none', true),
    catalogEntry('appMute', 'discrete', 'app', 'confirmed', 'app', true, true),
    catalogEntry('shortcut', 'discrete', 'keyboard', 'unverified', 'shortcut', false),
    catalogEntry('launch', 'discrete', 'launch', 'started', 'target', false),
    catalogEntry('macro', 'discrete', 'macro', 'leastOfSteps', 'macro', false, true),
  ];
}

function actionLabel(action: ActionSpec | null): string {
  switch (action?.kind) {
    case undefined:
      return '';
    case 'playPause':
      return 'Play/Pause';
    case 'previousTrack':
      return 'Previous';
    case 'nextTrack':
      return 'Next';
    case 'systemMute':
      return 'Mute';
    case 'shortcut':
      return action.keys;
    case 'launch':
      return action.target;
  }
}

function slot(action: ActionSpec | null, label: string | null = null): SlotDto {
  return { action, label, deviceLabel: label ?? actionLabel(action), overridden: false };
}
const key = (keys: string): ActionSpec => ({ kind: 'shortcut', keys });
const media = { kind: 'playPause' } as const;
const mediaButtons: ProfileConfigDto['buttons'] = [
  { press: slot({ kind: 'previousTrack' }), hold: slot(null) },
  { press: slot(media), hold: 'pin' },
  { press: slot({ kind: 'nextTrack' }), hold: slot(null) },
];
const shortcutButtons = (
  buttons: [[string, string], [string, string], [string, string]],
): ProfileConfigDto['buttons'] => [
  { press: slot(key(buttons[0][1]), buttons[0][0]), hold: slot(null) },
  { press: slot(key(buttons[1][1]), buttons[1][0]), hold: 'pin' },
  { press: slot(key(buttons[2][1]), buttons[2][0]), hold: slot(null) },
];
function builtin(
  id: ProfileId,
  name: string,
  apps: string[],
  buttons: ProfileConfigDto['buttons'] = mediaButtons,
  rotate: ProfileConfigDto['rotate']['spec'] = { kind: 'systemVolume' },
): ProfileConfigDto {
  return {
    id,
    name,
    apps,
    rotate: {
      spec: rotate,
      deviceLabel: rotate.kind === 'shortcuts' ? rotate.label : 'Volume',
      overridden: false,
    },
    press: slot(media),
    hold: slot({ kind: 'systemMute' }),
    buttons,
  };
}

/** The built-in profiles as Windows resolves them (what `?mock` shows before any override). */
function builtinProfiles(): ProfileConfigDto[] {
  return [
    builtin('general', 'General', []),
    builtin(
      'browser',
      'Browser',
      ['chrome.exe', 'msedge.exe', 'firefox.exe', 'brave.exe'],
      shortcutButtons([
        ['Back', 'Alt+Left'],
        ['Reload', 'Ctrl+R'],
        ['New tab', 'Ctrl+T'],
      ]),
      { kind: 'shortcuts', cw: 'Ctrl+Tab', ccw: 'Ctrl+Shift+Tab', label: 'Tabs' },
    ),
    builtin(
      'code',
      'Code',
      ['code.exe'],
      shortcutButtons([
        ['Terminal', 'Ctrl+`'],
        ['Run', 'F5'],
        ['Git', 'Ctrl+Shift+G'],
      ]),
    ),
    builtin('media', 'Media', ['spotify.exe']),
    builtin(
      'zoom',
      'Zoom',
      ['zoom.exe'],
      shortcutButtons([
        ['Mic', 'Alt+A'],
        ['Video', 'Alt+V'],
        ['Leave', 'Alt+Q'],
      ]),
    ),
    builtin(
      'teams',
      'Teams',
      ['ms-teams.exe', 'teams.exe'],
      shortcutButtons([
        ['Mic', 'Ctrl+Shift+M'],
        ['Video', 'Ctrl+Shift+O'],
        ['Leave', 'Ctrl+Shift+H'],
      ]),
    ),
  ];
}

function defaultConfig(): ConfigDto {
  return {
    version: 1,
    revision: 0,
    notice: null,
    profiles: builtinProfiles(),
    display: { defaultView: 'buddy', secondaryView: 'system' },
    buddy: { reactions: true, intensity: 'normal' },
  };
}
let config: ConfigDto = defaultConfig();
const configListeners = new Set<(config: ConfigDto) => void>();

function saveConfig(
  next: Pick<ConfigDto, 'display' | 'buddy' | 'profiles'>,
  notice: ConfigDto['notice'],
): ConfigDto {
  config = { ...config, ...next, notice, revision: config.revision + 1 };
  for (const listener of configListeners) listener(config);
  return config;
}

/** Keeps the desk status in step with the active profile's bindings. */
function syncDesk(): void {
  const active = config.profiles.find((p) => p.id === deskStatus.profileId);
  if (!active) return;
  const action = (s: SlotDto | 'pin'): DeskStatusDto['holdAction'] =>
    s === 'pin' || !s.action ? null : s.action.kind === 'systemMute' ? 'mute' : s.action.kind;
  updateDesk({
    pressAction: action(active.press),
    holdAction: action(active.hold),
    buttonActions: [
      action(active.buttons[0].press),
      action(active.buttons[1].press),
      action(active.buttons[2].press),
    ],
    buttonHoldActions: [
      action(active.buttons[0].hold),
      action(active.buttons[1].hold),
      action(active.buttons[2].hold),
    ],
    rotateLabel: active.rotate.deviceLabel,
    buttonLabels: [
      active.buttons[0].press.deviceLabel,
      active.buttons[1].press.deviceLabel,
      active.buttons[2].press.deviceLabel,
    ],
  });
}

export function mockGetConfig(): ConfigDto {
  return config;
}

/** Mirrors native rules: a double-press view equal to the default is refused; moving the default
 *  view takes the device there. */
export function mockSetDisplaySettings(
  defaultView: DisplayMode,
  secondaryView: DisplayMode | 'cycle',
): ConfigDto {
  if (secondaryView === defaultView) {
    throw new Error('the double-press view must differ from the default view');
  }
  const moved = defaultView !== config.display.defaultView;
  const saved = saveConfig(
    { display: { defaultView, secondaryView }, buddy: config.buddy, profiles: config.profiles },
    null,
  );
  if (moved) updateDesk({ mode: defaultView });
  return saved;
}

export function mockSetBuddySettings(reactions: boolean, intensity: Intensity): ConfigDto {
  return saveConfig(
    { display: config.display, buddy: { reactions, intensity }, profiles: config.profiles },
    null,
  );
}

export function mockResetConfig(): ConfigDto {
  const fresh = defaultConfig();
  const saved = saveConfig(fresh, null);
  updateDesk({ mode: fresh.display.defaultView });
  syncDesk();
  return saved;
}

function mockLabel(label: string): string {
  const text = label.trim();
  if (!text) throw new Error('a label cannot be empty');
  if ([...text].length > 32) throw new Error('a label is at most 32 characters');
  if (/[^\u0020-\u007e\u00a0-\u00ff]/.test(text)) {
    throw new Error('a label can only use Latin-1 characters');
  }
  return text;
}

function mockShortcut(keys: string): string {
  if (!keys.trim() || keys.trim().endsWith('+')) throw new Error('not a valid shortcut');
  return keys.trim();
}

function mockAction(action: ActionSpec): ActionSpec {
  if (action.kind === 'shortcut') return { kind: 'shortcut', keys: mockShortcut(action.keys) };
  if (action.kind === 'launch') {
    if (!action.target.trim()) throw new Error('not a valid application');
    return { kind: 'launch', target: action.target.trim() };
  }
  return action;
}

function editProfile(
  id: ProfileId,
  edit: (profile: ProfileConfigDto, builtin: ProfileConfigDto) => void,
): ConfigDto {
  const original = builtinProfiles().find((p) => p.id === id);
  if (!original) throw new Error('unknown profile');
  const profiles = structuredClone(config.profiles);
  const profile = profiles.find((p) => p.id === id);
  if (!profile) throw new Error('unknown profile');
  edit(profile, original);
  const saved = saveConfig({ display: config.display, buddy: config.buddy, profiles }, null);
  syncDesk();
  return saved;
}

/** Mirrors native rules loosely: a reset (`null`) restores the built-in slot, an unbound slot
 *  drops its label, and labels and shortcuts are validated. */
export function mockSetBinding(
  profile: ProfileId,
  control: ControlRef,
  slot: SlotSpec | null,
): ConfigDto {
  const next = slot && {
    action: slot.action && mockAction(slot.action),
    label: slot.action && slot.label ? mockLabel(slot.label) : null,
  };
  return editProfile(profile, (target, builtin) => {
    const place = (get: (p: ProfileConfigDto) => SlotDto, put: (d: SlotDto) => void) => {
      put(
        next
          ? { ...slotOf(next.action, next.label), overridden: true }
          : structuredClone(get(builtin)),
      );
    };
    const button = (i: 0 | 1 | 2, which: 'press' | 'hold') =>
      place(
        (p) => p.buttons[i][which] as SlotDto,
        (d) => {
          target.buttons[i][which] = d;
        },
      );
    switch (control) {
      case 'press':
        return place(
          (p) => p.press,
          (d) => {
            target.press = d;
          },
        );
      case 'hold':
        return place(
          (p) => p.hold,
          (d) => {
            target.hold = d;
          },
        );
      case 'button1Press':
        return button(0, 'press');
      case 'button1Hold':
        return button(0, 'hold');
      case 'button2Press':
        return button(1, 'press');
      case 'button3Press':
        return button(2, 'press');
      case 'button3Hold':
        return button(2, 'hold');
    }
  });
}

function slotOf(action: ActionSpec | null, label: string | null): SlotDto {
  return slot(action, label);
}

export function mockSetRotate(profile: ProfileId, rotate: RotateSpec | null): ConfigDto {
  return editProfile(profile, (target, builtin) => {
    if (!rotate) {
      target.rotate = structuredClone(builtin.rotate);
      return;
    }
    const spec: RotateSpec =
      rotate.kind === 'shortcuts'
        ? {
            kind: 'shortcuts',
            cw: mockShortcut(rotate.cw),
            ccw: mockShortcut(rotate.ccw),
            label: mockLabel(rotate.label),
          }
        : rotate;
    target.rotate = {
      spec,
      deviceLabel: spec.kind === 'shortcuts' ? spec.label : 'Volume',
      overridden: true,
    };
  });
}

export function mockResetProfile(profile: ProfileId): ConfigDto {
  return editProfile(profile, (target, builtin) => {
    Object.assign(target, structuredClone(builtin));
  });
}

export function mockOnConfigChanged(handler: (config: ConfigDto) => void): () => void {
  configListeners.add(handler);
  return () => {
    configListeners.delete(handler);
  };
}

export function mockOnDeskStatus(handler: (status: DeskStatusDto) => void): () => void {
  deskListeners.add(handler);
  handler(deskStatus);
  return () => {
    deskListeners.delete(handler);
  };
}

const STATE_TINT: Record<CompanionState, readonly [number, number, number]> = {
  booting: [90, 120, 200],
  idle: [80, 170, 220],
  happy: [250, 205, 70],
  busy: [235, 130, 60],
  sleeping: [120, 110, 200],
  offline: [90, 90, 100],
};

// Deterministic placeholder frame so the browser preview shows something without the real renderer:
// a face-tinted disc that bobs with `elapsedMs`. Real frames come from the shared Rust renderer.
export function mockPreviewFrame(state: CompanionState, elapsedMs: number): Uint8ClampedArray {
  const dim = PREVIEW_DIM;
  const rgba = new Uint8ClampedArray(dim * dim * 4);
  const [fr, fg, fb] = STATE_TINT[state];
  const phase = (elapsedMs % 1000) / 1000;
  const cx = dim / 2;
  const cy = dim / 2 + Math.sin(phase * Math.PI * 2) * 18;
  const radius = 72;
  for (let y = 0; y < dim; y += 1) {
    for (let x = 0; x < dim; x += 1) {
      const i = (y * dim + x) * 4;
      const inFace = Math.hypot(x - cx, y - cy) < radius;
      rgba[i] = inFace ? fr : 12;
      rgba[i + 1] = inFace ? fg : 12;
      rgba[i + 2] = inFace ? fb : 16;
      rgba[i + 3] = 255;
    }
  }
  return rgba;
}

/** Dev-browser stand-in for the native preview stream: a capped interval pushing mock frames. */
export function mockPreviewStream(
  state: CompanionState,
  fps: number,
  onFrame: (frame: Uint8ClampedArray) => void,
): { close: () => Promise<void> } {
  const interval = Math.max(1000 / Math.min(Math.max(fps, 1), 30), 1);
  let step = 0;
  const timer = setInterval(() => {
    onFrame(mockPreviewFrame(state, Math.round((step * 1000) / 30)));
    step += 1;
  }, interval);
  return {
    close: (): Promise<void> => {
      clearInterval(timer);
      return Promise.resolve();
    },
  };
}
