// Browser development IPC mock — DEV builds ONLY.
//
// This module is imported lazily behind a static `import.meta.env.DEV` guard (see `./index.ts`), so
// Vite/Rollup drop it entirely from production Tauri bundles (T124). The sentinel below is asserted
// absent from the production build by `scripts/check-mock-excluded.mjs`.

import type {
  ActivityEventDto,
  AppInfoDto,
  CompanionState,
  ConnectionStatusDto,
  DeskStatusDto,
  DisplayMode,
  TestActionRequest,
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

/** Mirrors native rules loosely: a shortcut/launch needs its text; the outcome is "unverified". */
export function mockRunTestAction(request: TestActionRequest): void {
  if (request.action === 'shortcut' && !request.shortcut?.trim()) {
    throw new Error('a shortcut action needs a shortcut');
  }
  if (request.action === 'launch' && !request.target?.trim()) {
    throw new Error('a launch action needs an application');
  }
  const result = request.action === 'mute' ? 'stateConfirmed' : 'unverified';
  updateDesk({
    muted: request.action === 'mute' ? !deskStatus.muted : deskStatus.muted,
    lastAction: { action: request.action, result, permissionRequired: false },
  });
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
