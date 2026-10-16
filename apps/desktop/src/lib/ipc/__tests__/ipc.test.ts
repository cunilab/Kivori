import { afterEach, describe, expect, it, vi } from 'vitest';

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: tauri.listen }));
import {
  flashFirmware,
  getFirmwareStatus,
  appReady,
  getAppInfo,
  getOnboarding,
  completeOnboarding,
  restartOnboarding,
  getAccessibility,
  requestAccessibility,
  getActivityLog,
  getConfig,
  getConnectionStatus,
  isTauri,
  onConfigChanged,
  onStartupChanged,
  setLaunchAtLogin,
  resetConfig,
  saveMacro,
  deleteMacro,
  resetProfile,
  setBinding,
  setBuddySettings,
  setRotate,
  setDisplaySettings,
  listStates,
  onActivityLog,
  getDeskStatus,
  onDeskStatus,
  listActionCatalog,
  testAction,
  testRotate,
  setDisplayMode,
  renderPreviewFrame,
  addCustomProfile,
  assignApp,
  renameProfile,
  deleteProfile,
  setProfilePinCycle,
  listRunningApps,
  getAppIcon,
  getProductArt,
  getUpdateStatus,
  onUpdateStatus,
  onUpdateDialogRequested,
  checkForUpdates,
  installUpdate,
  snoozeUpdate,
  skipUpdateVersion,
  setAutoUpdateCheck,
  getFirmwareOffer,
  onFirmwareOffer,
  onFirmwareOfferRequested,
  acceptFirmwareOffer,
  dismissFirmwareOffer,
} from '../index';
import { mockGetConfig } from '../mock';
import vocabulary from './vocabulary.json';
import {
  ACTION_SPEC_KINDS,
  CAPABILITY_NAMES,
  COMPANION_STATES,
  CONTROL_REFS,
  DESK_ACTIONS,
  HOST_AVAILABILITIES,
  CHIP_MODELS,
  HOST_FOCUS,
  RESET_REASONS,
  FLASH_FAILURES,
  HOST_PRESENCES,
  HOST_INPUT_PERMISSIONS,
  HOST_MEDIA,
  PREVIEW_DIM,
  PROFILE_IDS,
  PROFILE_LIMITS,
  ROTATE_SPEC_KINDS,
  STEP_SPEC_KINDS,
  ACCESSIBILITY_STATES,
  STARTUP_PLATFORMS,
  UPDATE_ADVICE,
  UPDATE_DISABLED_REASONS,
  UPDATE_STATES,
  FIRMWARE_OFFER_KINDS,
  FIRMWARE_REQUIRED_REASONS,
} from '../types';
import type { MacroSpec } from '../types';
import {
  MAX_MEDIA_TEXT,
  parseCatalog,
  parseConfig,
  parseConnectionStatus,
  parseDiagnostics,
  parseFirmwareStatus,
  parseUpdateStatus,
  parseFirmwareOffer,
} from '../validate';

afterEach(() => {
  delete (window as Window & { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
  tauri.invoke.mockReset();
  tauri.listen.mockReset();
});

describe('ipc wrappers (browser mock fallback)', () => {
  it('never offers or pretends to flash a device from the browser mock', async () => {
    expect(await getFirmwareStatus()).toMatchObject({ available: false, imageSize: 0 });
    await expect(flashFirmware()).rejects.toThrow('native runtime is unavailable');
  });
  it('is not running inside Tauri under jsdom', () => {
    expect(isTauri()).toBe(false);
  });

  it('reports Device Studio enabled in the dev mock', async () => {
    const info = await getAppInfo();
    expect(info.deviceStudioEnabled).toBe(true);
    // The protocol restarted at 0.1 before the first release; the mock speaks the same version.
    expect(info.protocolVersion).toEqual({ major: 0, minor: 1 });
    expect(info.supportedMajors).toEqual([info.protocolVersion.major]);
  });

  it('renders a full 240x240 opaque RGBA frame', async () => {
    const frame = await renderPreviewFrame('idle', 0);
    expect(frame).toBeInstanceOf(Uint8ClampedArray);
    expect(frame.length).toBe(PREVIEW_DIM * PREVIEW_DIM * 4);
    expect(frame[3]).toBe(255);
  });

  it('lists every companion state', async () => {
    expect(await listStates()).toEqual([...COMPANION_STATES]);
  });

  it('returns a default disconnected status', async () => {
    const status = await getConnectionStatus();
    expect(status.connection).toBe('disconnected');
    expect(status.desired).toBe('idle');
  });

  it('uses the exact typed activity-log command and event names in Tauri', async () => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} });
    tauri.invoke.mockResolvedValue([]);
    const unlisten = vi.fn();
    tauri.listen.mockResolvedValue(unlisten);

    await expect(getActivityLog(17)).resolves.toEqual([]);
    const handler = vi.fn();
    await expect(onActivityLog(handler)).resolves.toBe(unlisten);

    expect(tauri.invoke).toHaveBeenCalledWith('get_activity_log', { limit: 17 });
    expect(tauri.listen).toHaveBeenCalledWith('activity-log://event', expect.any(Function));
  });
});

const validDesk = {
  mode: 'clock',
  volumePercent: 40,
  muted: null,
  media: null,
  cpuPercent: 12,
  ramPercent: null,
  highLoad: false,
  pressAction: 'playPause',
  holdAction: 'mute',
  doublePressAction: 'nextView',
  buttonActions: ['previousTrack', 'playPause', 'nextTrack'],
  profile: null,
  pinned: false,
  rotateLabel: 'Volume',
  buttonLabels: ['Previous', 'Play/Pause', 'Next'],
  buttonHoldActions: [null, null, null],
  profileId: 'general',
  mediaTitle: 'Weightless',
  mediaArtist: '',
  lastAction: { action: 'shortcut', result: 'unverified', permissionRequired: true },
  mic: {
    muted: 'partial',
    inUse: null,
    live: false,
    devices: [
      { name: 'Built-in', state: 'muted' },
      { name: 'USB headset', state: 'live' },
    ],
  },
  recentApp: { id: 'figma.exe', name: 'Figma' },
};

