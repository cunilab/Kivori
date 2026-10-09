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
  getAppInfo,
  getActivityLog,
  getConfig,
  getConnectionStatus,
  isTauri,
  onConfigChanged,
  resetConfig,
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
  setDisplayMode,
  renderPreviewFrame,
} from '../index';
import vocabulary from './vocabulary.json';
import { COMPANION_STATES, CONTROL_REFS, DESK_ACTIONS, PREVIEW_DIM, PROFILE_IDS } from '../types';
import { MAX_MEDIA_TEXT, parseCatalog, parseConfig } from '../validate';

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
    expect(info.protocolVersion.major).toBeGreaterThanOrEqual(1);
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
};

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

  it('parses the mock catalog: every action once, App Volume and App Mute unsupported', async () => {
    const catalog = await listActionCatalog();
    expect(() => parseCatalog(catalog)).not.toThrow();
    expect(new Set(catalog.map((e) => e.id)).size).toBe(catalog.length);
    for (const id of ['appVolume', 'appMute']) {
      expect(catalog.find((e) => e.id === id)).toMatchObject({
        availability: 'unsupported',
        reason: 'Coming soon',
      });
    }
    expect(catalog.find((e) => e.id === 'launch')).toMatchObject({ verification: 'started' });
    expect(catalog.find((e) => e.id === 'shortcut')?.runsWhenProtected).toBe(false);
    expect(catalog.find((e) => e.id === 'systemMute')?.runsWhenProtected).toBe(true);
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
    display: { defaultView: 'buddy', secondaryView: 'cycle' },
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
      { ...validConfig, display: { defaultView: 'cycle', secondaryView: 'buddy' } },
      { ...validConfig, display: { defaultView: 'buddy', secondaryView: 'nope' } },
      { ...validConfig, buddy: { reactions: 'yes', intensity: 'normal' } },
      { ...validConfig, buddy: { reactions: true, intensity: 'extreme' } },
      { ...validConfig, profiles: 'none' },
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

  it('mirrors the native display rules in the browser mock', async () => {
    await resetConfig();
    await expect(setDisplaySettings('clock', 'clock')).rejects.toThrow('must differ');
    const saved = await setDisplaySettings('clock', 'media');
    expect(saved.display).toEqual({ defaultView: 'clock', secondaryView: 'media' });
    expect((await getDeskStatus()).mode).toBe('clock');
    const reset = await resetConfig();
    expect(reset.display).toEqual({ defaultView: 'buddy', secondaryView: 'system' });
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
  });
});
