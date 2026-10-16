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
  FirmwareOfferDto,
  FirmwareStatusDto,
  StartupSettingsDto,
  UpdateStatusDto,
  OnboardingDto,
  AccessibilityState,
  MascotAction,
  ConfigDto,
  DeviceInternalsDto,
  DiagnosticsDto,
  AppRefDto,
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
  parseDeviceInternals,
  parseDiagnostics,
  parseFirmwareOffer,
  parseFirmwareStatus,
  parseStartupSettings,
  parseUpdateStatus,
  parseOnboarding,
  parseAccessibility,
  appRef,
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

/**
 * Tells the native core the first screen is ready, so it shows the main window (and closes the
 * startup splash). A no-op outside Tauri; a failure is swallowed because the core shows the window
 * after a timeout anyway.
 */
export async function appReady(): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke<void>('app_ready');
  } catch {
    // The native fallback timer covers a missed signal.
  }
}

export async function getConnectionStatus(): Promise<ConnectionStatusDto> {
  if (isTauri()) return parseConnectionStatus(await invoke<unknown>('get_connection_status'));
  if (import.meta.env.DEV) return (await devMock()).mockConnectionStatus();
  return unavailable();
}

/** Firmware status belongs to the native runtime and survives Overview navigation. */
export async function getFirmwareStatus(): Promise<FirmwareStatusDto> {
  if (isTauri()) return parseFirmwareStatus(await invoke<unknown>('get_firmware_status'));
  if (import.meta.env.DEV) return (await devMock()).mockFirmwareStatus();
  return unavailable();
}

/** Whether Kivori starts at login, as the OS reports it. */
export async function getStartupSettings(): Promise<StartupSettingsDto> {
  if (isTauri()) return parseStartupSettings(await invoke<unknown>('get_startup_settings'));
  if (import.meta.env.DEV) return (await devMock()).mockGetStartupSettings();
  return unavailable();
}

/** Subscribes to launch-at-login changes (the tray item or this app); an invalid payload is dropped. */
export async function onStartupChanged(
  handler: (settings: StartupSettingsDto) => void,
): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('startup://changed', (event) => {
      try {
        handler(parseStartupSettings(event.payload));
      } catch {
        // Invalid payloads never reach the UI.
      }
    });
  }
  if (import.meta.env.DEV) return (await devMock()).mockOnStartupChanged(handler);
  return unavailable();
}

/** Adds or removes the OS login entry; rejects with the native error string. */
export async function setLaunchAtLogin(enabled: boolean): Promise<StartupSettingsDto> {
  if (isTauri()) {
    return parseStartupSettings(await invoke<unknown>('set_launch_at_login', { enabled }));
  }
  if (import.meta.env.DEV) return (await devMock()).mockSetLaunchAtLogin(enabled);
  return unavailable();
}

/** The app updater's status (ADR-0015). */
export async function getUpdateStatus(): Promise<UpdateStatusDto> {
  if (isTauri()) return parseUpdateStatus(await invoke<unknown>('get_update_status'));
  if (import.meta.env.DEV) return (await devMock()).mockGetUpdateStatus();
  return unavailable();
}

/** Subscribes to updater changes (`updater://status`); an invalid payload is dropped. */
export async function onUpdateStatus(
  handler: (status: UpdateStatusDto) => void,
): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('updater://status', (event) => {
      try {
        handler(parseUpdateStatus(event.payload));
      } catch {
        // Invalid payloads never reach the UI.
      }
    });
  }
  if (import.meta.env.DEV) return (await devMock()).mockOnUpdateStatus(handler);
  return unavailable();
}

/** The tray's "Update to vX.Y.Z…" asks the window to show the update dialog. */
export async function onUpdateDialogRequested(handler: () => void): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('updater://show', () => handler());
  }
  if (import.meta.env.DEV) return () => {};
  return unavailable();
}

async function updateCommand(
  command: string,
  mock: (m: typeof import('./mock')) => UpdateStatusDto,
  args?: Record<string, unknown>,
): Promise<UpdateStatusDto> {
  if (isTauri()) return parseUpdateStatus(await invoke<unknown>(command, args));
  if (import.meta.env.DEV) return mock(await devMock());
  return unavailable();
}

/** "Check now"; the result arrives as `updater://status`. Offline says nothing. */
export function checkForUpdates(): Promise<UpdateStatusDto> {
  return updateCommand('check_for_updates', (m) => m.mockCheckForUpdates());
}

/** "Install & restart" (and "Try again"). */
export function installUpdate(): Promise<UpdateStatusDto> {
  return updateCommand('install_update', (m) => m.mockInstallUpdate());
}

/** "Later": asks again in 3 days. */
export function snoozeUpdate(): Promise<UpdateStatusDto> {
  return updateCommand('snooze_update', (m) => m.mockDismissUpdate());
}