describe('app ready signal', () => {
  it('does nothing in a plain browser', async () => {
    await appReady();
    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  it('calls the native app_ready command in Tauri, and never throws if it fails', async () => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} });
    tauri.invoke.mockResolvedValue(undefined);
    await appReady();
    expect(tauri.invoke).toHaveBeenCalledWith('app_ready', undefined);
    tauri.invoke.mockRejectedValue(new Error('boom'));
    await expect(appReady()).resolves.toBeUndefined();
  });
});

describe('desk ipc (Tauri)', () => {
  const enterTauri = (): void => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} });
  };

  it('passes a valid status, nulls included, and uses the exact command names', async () => {
    enterTauri();
    tauri.invoke.mockResolvedValue(validDesk);
    await expect(getDeskStatus()).resolves.toEqual(validDesk);
    tauri.invoke.mockResolvedValue(undefined);
    await setDisplayMode('media');
    await testAction({ kind: 'shortcut', keys: 'Ctrl+M' });
    expect(tauri.invoke).toHaveBeenCalledWith('get_desk_status', undefined);
    expect(tauri.invoke).toHaveBeenCalledWith('set_display_mode', { mode: 'media' });
    expect(tauri.invoke).toHaveBeenCalledWith('test_action', {
      action: { kind: 'shortcut', keys: 'Ctrl+M' },
    });
    await testRotate({ kind: 'systemVolume' });
    expect(tauri.invoke).toHaveBeenCalledWith('test_rotate', { rotate: { kind: 'systemVolume' } });
  });

  it('rejects a mic state with an unknown token', async () => {
    enterTauri();
    tauri.invoke.mockResolvedValue({ ...validDesk, mic: { ...validDesk.mic, muted: 'maybe' } });
    await expect(getDeskStatus()).rejects.toThrow();
    tauri.invoke.mockResolvedValue({
      ...validDesk,
      mic: { ...validDesk.mic, devices: [{ name: 'A', state: 'broken' }] },
    });
    await expect(getDeskStatus()).rejects.toThrow();
  });

  it.each([
    ['mode', { mode: 'rainbow' }],
    ['media', { media: 'buffering' }],
    ['result', { lastAction: { action: 'mute', result: 'success', permissionRequired: false } }],
    ['action', { pressAction: 'format-disk' }],
    ['percent', { cpuPercent: 250 }],
    ['double-press action', { doublePressAction: 'formatDisk' }],
    ['missing double-press action', { doublePressAction: undefined }],
    ['media title type', { mediaTitle: 42 }],
    ['media artist type', { mediaArtist: { name: 'x' } }],
    ['pinned type', { pinned: 'yes' }],
    ['missing rotate label', { rotateLabel: undefined }],
    ['button labels length', { buttonLabels: ['Back'] }],
    ['hold action', { buttonHoldActions: ['format-disk', null, null] }],
    ['hold actions length', { buttonHoldActions: [null] }],
    ['profile id', { profileId: 'gaming' }],
    ['profile type', { profile: 7 }],
  ])('rejects an unknown %s token', async (_name, patch) => {
    enterTauri();
    tauri.invoke.mockResolvedValue({ ...validDesk, ...patch });
    await expect(getDeskStatus()).rejects.toThrow();
  });

  it.each(['appVolume', 'appMute', 'macro'])('accepts the %s token in a status', async (token) => {
    enterTauri();
    const status = {
      ...validDesk,
      holdAction: token,
      lastAction: { action: token, result: 'error', permissionRequired: false },
    };
    tauri.invoke.mockResolvedValue(status);
    await expect(getDeskStatus()).resolves.toEqual(status);
  });

  it('accepts unbound Press and Hold', async () => {
    enterTauri();
    const unbound = { ...validDesk, pressAction: null, holdAction: null };
    tauri.invoke.mockResolvedValue(unbound);
    await expect(getDeskStatus()).resolves.toEqual(unbound);
  });

  it('keeps unknown now-playing as null and clamps an overlong title instead of dropping status', async () => {
    enterTauri();
    tauri.invoke.mockResolvedValue({ ...validDesk, mediaTitle: null, mediaArtist: null });
    await expect(getDeskStatus()).resolves.toMatchObject({ mediaTitle: null, mediaArtist: null });
    tauri.invoke.mockResolvedValue({ ...validDesk, mediaTitle: 'x'.repeat(5000) });
    const status = await getDeskStatus();
    expect(status.mediaTitle).toHaveLength(MAX_MEDIA_TEXT);
    expect(status.mode).toBe('clock');
  });

  it('drops an invalid desk://status payload and forwards a valid one', async () => {
    enterTauri();
    let emit: (event: { payload: unknown }) => void = () => {};
    tauri.listen.mockImplementation((_name: string, cb: typeof emit) => {
      emit = cb;
      return Promise.resolve(() => {});
    });
    const handler = vi.fn();
    await onDeskStatus(handler);
    expect(tauri.listen).toHaveBeenCalledWith('desk://status', expect.any(Function));
    emit({ payload: { ...validDesk, mode: 'nope' } });
    expect(handler).not.toHaveBeenCalled();
    emit({ payload: validDesk });
    expect(handler).toHaveBeenCalledWith(validDesk);
  });

  it('accepts the new activity types and rejects unknown ones', async () => {
    enterTauri();
    const base = { id: 1, at: 't', summary: 's', severity: 'info', source: 'action' };
    const good = [
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
      'firmwareOfferShown',
      'firmwareOfferAccepted',
      'firmwareOfferLater',
    ].map((type, i) => ({
      ...base,
      id: i,
      type,
      outcome: 'observed',
      metadata: { retryCount: 0, elapsedMs: 0, action: 'playPause' },
    }));
    // The new desktop-only tokens are valid activity metadata; a launch target never is.
    const tokens = ['appVolume', 'appMute', 'macro'].map((action, i) => ({
      ...base,
      id: 100 + i,
      type: 'deskActionConfirmed',
      outcome: 'observed',
      metadata: { retryCount: 0, elapsedMs: 0, action },
    }));
    good.push(...tokens);
    const bad = [
      { ...base, type: 'deskActionExploded', outcome: 'observed', metadata: null },
      { ...base, type: 'deskActionFailed', outcome: 'observed', metadata: { action: 'rm -rf' } },
    ];
    tauri.invoke.mockResolvedValue([...good, ...bad]);
    await expect(getActivityLog(10)).resolves.toEqual(good);
  });
});

