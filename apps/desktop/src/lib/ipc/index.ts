// Typed IPC wrappers — the frontend's ONLY door to the native core (contracts/ipc.md §1).
//
// Selection is decided at the module boundary, not merely at runtime (T124):
//   • Inside Tauri  → real `invoke`/`listen`.
//   • DEV browser   → the lazily-imported `./mock` (the static `import.meta.env.DEV` guard lets Vite
//                     drop `./mock` from production bundles).
//   • Production, not Tauri → throw. Production NEVER silently falls back to mock behaviour.

import type {
  ActionCatalogEntryDto,
  ActionSpec,
  AppInfoDto,
  CompanionState,
  ConnectionStatusDto,
  ActivityEventDto,
  SendableState,
  AnimationTimeline,
  FirmwareStatusDto,
  MascotAction,
  ConfigDto,
  DiagnosticsDto,
  ControlRef,
  ProfileId,
  RotateSpec,
  SlotSpec,
  MacroSpec,
  DeskStatusDto,
  DisplayMode,
  Intensity,
} from './types';
import {
  isActivityEvent,
  parseCatalog,
  parseConfig,
  parseConnectionStatus,
  parseDeskStatus,
  parseDiagnostics,
} from './validate';

/// Handle returned by an event subscription; call it to unsubscribe.
export type Unlisten = () => void;

/// A live preview-frame stream (contracts/ipc.md §3). `close()` cancels it natively.
export interface PreviewStream {
  close: () => Promise<void>;
  update?: (animation: AnimationTimeline, elapsedMs: number) => Promise<void>;
}

/// True when running inside the Tauri webview (the core injects this global in v2).
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const core = await import('@tauri-apps/api/core');
  return core.invoke<T>(command, args);
}

// Dev-only browser mock, loaded lazily and ONLY in dev builds. The static `import.meta.env.DEV` guard
// is compiled to `false` in production, so bundlers eliminate the `./mock` import from prod output.
async function devMock(): Promise<typeof import('./mock')> {
  if (!import.meta.env.DEV) {
    throw new Error(
      'Kivori: the native runtime is unavailable and there is no mock in production.',
    );
  }
  return import('./mock');
}

function unavailable(): never {
  throw new Error('Kivori: the native runtime is unavailable.');
}

export async function getAppInfo(): Promise<AppInfoDto> {
  if (isTauri()) return invoke<AppInfoDto>('get_app_info');
  if (import.meta.env.DEV) return (await devMock()).mockAppInfo();
  return unavailable();
}

export async function getConnectionStatus(): Promise<ConnectionStatusDto> {
  if (isTauri()) return parseConnectionStatus(await invoke<unknown>('get_connection_status'));
  if (import.meta.env.DEV) return (await devMock()).mockConnectionStatus();
  return unavailable();
}

/** Firmware status belongs to the native runtime and survives Overview navigation. */
export async function getFirmwareStatus(): Promise<FirmwareStatusDto> {
  if (isTauri()) return invoke<FirmwareStatusDto>('get_firmware_status');
  if (import.meta.env.DEV) {
    return {
      available: false,
      phase: 'idle',
      message: 'Open the native Kivori app to flash firmware.',
      imageSize: 0,
    };
  }
  return unavailable();
}

/** Installs only the bundled firmware on the already-connected device. */
export async function flashFirmware(): Promise<void> {
  if (isTauri()) return invoke<void>('flash_firmware');
  return unavailable();
}

export async function listStates(): Promise<CompanionState[]> {
  if (isTauri()) return invoke<CompanionState[]>('list_states');
  if (import.meta.env.DEV) return (await devMock()).mockListStates();
  return unavailable();
}

export async function setDesiredState(state: SendableState): Promise<void> {
  if (isTauri()) return invoke<void>('set_desired_state', { state });
  if (import.meta.env.DEV) return;
  return unavailable();
}

/** Versions, connection health, host services and a config summary. Safe to copy and share. */
export async function getDiagnostics(): Promise<DiagnosticsDto> {
  if (isTauri()) return parseDiagnostics(await invoke<unknown>('get_diagnostics'));
  if (import.meta.env.DEV) return (await devMock()).mockGetDiagnostics();
  return unavailable();
}

/** The saved settings (display and buddy). */
export async function getConfig(): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('get_config'));
  if (import.meta.env.DEV) return (await devMock()).mockGetConfig();
  return unavailable();
}