/** "Skip this version". */
export function skipUpdateVersion(): Promise<UpdateStatusDto> {
  return updateCommand('skip_update_version', (m) => m.mockDismissUpdate());
}

/** "Check for updates automatically". */
export function setAutoUpdateCheck(enabled: boolean): Promise<UpdateStatusDto> {
  return updateCommand('set_auto_update_check', (m) => m.mockSetAutoUpdateCheck(enabled), {
    enabled,
  });
}

/** Whether first-run setup is finished. */
export async function getOnboarding(): Promise<OnboardingDto> {
  if (isTauri()) return parseOnboarding(await invoke<unknown>('get_onboarding'));
  if (import.meta.env.DEV) return (await devMock()).mockGetOnboarding();
  return unavailable();
}

/** Marks first-run setup finished (Finish and Skip setup both end here). */
export async function completeOnboarding(): Promise<OnboardingDto> {
  if (isTauri()) return parseOnboarding(await invoke<unknown>('complete_onboarding'));
  if (import.meta.env.DEV) return (await devMock()).mockSetOnboarding(true);
  return unavailable();
}

/** Reopens first-run setup (the Device page's "Run setup again"). */
export async function restartOnboarding(): Promise<OnboardingDto> {
  if (isTauri()) return parseOnboarding(await invoke<unknown>('restart_onboarding'));
  if (import.meta.env.DEV) return (await devMock()).mockSetOnboarding(false);
  return unavailable();
}

/** The Accessibility permission now. Never prompts. */
export async function getAccessibility(): Promise<AccessibilityState> {
  if (isTauri()) return parseAccessibility(await invoke<unknown>('get_accessibility'));
  if (import.meta.env.DEV) return (await devMock()).mockGetAccessibility();
  return unavailable();
}

/** Asks macOS to show its Accessibility prompt; resolves with the state right after. */
export async function requestAccessibility(): Promise<AccessibilityState> {
  if (isTauri()) return parseAccessibility(await invoke<unknown>('request_accessibility'));
  if (import.meta.env.DEV) return (await devMock()).mockGetAccessibility();
  return unavailable();
}

/** Opens System Settings at the Accessibility list. */
export async function openAccessibilitySettings(): Promise<void> {
  if (isTauri()) return invoke<void>('open_accessibility_settings');
  if (import.meta.env.DEV) return;
  return unavailable();
}

/** The firmware update offer (ADR-0015, step 6). */
export async function getFirmwareOffer(): Promise<FirmwareOfferDto> {
  if (isTauri()) return parseFirmwareOffer(await invoke<unknown>('get_firmware_offer'));
  if (import.meta.env.DEV) return (await devMock()).mockGetFirmwareOffer();
  return unavailable();
}

/** Subscribes to offer changes (`firmware://offer`); an invalid payload is dropped. */
export async function onFirmwareOffer(
  handler: (offer: FirmwareOfferDto) => void,
): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('firmware://offer', (event) => {
      try {
        handler(parseFirmwareOffer(event.payload));
      } catch {
        // Invalid payloads never reach the UI.
      }
    });
  }
  if (import.meta.env.DEV) return (await devMock()).mockOnFirmwareOffer(handler);
  return unavailable();
}

/** The tray's "Update Kivori firmware…" asks the window to show the firmware offer dialog. */
export async function onFirmwareOfferRequested(handler: () => void): Promise<Unlisten> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<unknown>('firmware://show-offer', () => handler());
  }
  if (import.meta.env.DEV) return () => {};
  return unavailable();
}

/** "Update now" on the offer: the same flash as `flashFirmware`; progress is in the firmware status. */
export async function acceptFirmwareOffer(): Promise<void> {
  if (isTauri()) return invoke<void>('accept_firmware_offer');
  if (import.meta.env.DEV) return (await devMock()).mockAcceptFirmwareOffer();
  return unavailable();
}

/** "Later" on an optional offer: asked again when the device next connects. */
export async function dismissFirmwareOffer(): Promise<FirmwareOfferDto> {
  if (isTauri()) return parseFirmwareOffer(await invoke<unknown>('dismiss_firmware_offer'));
  if (import.meta.env.DEV) return (await devMock()).mockDismissFirmwareOffer();
  return unavailable();
}

/** Installs only the bundled firmware on the already-connected device. */
export async function flashFirmware(): Promise<void> {
  if (isTauri()) return invoke<void>('flash_firmware');
  return unavailable();
}

/**
 * Restores the bundled firmware on the one Kivori device present, even one that cannot connect
 * (wrong firmware, or held in download mode with the BOOT button). Takes no port. `product` is a
 * restore choice's id, sent only after the app asked which product the device is.
 */