describe('action catalog ipc', () => {
  const entry = {
    id: 'launch',
    slot: 'discrete',
    scope: 'launch',
    verification: 'started',
    params: 'target',
    availability: 'available',
    reason: null,
    runsWhenProtected: false,
  };

  it('parses the mock catalog: every action once, all available on Windows', async () => {
    const catalog = await listActionCatalog();
    expect(() => parseCatalog(catalog)).not.toThrow();
    expect(new Set(catalog.map((e) => e.id)).size).toBe(catalog.length);
    for (const id of ['appVolume', 'appMute']) {
      expect(catalog.find((e) => e.id === id)).toMatchObject({
        availability: 'available',
        reason: null,
        verification: 'confirmed',
        runsWhenProtected: true,
      });
    }
    expect(catalog.find((e) => e.id === 'macro')).toMatchObject({
      availability: 'available',
      reason: null,
      verification: 'leastOfSteps',
      runsWhenProtected: false,
    });
    expect(catalog.find((e) => e.id === 'launch')).toMatchObject({ verification: 'started' });
    expect(catalog.find((e) => e.id === 'shortcut')?.runsWhenProtected).toBe(false);
    expect(catalog.find((e) => e.id === 'systemMute')?.runsWhenProtected).toBe(true);
  });

  it('marks App Volume and App Mute unsupported under ?mock=mac', async () => {
    window.history.pushState({}, '', '/?mock=mac');
    try {
      const catalog = await listActionCatalog();
      expect(() => parseCatalog(catalog)).not.toThrow();
      for (const id of ['appVolume', 'appMute']) {
        expect(catalog.find((e) => e.id === id)).toMatchObject({
          availability: 'unsupported',
          reason: "Per-app volume isn't available on macOS",
        });
      }
      expect(catalog.find((e) => e.id === 'systemVolume')?.availability).toBe('available');
      // Never a fallback to the system mute.
      await testAction({ kind: 'appMute', app: 'spotify.exe' });
      expect((await getDeskStatus()).lastAction).toMatchObject({
        action: 'appMute',
        result: 'error',
      });
    } finally {
      window.history.pushState({}, '', '/');
    }
  });

  it('binds App Volume to the knob and App Mute to a button in the mock', async () => {
    await resetConfig();
    const knob = await setRotate('media', { kind: 'appVolume', app: ' Spotify.EXE ' });
    expect(knob.profiles[3].rotate).toMatchObject({
      spec: { kind: 'appVolume', app: 'spotify.exe' },
      deviceLabel: 'spotify',
      overridden: true,
    });
    expect(parseConfig(JSON.parse(JSON.stringify(knob)))).toEqual(knob);
    const labelled = await setRotate('media', { kind: 'appVolume', app: 'vlc.exe', label: 'VLC' });
    expect(labelled.profiles[3].rotate.deviceLabel).toBe('VLC');
    await expect(setRotate('media', { kind: 'appVolume', app: ' ' })).rejects.toThrow('empty');
    await expect(setRotate('media', { kind: 'appVolume', app: 'a'.repeat(129) })).rejects.toThrow(
      '128',
    );
    const mute = await setBinding('media', 'hold', {
      action: { kind: 'appMute', app: 'Spotify.exe' },
    });
    expect(mute.profiles[3].hold).toMatchObject({
      action: { kind: 'appMute', app: 'spotify.exe' },
      deviceLabel: 'Mute',
    });
    expect((await getDeskStatus()).holdAction).toBeDefined();
    await resetConfig();
  });

  it('rejects an app id the native side would refuse', async () => {
    const base = JSON.parse(JSON.stringify(await getConfig()));
    base.profiles[0].rotate.spec = { kind: 'appVolume', app: 'x'.repeat(129) };
    expect(() => parseConfig(base)).toThrow();
    base.profiles[0].rotate.spec = { kind: 'appVolume', app: 'spotify.exe' };
    expect(() => parseConfig(base)).not.toThrow();
  });

  it('uses the exact command name and validates the payload', async () => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} });
    tauri.invoke.mockResolvedValue([entry]);
    await expect(listActionCatalog()).resolves.toEqual([entry]);
    expect(tauri.invoke).toHaveBeenCalledWith('list_action_catalog', undefined);
  });

  it.each([
    ['slot', { slot: 'knob' }],
    ['scope', { scope: 'galaxy' }],
    ['verification', { verification: 'certain' }],
    ['params', { params: 'path' }],
    ['availability', { availability: 'maybe' }],
    ['reason type', { reason: 3 }],
    ['id', { id: '' }],
    ['runsWhenProtected', { runsWhenProtected: 'no' }],
  ])('rejects an unknown %s', async (_name, patch) => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} });
    tauri.invoke.mockResolvedValue([{ ...entry, ...patch }]);
    await expect(listActionCatalog()).rejects.toThrow();
  });

  it('rejects a catalog that is not a list', () => {
    expect(() => parseCatalog({})).toThrow();
  });

  it('mock testAction updates lastAction and rejects an empty shortcut', async () => {
    await testAction({ kind: 'systemMute' });
    expect((await getDeskStatus()).lastAction).toEqual({
      action: 'mute',
      result: 'stateConfirmed',
      permissionRequired: false,
    });
    await expect(testAction({ kind: 'shortcut', keys: ' ' })).rejects.toThrow();
  });
});