/** Saves the home view and what a double press shows; rejects with the native error string. */
export async function setDisplaySettings(
  defaultView: DisplayMode,
  secondaryView: DisplayMode | 'cycle',
): Promise<ConfigDto> {
  if (isTauri()) {
    return parseConfig(
      await invoke<unknown>('set_display_settings', { defaultView, secondaryView }),
    );
  }
  if (import.meta.env.DEV) {
    return (await devMock()).mockSetDisplaySettings(defaultView, secondaryView);
  }
  return unavailable();
}

/** Saves the buddy's reactions and intensity; rejects with the native error string. */
export async function setBuddySettings(
  reactions: boolean,
  intensity: Intensity,
): Promise<ConfigDto> {
  if (isTauri()) {
    return parseConfig(await invoke<unknown>('set_buddy_settings', { reactions, intensity }));
  }
  if (import.meta.env.DEV) return (await devMock()).mockSetBuddySettings(reactions, intensity);
  return unavailable();
}

/** Rebinds one control of a profile; `slot: null` resets it to the built-in. Rejects with the native error string. */
export async function setBinding(
  profile: ProfileId,
  control: ControlRef,
  slot: SlotSpec | null,
): Promise<ConfigDto> {
  if (isTauri()) {
    return parseConfig(await invoke<unknown>('set_binding', { profile, control, slot }));
  }
  if (import.meta.env.DEV) return (await devMock()).mockSetBinding(profile, control, slot);
  return unavailable();
}

/** Changes what the knob does in a profile; `rotate: null` resets it. */
export async function setRotate(profile: ProfileId, rotate: RotateSpec | null): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('set_rotate', { profile, rotate }));
  if (import.meta.env.DEV) return (await devMock()).mockSetRotate(profile, rotate);
  return unavailable();
}

/** Creates or replaces a macro by its id; rejects with the native error string. */
export async function saveMacro(spec: MacroSpec): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('save_macro', { spec }));
  if (import.meta.env.DEV) return (await devMock()).mockSaveMacro(spec);
  return unavailable();
}

/** Deletes a macro; refused (rejects) while a control is bound to it. */
export async function deleteMacro(id: string): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('delete_macro', { id }));
  if (import.meta.env.DEV) return (await devMock()).mockDeleteMacro(id);
  return unavailable();
}

/** Drops every override of one profile. */
export async function resetProfile(profile: ProfileId): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('reset_profile', { profile }));
  if (import.meta.env.DEV) return (await devMock()).mockResetProfile(profile);
  return unavailable();
}

/** Restores every setting to its default (the previous file is kept as one backup). */
export async function resetConfig(): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('reset_config'));
  if (import.meta.env.DEV) return (await devMock()).mockResetConfig();
  return unavailable();
}

/** Subscribes to saved or reset settings; a payload with an unknown token is dropped. */
export async function onConfigChanged(handler: (config: ConfigDto) => void): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('config://changed', (event) => {
      try {
        handler(parseConfig(event.payload));
      } catch {
        // Invalid payloads never reach the UI.
      }
    });
  }
  if (import.meta.env.DEV) return (await devMock()).mockOnConfigChanged(handler);
  return unavailable();
}

/** Plays one social reaction on a compatible connected device. */
export async function playMascotAction(action: MascotAction): Promise<void> {
  if (isTauri()) return invoke<void>('play_mascot_action', { action });
  if (import.meta.env.DEV) return;
  return unavailable();
}

/** Returns up to `limit` typed, safe events from this native process session. */
export async function getActivityLog(limit: number): Promise<ActivityEventDto[]> {
  if (isTauri()) {
    const events = await invoke<unknown[]>('get_activity_log', { limit });
    return events.filter(isActivityEvent);
  }
  if (import.meta.env.DEV) return (await devMock()).mockActivityLog();
  return unavailable();
}

/**
 * Fetches a rendered preview frame as RGBA8888 (240×240, row-major) — the canonical RGB565→RGBA
 * expansion happens in Rust (constraint 2). Dev-only command; used for scrub/step, where an exact
 * frame for an exact millisecond is required (SC-011).
 */
export async function renderPreviewFrame(
  state: CompanionState,
  elapsedMs: number,
  animation?: AnimationTimeline,
): Promise<Uint8ClampedArray> {
  if (isTauri()) {
    const buffer = await invoke<ArrayBuffer>('render_preview_frame', {
      state,
      elapsedMs: Math.round(elapsedMs),
      animation,
    });
    return new Uint8ClampedArray(buffer);
  }
  if (import.meta.env.DEV) return (await devMock()).mockPreviewFrame(state, elapsedMs);
  return unavailable();
}

