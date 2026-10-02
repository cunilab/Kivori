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

export function mockConnectionStatus(): ConnectionStatusDto {
  return {
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
}

export function mockListStates(): CompanionState[] {
  return [...COMPANION_STATES];
}

export function mockActivityLog(): ActivityEventDto[] {
  return [];
}

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
  lastAction: null,
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