describe('config ipc', () => {
  const validConfig = {
    version: 1,
    revision: 3,
    notice: null,
    profiles: [],
    macros: [],
    display: { defaultView: 'buddy', secondaryView: 'cycle', brightness: 5 },
    buddy: { reactions: true, intensity: 'normal' },
  };

  it('accepts the mock defaults and every notice, and rejects unknown tokens', async () => {
    const config = await resetConfig();
    expect(() => parseConfig(config)).not.toThrow();
    for (const notice of ['recoveredCorrupt', 'recoveredNewerVersion', 'migrated']) {
      expect(parseConfig({ ...validConfig, notice }).notice).toBe(notice);
    }
    for (const bad of [
      null,
      { ...validConfig, notice: 'oops' },
      { ...validConfig, revision: -1 },
      { ...validConfig, display: { defaultView: 'cycle', secondaryView: 'buddy', brightness: 5 } },
      { ...validConfig, display: { ...validConfig.display, brightness: 0 } },
      { ...validConfig, display: { ...validConfig.display, brightness: 6 } },
      { ...validConfig, display: { ...validConfig.display, brightness: 2.5 } },
      { ...validConfig, display: { defaultView: 'buddy', secondaryView: 'nope', brightness: 5 } },
      { ...validConfig, buddy: { reactions: 'yes', intensity: 'normal' } },
      { ...validConfig, buddy: { reactions: true, intensity: 'extreme' } },
      { ...validConfig, profiles: 'none' },
      { ...validConfig, macros: 'none' },
      { ...validConfig, macros: [{ id: 'm', name: 'M', steps: [{ kind: 'delay', ms: 49 }] }] },
      {
        ...validConfig,
        macros: [
          {
            id: 'm',
            name: 'M',
            steps: [{ kind: 'action', action: { kind: 'macro', id: 'm' } }],
          },
        ],
      },
    ]) {
      expect(() => parseConfig(bad), JSON.stringify(bad)).toThrow();
    }
  });

  const enterTauri = (): void => {
    (window as Window & { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
  };

  it('calls the native commands with camelCase arguments inside Tauri', async () => {
    enterTauri();
    tauri.invoke.mockResolvedValue(validConfig);
    await expect(getConfig()).resolves.toEqual(validConfig);
    expect(tauri.invoke).toHaveBeenLastCalledWith('get_config', undefined);
    await setDisplaySettings('clock', 'cycle');
    expect(tauri.invoke).toHaveBeenLastCalledWith('set_display_settings', {
      defaultView: 'clock',
      secondaryView: 'cycle',
    });
    await setBuddySettings(false, 'high');
    expect(tauri.invoke).toHaveBeenLastCalledWith('set_buddy_settings', {
      reactions: false,
      intensity: 'high',
    });
    await resetConfig();
    expect(tauri.invoke).toHaveBeenLastCalledWith('reset_config', undefined);
    tauri.invoke.mockResolvedValue({ ...validConfig, notice: 'bogus' });
    await expect(getConfig()).rejects.toThrow('unexpected config notice');
  });

  it('drops an invalid config://changed payload and forwards a valid one', async () => {
    enterTauri();
    let emit: (event: { payload: unknown }) => void = () => {};
    tauri.listen.mockImplementation((_name: string, cb: typeof emit) => {
      emit = cb;
      return Promise.resolve(() => {});
    });
    const handler = vi.fn();
    await onConfigChanged(handler);
    expect(tauri.listen).toHaveBeenCalledWith('config://changed', expect.any(Function));
    emit({ payload: { ...validConfig, notice: 'bogus' } });
    expect(handler).not.toHaveBeenCalled();
    emit({ payload: validConfig });
    expect(handler).toHaveBeenCalledWith(validConfig);
  });

  it('drops an invalid startup://changed payload and forwards a valid one', async () => {
    enterTauri();
    let emit: (event: { payload: unknown }) => void = () => {};
    tauri.listen.mockImplementation((_name: string, cb: typeof emit) => {
      emit = cb;
      return Promise.resolve(() => {});
    });
    const handler = vi.fn();
    await onStartupChanged(handler);
    expect(tauri.listen).toHaveBeenCalledWith('startup://changed', expect.any(Function));
    emit({ payload: { launchAtLogin: 'yes', platform: 'windows' } });
    expect(handler).not.toHaveBeenCalled();
    emit({ payload: { launchAtLogin: true, platform: 'windows' } });
    expect(handler).toHaveBeenCalledWith({ launchAtLogin: true, platform: 'windows' });
  });

  it('tells the browser mock listeners when launch at login changes', async () => {
    const handler = vi.fn();
    const stop = await onStartupChanged(handler);
    await setLaunchAtLogin(true);
    expect(handler).toHaveBeenCalledWith(expect.objectContaining({ launchAtLogin: true }));
    stop();
    await setLaunchAtLogin(false);
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('mirrors the native display rules in the browser mock', async () => {
    await resetConfig();
    await expect(setDisplaySettings('clock', 'clock')).rejects.toThrow('must differ');
    const saved = await setDisplaySettings('clock', 'media');
    expect(saved.display).toEqual({ defaultView: 'clock', secondaryView: 'media', brightness: 5 });
    expect((await getDeskStatus()).mode).toBe('clock');
    const reset = await resetConfig();
    expect(reset.display).toEqual({ defaultView: 'buddy', secondaryView: 'system', brightness: 5 });
    expect(reset.revision).toBeGreaterThan(saved.revision);
  });

  it('lists the built-in profiles in the mock and they pass parseConfig', async () => {
    const config = await resetConfig();
    expect(() => parseConfig(JSON.parse(JSON.stringify(config)))).not.toThrow();
    expect(config.profiles.map((p) => p.id)).toEqual([...PROFILE_IDS]);
    const [general, browser] = config.profiles;
    expect(general.rotate.spec).toEqual({ kind: 'systemVolume' });
    expect(browser.buttons[1].hold).toBe('pin');
    expect(browser.buttons[0].press).toMatchObject({ deviceLabel: 'Back', overridden: false });
  });

  it('saves, validates and deletes macros in the mock, refusing a bound delete', async () => {
    await resetConfig();
    const play = { kind: 'action', action: { kind: 'playPause' } } as const;
    const spec: MacroSpec = {
      id: 'standup',
      name: 'Standup',
      steps: [play, { kind: 'delay', ms: 50 }],
    };
    const saved = await saveMacro(spec);
    expect(saved.macros).toEqual([spec]);
    expect(parseConfig(JSON.parse(JSON.stringify(saved)))).toEqual(saved);
    for (const bad of [
      { ...spec, steps: Array.from({ length: 9 }, () => play) },
      { ...spec, steps: [play, { kind: 'delay', ms: 49 }] },
      { ...spec, steps: [{ kind: 'action', action: { kind: 'macro', id: 'standup' } }] },
      { ...spec, steps: [{ kind: 'delay', ms: 100 }] },
    ]) {
      await expect(saveMacro(bad as MacroSpec), JSON.stringify(bad)).rejects.toThrow();
    }
    await expect(
      setBinding('general', 'press', { action: { kind: 'macro', id: 'ghost' } }),
    ).rejects.toThrow('unknown macro');
    await setBinding('general', 'press', { action: { kind: 'macro', id: 'standup' } });
    expect((await getDeskStatus()).pressAction).toBe('macro');
    await expect(deleteMacro('standup')).rejects.toThrow('still bound');
    await setBinding('general', 'press', null);
    expect((await deleteMacro('standup')).macros).toEqual([]);
  });

  it('rebinds, unbinds and resets a control in the mock', async () => {
    await resetConfig();
    const saved = await setBinding('general', 'button1Hold', {
      action: { kind: 'systemMute' },
      label: ' Quiet ',
    });
    expect(saved.profiles[0].buttons[0].hold).toMatchObject({
      action: { kind: 'systemMute' },
      deviceLabel: 'Quiet',
      overridden: true,
    });
    expect(parseConfig(JSON.parse(JSON.stringify(saved)))).toEqual(saved);
    const unbound = await setBinding('general', 'press', { action: null });
    expect(unbound.profiles[0].press).toMatchObject({ action: null, overridden: true });
    expect((await getDeskStatus()).pressAction).toBeNull();
    const back = await setBinding('general', 'press', null);
    expect(back.profiles[0].press).toMatchObject({
      action: { kind: 'playPause' },
      overridden: false,
    });
    await expect(
      setBinding('general', 'hold', { action: { kind: 'playPause' }, label: 'x'.repeat(33) }),
    ).rejects.toThrow('32 characters');
    await expect(
      setBinding('code', 'press', { action: { kind: 'shortcut', keys: 'Ctrl+' } }),
    ).rejects.toThrow('shortcut');
    const reset = await resetProfile('general');
    expect(reset.profiles[0].buttons[0].hold).toMatchObject({ overridden: false });
    const knob = await setRotate('media', {
      kind: 'shortcuts',
      cw: 'Ctrl+Right',
      ccw: 'Ctrl+Left',
      label: 'Seek',
    });
    expect(knob.profiles[3].rotate).toMatchObject({ deviceLabel: 'Seek', overridden: true });
    expect((await setRotate('media', null)).profiles[3].rotate.overridden).toBe(false);
  });

  it('calls the binding commands with camelCase arguments inside Tauri', async () => {
    enterTauri();
    tauri.invoke.mockResolvedValue(validConfig);
    await setBinding('zoom', 'button3Hold', null);
    expect(tauri.invoke).toHaveBeenLastCalledWith('set_binding', {
      profile: 'zoom',
      control: 'button3Hold',
      slot: null,
    });
    await setRotate('browser', { kind: 'systemVolume' });
    expect(tauri.invoke).toHaveBeenLastCalledWith('set_rotate', {
      profile: 'browser',
      rotate: { kind: 'systemVolume' },
    });
    await resetProfile('code');
    expect(tauri.invoke).toHaveBeenLastCalledWith('reset_profile', { profile: 'code' });
  });
});

// The Rust side asserts it matches vocabulary.json; this asserts the TS side does, so a token
// added on one side only fails a test instead of making parseDeskStatus throw at runtime.
describe('Rust/TS token vocabulary', () => {
  it('matches the shared vocabulary file', () => {
    expect([...DESK_ACTIONS]).toEqual(vocabulary.deskActions);
    expect([...PROFILE_IDS]).toEqual(vocabulary.profileIds);
    expect([...CONTROL_REFS]).toEqual(vocabulary.controls);
    expect([...ACTION_SPEC_KINDS]).toEqual(vocabulary.actionSpecKinds);
    expect([...STEP_SPEC_KINDS]).toEqual(vocabulary.stepSpecKinds);
    expect([...ROTATE_SPEC_KINDS]).toEqual(vocabulary.rotateSpecKinds);
    expect([...CAPABILITY_NAMES]).toEqual(vocabulary.capabilityNames);
    expect([...HOST_AVAILABILITIES]).toEqual(vocabulary.hostAvailability);
    expect([...HOST_MEDIA]).toEqual(vocabulary.hostMedia);
    expect([...CHIP_MODELS]).toEqual(vocabulary.chipModels);
    expect([...RESET_REASONS]).toEqual(vocabulary.resetReasons);
    expect([...HOST_FOCUS]).toEqual(vocabulary.hostFocus);
    expect([...HOST_PRESENCES]).toEqual(vocabulary.hostPresence);
    expect([...HOST_INPUT_PERMISSIONS]).toEqual(vocabulary.hostInputPermission);
    expect([...FLASH_FAILURES]).toEqual(vocabulary.flashFailures);
    expect([...UPDATE_ADVICE]).toEqual(vocabulary.updateAdvice);
    expect([...STARTUP_PLATFORMS]).toEqual(vocabulary.startupPlatform);
    expect([...ACCESSIBILITY_STATES]).toEqual(vocabulary.accessibility);
    expect([...UPDATE_STATES]).toEqual(vocabulary.updateStates);
    expect([...UPDATE_DISABLED_REASONS]).toEqual(vocabulary.updateDisabledReasons);
    expect([...FIRMWARE_OFFER_KINDS]).toEqual(vocabulary.firmwareOfferKinds);
    expect([...FIRMWARE_REQUIRED_REASONS]).toEqual(vocabulary.firmwareRequiredReasons);
    expect(PROFILE_LIMITS).toEqual(vocabulary.profileLimits);
  });
});

describe('onboarding commands', () => {
  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    tauri.invoke.mockReset();
  });

  it('validate what the native side returns', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    tauri.invoke.mockResolvedValueOnce({ completed: true });
    await expect(getOnboarding()).resolves.toEqual({ completed: true });
    expect(tauri.invoke).toHaveBeenCalledWith('get_onboarding', undefined);
    tauri.invoke.mockResolvedValueOnce({ completed: 'yes' });
    await expect(completeOnboarding()).rejects.toThrow();
    tauri.invoke.mockResolvedValueOnce({ completed: false });
    await expect(restartOnboarding()).resolves.toEqual({ completed: false });
    tauri.invoke.mockResolvedValueOnce('missing');
    await expect(getAccessibility()).resolves.toBe('missing');
    tauri.invoke.mockResolvedValueOnce('maybe');
    await expect(requestAccessibility()).rejects.toThrow();
  });
});

describe('parseDiagnostics', () => {
  it('accepts the connected and the disconnected mock payloads', async () => {
    const mock = await import('../mock');
    expect(() =>
      parseDiagnostics(JSON.parse(JSON.stringify(mock.mockGetDiagnostics()))),
    ).not.toThrow();
  });

  it('rejects an unknown token and a negative counter', async () => {
    const mock = await import('../mock');
    const good = JSON.parse(JSON.stringify(mock.mockGetDiagnostics()));
    expect(() => parseDiagnostics({ ...good, host: { ...good.host, focus: 'maybe' } })).toThrow();
    expect(() =>
      parseDiagnostics({ ...good, health: { ...good.health, sequenceGaps: -1 } }),
    ).toThrow();
    expect(() => parseDiagnostics(null)).toThrow();
  });
});

describe('parseFirmwareStatus', () => {
  const good = {
    available: true,
    phase: 'failed',
    message: 'x',
    imageSize: 10,
    failure: 'portBusy',
    bundledVersion: '0.1.0',
    advice: 'updateAvailable',
  };

  it('accepts every failure token and a null one', () => {
    for (const failure of FLASH_FAILURES) {
      expect(parseFirmwareStatus({ ...good, failure }).failure).toBe(failure);
    }
    expect(
      parseFirmwareStatus({ ...good, failure: null, bundledVersion: null }).failure,
    ).toBeNull();
  });

  it('rejects unknown tokens and a missing payload', () => {
    expect(() => parseFirmwareStatus({ ...good, failure: 'meltdown' })).toThrow();
    expect(() => parseFirmwareStatus({ ...good, advice: 'maybe' })).toThrow();
    expect(() => parseFirmwareStatus({ ...good, phase: 'exploding' })).toThrow();
    expect(() => parseFirmwareStatus(null)).toThrow();
  });
});

describe('parseConnectionStatus', () => {
  it('validates effective capabilities and defaults older payloads to no support', () => {
    const base = { host: 'active' };
    expect(parseConnectionStatus(base).capabilities).toEqual([]);
    expect(parseConnectionStatus({ ...base, capabilities: ['deskStatusV1'] }).capabilities).toEqual(
      ['deskStatusV1'],
    );
    for (const capabilities of [null, 'deskStatusV1', [42], ['futureFeature']]) {
      expect(() => parseConnectionStatus({ ...base, capabilities })).toThrow();
    }
  });
  it('accepts every host presence and rejects an unknown one', async () => {
    const mock = await import('../mock');
    const good = JSON.parse(JSON.stringify(mock.mockConnectionStatus()));
    for (const host of HOST_PRESENCES) {
      expect(parseConnectionStatus({ ...good, host }).host).toBe(host);
    }
    expect(() => parseConnectionStatus({ ...good, host: 'hibernating' })).toThrow();
    expect(() => parseConnectionStatus({ ...good, host: undefined })).toThrow();
    expect(() => parseConnectionStatus(null)).toThrow();
  });
});

describe('profile commands', () => {
  const enterTauri = (): void => {
    (window as Window & { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
  };
  beforeEach(() => {
    enterTauri();
  });

  it('call the native commands with camelCase arguments', async () => {
    tauri.invoke.mockResolvedValue(mockGetConfig());
    await addCustomProfile('figma.exe', 'Figma');
    expect(tauri.invoke).toHaveBeenLastCalledWith('add_custom_profile', {
      appId: 'figma.exe',
      name: 'Figma',
    });
    await assignApp('code', 'idea64.exe');
    expect(tauri.invoke).toHaveBeenLastCalledWith('assign_app', {
      profile: 'code',
      appId: 'idea64.exe',
    });
    await renameProfile('custom-1', 'Design');
    expect(tauri.invoke).toHaveBeenLastCalledWith('rename_profile', {
      profile: 'custom-1',
      name: 'Design',
    });
    await deleteProfile('custom-1');
    expect(tauri.invoke).toHaveBeenLastCalledWith('delete_profile', { profile: 'custom-1' });
    await setProfilePinCycle('custom-1', true);
    expect(tauri.invoke).toHaveBeenLastCalledWith('set_profile_pin_cycle', {
      profile: 'custom-1',
      enabled: true,
    });
  });

  it('validates running apps and icons, and an icon failure is just no icon', async () => {
    tauri.invoke.mockResolvedValue([{ id: 'figma.exe', name: 'Figma' }]);
    await expect(listRunningApps()).resolves.toEqual([{ id: 'figma.exe', name: 'Figma' }]);
    tauri.invoke.mockResolvedValue([{ id: 3 }]);
    await expect(listRunningApps()).rejects.toThrow();
    tauri.invoke.mockResolvedValue('data:image/png;base64,AAAA');
    await expect(getAppIcon('icon-ok.exe')).resolves.toBe('data:image/png;base64,AAAA');
    tauri.invoke.mockResolvedValue('javascript:alert(1)');
    await expect(getAppIcon('icon-bad.exe')).resolves.toBeNull();
    tauri.invoke.mockRejectedValue(new Error('nope'));
    await expect(getAppIcon('icon-fail.exe')).resolves.toBeNull();
  });

  it('loads immutable catalog art once per key and treats invalid/failed responses as absent', async () => {
    tauri.invoke.mockResolvedValue('data:image/png;base64,AAAA');
    await expect(getProductArt('art-a')).resolves.toBe('data:image/png;base64,AAAA');
    expect(tauri.invoke).toHaveBeenLastCalledWith('get_product_art', { product: 'art-a' });
    tauri.invoke.mockClear();
    await getProductArt('art-a');
    expect(tauri.invoke).not.toHaveBeenCalled();
    tauri.invoke.mockResolvedValue('unbundled.png');
    await expect(getProductArt('art-bad')).resolves.toBeNull();
    tauri.invoke.mockResolvedValue('data:image/png;base64,' + 'A'.repeat(700_000));
    await expect(getProductArt('art-large')).resolves.toBeNull();
    tauri.invoke.mockRejectedValue(new Error('unavailable'));
    await expect(getProductArt('art-failed')).resolves.toBeNull();
    tauri.invoke.mockClear();
    tauri.invoke.mockResolvedValue('data:image/png;base64,AAAA');
    await expect(getProductArt('art-failed')).resolves.toBe('data:image/png;base64,AAAA');
    expect(tauri.invoke).toHaveBeenCalledTimes(1);
  });

  it('accepts a custom profile id in the desk status and rejects a malformed one', async () => {
    tauri.invoke.mockResolvedValue({ ...validDesk, profileId: 'custom-12' });
    await expect(getDeskStatus()).resolves.toMatchObject({ profileId: 'custom-12' });
    tauri.invoke.mockResolvedValue({ ...validDesk, profileId: 'custom-' });
    await expect(getDeskStatus()).rejects.toThrow();
    tauri.invoke.mockResolvedValue({ ...validDesk, recentApp: null });
    await expect(getDeskStatus()).resolves.toMatchObject({ recentApp: null });
  });
});

describe('app updater', () => {
  const enterTauri = (): void => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} });
  };
  const available = {
    state: 'available',
    reason: null,
    currentVersion: '0.1.0',
    lastChecked: '2026-10-12T08:00:00Z',
    autoCheck: true,
    update: { version: '0.2.0', notes: '<b>bold</b>', date: '2026-10-12T00:00:00Z' },
    progress: null,
  };

  it('validates the status and rejects unknown or inconsistent shapes', () => {
    expect(parseUpdateStatus(available)).toEqual(available);
    expect(() => parseUpdateStatus({ ...available, state: 'exploding' })).toThrow();
    // A release is required exactly in the states about one.
    expect(() => parseUpdateStatus({ ...available, update: null })).toThrow();
    expect(() => parseUpdateStatus({ ...available, state: 'idle' })).toThrow();
    expect(() => parseUpdateStatus({ ...available, autoCheck: 'yes' })).toThrow();
    const disabled = { ...available, state: 'disabled', reason: 'noKey', update: null };
    expect(parseUpdateStatus(disabled).reason).toBe('noKey');
    expect(() => parseUpdateStatus({ ...disabled, reason: 'whatever' })).toThrow();
    // The reason only means something while disabled.
    expect(parseUpdateStatus({ ...available, reason: 'noKey' }).reason).toBeNull();
    const downloading = {
      ...available,
      state: 'downloading',
      progress: { downloaded: 10, total: null },
    };
    expect(parseUpdateStatus(downloading).progress).toEqual({ downloaded: 10, total: null });
    expect(() =>
      parseUpdateStatus({ ...downloading, progress: { downloaded: -1, total: 5 } }),
    ).toThrow();
    // Notes stay plain text, untouched.
    expect(parseUpdateStatus(available).update?.notes).toBe('<b>bold</b>');
  });

  it('calls the typed native commands and validates what they return', async () => {
    enterTauri();
    tauri.invoke.mockResolvedValue(available);
    await expect(getUpdateStatus()).resolves.toEqual(available);
    expect(tauri.invoke).toHaveBeenLastCalledWith('get_update_status', undefined);
    await checkForUpdates();
    expect(tauri.invoke).toHaveBeenLastCalledWith('check_for_updates', undefined);
    await installUpdate();
    expect(tauri.invoke).toHaveBeenLastCalledWith('install_update', undefined);
    await snoozeUpdate();
    expect(tauri.invoke).toHaveBeenLastCalledWith('snooze_update', undefined);
    await skipUpdateVersion();
    expect(tauri.invoke).toHaveBeenLastCalledWith('skip_update_version', undefined);
    await setAutoUpdateCheck(false);
    expect(tauri.invoke).toHaveBeenLastCalledWith('set_auto_update_check', { enabled: false });
    tauri.invoke.mockResolvedValue({ ...available, state: 'bogus' });
    await expect(getUpdateStatus()).rejects.toThrow();
  });

  it('drops an invalid updater://status payload and forwards the tray request', async () => {
    enterTauri();
    const listeners: Record<string, (event: { payload: unknown }) => void> = {};
    tauri.listen.mockImplementation((name: string, cb: (event: { payload: unknown }) => void) => {
      listeners[name] = cb;
      return Promise.resolve(() => {});
    });
    const handler = vi.fn();
    await onUpdateStatus(handler);
    listeners['updater://status']!({ payload: { ...available, state: 'nope' } });
    expect(handler).not.toHaveBeenCalled();
    listeners['updater://status']!({ payload: available });
    expect(handler).toHaveBeenCalledWith(available);
    const show = vi.fn();
    await onUpdateDialogRequested(show);
    listeners['updater://show']!({ payload: null });
    expect(show).toHaveBeenCalledTimes(1);
  });

  it('shows a development build as disabled in the browser mock', async () => {
    const status = await getUpdateStatus();
    expect(status).toMatchObject({ state: 'disabled', reason: 'devBuild', update: null });
  });
});