export async function restoreFirmware(product?: string): Promise<void> {
  if (isTauri()) return invoke<void>('restore_firmware', { product: product ?? null });
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

/**
 * The developer-mode Device internals card. Asks the device for a fresh snapshot and returns the
 * latest one it answered with, so call it about once a second, and only while the card is visible.
 */
export async function getDeviceInternals(): Promise<DeviceInternalsDto> {
  if (isTauri()) return parseDeviceInternals(await invoke<unknown>('get_device_internals'));
  if (import.meta.env.DEV) return (await devMock()).mockGetDeviceInternals();
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

/** Saves the backlight brightness step (1 to 5); the device follows at once. */
export async function setBrightness(step: number): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('set_brightness', { step }));
  if (import.meta.env.DEV) return (await devMock()).mockSetBrightness(step);
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

/** Creates a custom profile for an app (a copy of General's bindings, made now). `name` is the
 *  app's display name; the app moves out of any profile that held it. */
export async function addCustomProfile(appId: string, name: string): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('add_custom_profile', { appId, name }));
  if (import.meta.env.DEV) return (await devMock()).mockAddCustomProfile(appId, name);
  return unavailable();
}

/** Moves an app into a profile (`'general'` = out of the profile that lists it). */
export async function assignApp(profile: ProfileId, appId: string): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('assign_app', { profile, appId }));
  if (import.meta.env.DEV) return (await devMock()).mockAssignApp(profile, appId);
  return unavailable();
}

/** Renames a custom profile. */
export async function renameProfile(profile: ProfileId, name: string): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('rename_profile', { profile, name }));
  if (import.meta.env.DEV) return (await devMock()).mockRenameProfile(profile, name);
  return unavailable();
}

/** Deletes a custom profile; its apps stop matching it. */
export async function deleteProfile(profile: ProfileId): Promise<ConfigDto> {
  if (isTauri()) return parseConfig(await invoke<unknown>('delete_profile', { profile }));
  if (import.meta.env.DEV) return (await devMock()).mockDeleteProfile(profile);
  return unavailable();
}

/** Puts a custom profile in or out of the middle button's pin cycle. */
export async function setProfilePinCycle(profile: ProfileId, enabled: boolean): Promise<ConfigDto> {
  if (isTauri()) {
    return parseConfig(await invoke<unknown>('set_profile_pin_cycle', { profile, enabled }));
  }
  if (import.meta.env.DEV) return (await devMock()).mockSetProfilePinCycle(profile, enabled);
  return unavailable();
}

/** The apps with a window open now. Resolves to an empty list where this OS cannot say. */
export async function listRunningApps(): Promise<AppRefDto[]> {
  if (isTauri()) {
    const raw = await invoke<unknown>('list_running_apps');
    if (!Array.isArray(raw)) throw new Error('Kivori: invalid running apps.');
    return raw.map(appRef);
  }
  if (import.meta.env.DEV) return (await devMock()).mockListRunningApps();
  return unavailable();
}

const productArt = new Map<string, Promise<string | null>>();

/** Compiled, immutable artwork only. Missing/failed lookups keep the neutral figure. */
export function getProductArt(product: string): Promise<string | null> {
  const cached = productArt.get(product);
  if (cached) return cached;
  const lookup = (async (): Promise<string | null> => {
    if (isTauri()) {
      const url = await invoke<unknown>('get_product_art', { product });
      return typeof url === 'string' &&
        url.length <= 700_000 &&
        /^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(url)
        ? url
        : null;
    }
    return null;
  })().catch(() => {
    // A later lookup can recover from a transient IPC failure.
    productArt.delete(product);
    return null;
  });
  productArt.set(product, lookup);
  return lookup;
}

const icons = new Map<string, Promise<string | null>>();

/**
 * An app's icon as a PNG data URL, `null` when none can be read (show a letter). One lookup per
 * app id: the native side caches too, and a failure is simply a letter, never an error.
 */
export function getAppIcon(appId: string): Promise<string | null> {
  const known = icons.get(appId);
  if (known) return known;
  const lookup = (async (): Promise<string | null> => {
    try {
      if (isTauri()) {
        const url = await invoke<unknown>('get_app_icon', { appId });
        return typeof url === 'string' && url.startsWith('data:image/png;base64,') ? url : null;
      }
      if (import.meta.env.DEV) return (await devMock()).mockGetAppIcon(appId);
    } catch {
      // Nothing to show but the letter.
    }
    return null;
  })();
  icons.set(appId, lookup);
  // A miss may be an app that has not started yet: ask again next time.
  void lookup.then((url) => url ?? icons.delete(appId));
  return lookup;
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

/**
 * Tries the knob's binding once. A volume knob steps one detent up and back, so the volume ends
 * where it started; two shortcuts send the clockwise one once. The outcome arrives in the desk
 * status; rejects with the native error string.
 */
export async function testRotate(rotate: RotateSpec): Promise<void> {
  if (isTauri()) return invoke<void>('test_rotate', { rotate });
  if (import.meta.env.DEV) return (await devMock()).mockTestRotate(rotate);
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