/** Device Studio affordance: identical to {@link setDesiredState} but labelled as a developer action. */
export async function mirrorState(state: SendableState): Promise<void> {
  if (isTauri()) return invoke<void>('mirror_state', { state });
  if (import.meta.env.DEV) return;
  return unavailable();
}

/** Subscribes to connection-status changes; resolves to an unsubscribe handle. */
export async function onConnectionStatus(
  handler: (status: ConnectionStatusDto) => void,
): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('connection://status', (event) => {
      try {
        handler(parseConnectionStatus(event.payload));
      } catch {
        // Invalid payloads never reach the UI.
      }
    });
  }
  if (import.meta.env.DEV) {
    handler((await devMock()).mockConnectionStatus());
    return () => {};
  }
  return unavailable();
}

/** Subscribes to native-issued typed session-activity events. */
export async function onActivityLog(
  handler: (activity: ActivityEventDto) => void,
): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('activity-log://event', (event) => {
      if (isActivityEvent(event.payload)) handler(event.payload);
    });
  }
  if (import.meta.env.DEV) return () => {};
  return unavailable();
}

/** The current desk projection (display mode, monitored values, last action outcome). */
export async function getDeskStatus(): Promise<DeskStatusDto> {
  if (isTauri()) return parseDeskStatus(await invoke<unknown>('get_desk_status'));
  if (import.meta.env.DEV) return (await devMock()).mockDeskStatus();
  return unavailable();
}

/** Selects the device's full-screen view; rejects with the native error string. */
export async function setDisplayMode(mode: DisplayMode): Promise<void> {
  if (isTauri()) return invoke<void>('set_display_mode', { mode });
  if (import.meta.env.DEV) return (await devMock()).mockSetDisplayMode(mode);
  return unavailable();
}

/** Every bindable action: how each is verified and whether it is available here. */
export async function listActionCatalog(): Promise<ActionCatalogEntryDto[]> {
  if (isTauri()) return parseCatalog(await invoke<unknown>('list_action_catalog'));
  if (import.meta.env.DEV) return (await devMock()).mockListActionCatalog();
  return unavailable();
}

/**
 * Tries one action now, as if its control fired; rejects with the native error string. The outcome
 * arrives in the desk status `lastAction`. Under a protected foreground only system actions run.
 */
export async function testAction(action: ActionSpec): Promise<void> {
  if (isTauri()) return invoke<void>('test_action', { action });
  if (import.meta.env.DEV) return (await devMock()).mockTestAction(action);
  return unavailable();
}

/** Subscribes to desk projection changes; a payload with an unknown token is dropped. */
export async function onDeskStatus(handler: (status: DeskStatusDto) => void): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('desk://status', (event) => {
      try {
        handler(parseDeskStatus(event.payload));
      } catch {
        // Invalid payloads never reach the UI.
      }
    });
  }
  if (import.meta.env.DEV) return (await devMock()).mockOnDeskStatus(handler);
  return unavailable();
}

/**
 * Opens a native preview-frame stream at a capped `fps` (contracts/ipc.md §3). Each frame arrives as
 * raw RGBA8888 straight from the shared renderer and is acknowledged, which is what releases the
 * native back-pressure slot. The canvas only blits these bytes — no frame is produced in TypeScript.
 */
export async function openPreviewStream(
  state: CompanionState,
  fps: number,
  onFrame: (frame: Uint8ClampedArray) => void,
  animation?: AnimationTimeline,
  elapsedMs = 0,
): Promise<PreviewStream> {
  if (isTauri()) {
    const { Channel } = await import('@tauri-apps/api/core');
    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (buffer): void => {
      onFrame(new Uint8ClampedArray(buffer));
      // Acknowledge so the native producer may render the next frame (bounded in-flight frames).
      void invoke<boolean>('ack_preview_frame', { handle: channel.id }).catch(() => {});
    };
    const handle = await invoke<number>('open_preview_stream', {
      state,
      fps,
      channel,
      animation,
      elapsedMs: Math.round(elapsedMs),
    });
    return {
      update: async (animation, elapsedMs): Promise<void> => {
        await invoke('update_preview_stream', {
          handle,
          animation,
          elapsedMs: Math.round(elapsedMs),
        });
      },
      close: async (): Promise<void> => {
        await invoke<boolean>('close_preview_stream', { handle });
      },
    };
  }
  if (import.meta.env.DEV) return (await devMock()).mockPreviewStream(state, fps, onFrame);
  return unavailable();
}