describe('firmware offer', () => {
  const enterTauri = (): void => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} });
  };
  const optional = {
    kind: 'offerOptional',
    product: 'Kivori Pip',
    from: '0.1.0',
    to: '0.2.0',
    reason: null,
    key: '1:abcd1234:0.2.0',
  };
  const none = { kind: 'none', product: null, from: null, to: null, reason: null, key: null };

  it('validates the offer and rejects unknown or incomplete shapes', () => {
    expect(parseFirmwareOffer(optional)).toEqual(optional);
    expect(parseFirmwareOffer(none)).toEqual(none);
    expect(() => parseFirmwareOffer({ ...optional, kind: 'forceFlash' })).toThrow();
    // A question needs its versions and key.
    expect(() => parseFirmwareOffer({ ...optional, to: null })).toThrow();
    expect(() => parseFirmwareOffer({ ...optional, from: null })).toThrow();
    expect(() => parseFirmwareOffer({ ...optional, key: null })).toThrow();
    const required = { ...none, kind: 'required', to: '0.2.0', reason: 'protocolTooOld', key: 'k' };
    expect(parseFirmwareOffer(required)).toEqual(required);
    expect(parseFirmwareOffer({ ...required, to: null }).to).toBeNull();
    expect(() =>
      parseFirmwareOffer({ ...required, to: null, reason: 'missingCapabilities' }),
    ).toThrow();
    expect(() => parseFirmwareOffer({ ...required, reason: 'because' })).toThrow();
    // Fields that mean nothing for a kind are dropped.
    expect(parseFirmwareOffer({ ...optional, kind: 'updateApp' })).toEqual({
      ...none,
      kind: 'updateApp',
    });
    expect(parseFirmwareOffer({ ...optional, reason: 'protocolTooOld' }).reason).toBeNull();
  });

  it('calls the typed native commands and validates what they return', async () => {
    enterTauri();
    tauri.invoke.mockResolvedValue(optional);
    await expect(getFirmwareOffer()).resolves.toEqual(optional);
    expect(tauri.invoke).toHaveBeenLastCalledWith('get_firmware_offer', undefined);
    tauri.invoke.mockResolvedValue(undefined);
    await acceptFirmwareOffer();
    expect(tauri.invoke).toHaveBeenLastCalledWith('accept_firmware_offer', undefined);
    tauri.invoke.mockResolvedValue(none);
    await expect(dismissFirmwareOffer()).resolves.toEqual(none);
    expect(tauri.invoke).toHaveBeenLastCalledWith('dismiss_firmware_offer', undefined);
    tauri.invoke.mockResolvedValue({ ...optional, kind: 'bogus' });
    await expect(getFirmwareOffer()).rejects.toThrow();
  });

  it('drops an invalid firmware://offer payload and forwards the tray request', async () => {
    enterTauri();
    const listeners: Record<string, (event: { payload: unknown }) => void> = {};
    tauri.listen.mockImplementation((name: string, cb: (event: { payload: unknown }) => void) => {
      listeners[name] = cb;
      return Promise.resolve(() => {});
    });
    const handler = vi.fn();
    await onFirmwareOffer(handler);
    listeners['firmware://offer']!({ payload: { ...optional, kind: 'nope' } });
    expect(handler).not.toHaveBeenCalled();
    listeners['firmware://offer']!({ payload: optional });
    expect(handler).toHaveBeenCalledWith(optional);
    const show = vi.fn();
    await onFirmwareOfferRequested(show);
    listeners['firmware://show-offer']!({ payload: null });
    expect(show).toHaveBeenCalledTimes(1);
  });

  it('offers nothing in the plain browser mock', async () => {
    await expect(getFirmwareOffer()).resolves.toMatchObject({ kind: 'none', key: null });
  });
});
