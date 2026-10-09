// Runtime validation for native payloads that carry closed vocabularies. TypeScript types vanish at
// runtime, so tokens are checked here: an unknown token is rejected, never passed through to the UI.

import {
  ACTION_SPEC_KINDS,
  STEP_SPEC_KINDS,
  ACTIVITY_EVENT_TYPES,
  CATALOG_AVAILABILITIES,
  CATALOG_PARAMS,
  CATALOG_SCOPES,
  CATALOG_SLOTS,
  CATALOG_VERIFICATIONS,
  CONFIG_NOTICES,
  DESK_ACTIONS,
  DESK_RESULTS,
  DISPLAY_MODES,
  DOUBLE_PRESS_ACTIONS,
  INTENSITIES,
  MASCOT_ACTIONS,
  MEDIA_STATUSES,
  PROFILE_IDS,
  ROTATE_SPEC_KINDS,
} from './types';
import type {
  ActionCatalogEntryDto,
  ActionSpec,
  ActivityEventDto,
  ButtonDto,
  ConfigDto,
  MacroSpec,
  StepSpec,
  DeskStatusDto,
  ProfileConfigDto,
  RotateSpec,
  SlotDto,
} from './types';

function oneOf<T extends string>(name: string, allowed: readonly T[], value: unknown): T {
  if (typeof value === 'string' && (allowed as readonly string[]).includes(value))
    return value as T;
  throw new Error(`Kivori: unexpected ${name} token.`);
}

function percentOrNull(name: string, value: unknown): number | null {
  if (value === null) return null;
  if (typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 100) {
    return value;
  }
  throw new Error(`Kivori: invalid ${name}.`);
}

function boolOrNull(name: string, value: unknown): boolean | null {
  if (value === null || typeof value === 'boolean') return value;
  throw new Error(`Kivori: invalid ${name}.`);
}

// Free text from the OS media session (any player can set it). Clamped rather than rejected, so one
// overlong title cannot blank the whole desk view; a non-string is still a contract violation.
export const MAX_MEDIA_TEXT = 200;

function textOrNull(name: string, value: unknown): string | null {
  if (value === null) return null;
  if (typeof value === 'string') return [...value].slice(0, MAX_MEDIA_TEXT).join('');
  throw new Error(`Kivori: invalid ${name}.`);
}

function buttonActions(name: string, value: unknown): DeskStatusDto['buttonActions'] {
  if (!Array.isArray(value) || value.length !== 3) throw new Error(`Kivori: invalid ${name}.`);
  const one = (v: unknown) => (v === null ? null : oneOf('desk action', DESK_ACTIONS, v));
  return [one(value[0]), one(value[1]), one(value[2])];
}

function actionOrNull(value: unknown): DeskStatusDto['holdAction'] {
  return value === null ? null : oneOf('desk action', DESK_ACTIONS, value);
}

// Labels Desktop derives from its own profiles; clamped like media text, never trusted to be short.
function label(name: string, value: unknown): string {
  const text = textOrNull(name, value);
  if (text === null) throw new Error(`Kivori: invalid ${name}.`);
  return text;
}

function buttonLabels(value: unknown): DeskStatusDto['buttonLabels'] {
  if (!Array.isArray(value) || value.length !== 3) throw new Error('Kivori: invalid buttonLabels.');
  return [
    label('buttonLabels', value[0]),
    label('buttonLabels', value[1]),
    label('buttonLabels', value[2]),
  ];
}

/** Validates a `DeskStatusDto`, throwing on any unknown token or malformed value. */
export function parseDeskStatus(raw: unknown): DeskStatusDto {
  if (typeof raw !== 'object' || raw === null) throw new Error('Kivori: invalid desk status.');
  const r = raw as Record<string, unknown>;
  let lastAction: DeskStatusDto['lastAction'] = null;
  if (r.lastAction !== null) {
    const last = r.lastAction as Record<string, unknown> | undefined;
    if (typeof last !== 'object' || typeof last.permissionRequired !== 'boolean') {
      throw new Error('Kivori: invalid desk action.');
    }
    lastAction = {
      action: oneOf('desk action', DESK_ACTIONS, last.action),
      result: oneOf('desk result', DESK_RESULTS, last.result),
      permissionRequired: last.permissionRequired,
    };
  }
  if (typeof r.highLoad !== 'boolean') throw new Error('Kivori: invalid highLoad.');
  if (typeof r.pinned !== 'boolean') throw new Error('Kivori: invalid pinned.');
  return {
    mode: oneOf('display mode', DISPLAY_MODES, r.mode),
    volumePercent: percentOrNull('volumePercent', r.volumePercent),
    muted: boolOrNull('muted', r.muted),
    media: r.media === null ? null : oneOf('media', MEDIA_STATUSES, r.media),
    cpuPercent: percentOrNull('cpuPercent', r.cpuPercent),
    ramPercent: percentOrNull('ramPercent', r.ramPercent),
    highLoad: r.highLoad,
    pressAction: actionOrNull(r.pressAction),
    holdAction: actionOrNull(r.holdAction),
    doublePressAction: oneOf('double-press action', DOUBLE_PRESS_ACTIONS, r.doublePressAction),
    buttonActions: buttonActions('buttonActions', r.buttonActions),
    buttonHoldActions: buttonActions('buttonHoldActions', r.buttonHoldActions),
    profileId: oneOf('profile', PROFILE_IDS, r.profileId),
    profile: textOrNull('profile', r.profile),
    pinned: r.pinned,
    rotateLabel: label('rotateLabel', r.rotateLabel),
    buttonLabels: buttonLabels(r.buttonLabels),
    mediaTitle: textOrNull('mediaTitle', r.mediaTitle),
    mediaArtist: textOrNull('mediaArtist', r.mediaArtist),
    lastAction,
  };
}

function record(name: string, value: unknown): Record<string, unknown> {
  if (typeof value !== 'object' || value === null) throw new Error(`Kivori: invalid ${name}.`);
  return value as Record<string, unknown>;
}

function text(name: string, value: unknown, max = 1024): string {
  if (typeof value === 'string' && value.length <= max) return value;
  throw new Error(`Kivori: invalid ${name}.`);
}

/** The longest app id the native side accepts. */
const MAX_APP_ID = 128;

/** The longest macro id the native side accepts. */
const MAX_MACRO_ID = 64;

function actionSpec(raw: unknown): ActionSpec {
  const r = record('action', raw);
  switch (oneOf('action kind', ACTION_SPEC_KINDS, r.kind)) {
    case 'shortcut':
      return { kind: 'shortcut', keys: text('shortcut', r.keys) };
    case 'launch':
      return { kind: 'launch', target: text('launch target', r.target) };
    case 'appMute':
      return { kind: 'appMute', app: text('app', r.app, MAX_APP_ID) };
    case 'macro':
      return { kind: 'macro', id: text('macro id', r.id, MAX_MACRO_ID) };
    default:
      return { kind: r.kind } as ActionSpec;
  }
}

function stepSpec(raw: unknown): StepSpec {
  const r = record('step', raw);
  if (oneOf('step kind', STEP_SPEC_KINDS, r.kind) === 'delay') {
    const { ms } = r;
    if (typeof ms !== 'number' || !Number.isInteger(ms) || ms < 50 || ms > 2000) {
      throw new Error('Kivori: invalid delay.');
    }
    return { kind: 'delay', ms };
  }
  const action = actionSpec(r.action);
  // A step is never a macro (no nesting).
  if (action.kind === 'macro') throw new Error('Kivori: invalid step.');
  return { kind: 'action', action };
}

function macroSpec(raw: unknown): MacroSpec {
  const r = record('macro', raw);
  if (!Array.isArray(r.steps) || r.steps.length > 8) throw new Error('Kivori: invalid steps.');
  return {
    id: text('macro id', r.id, MAX_MACRO_ID),
    name: text('macro name', r.name),
    steps: r.steps.map(stepSpec),
  };
}

function rotateSpec(raw: unknown): RotateSpec {
  const r = record('rotate', raw);
  switch (oneOf('rotate kind', ROTATE_SPEC_KINDS, r.kind)) {
    case 'appVolume':
      return {
        kind: 'appVolume',
        app: text('app', r.app, MAX_APP_ID),
        ...(typeof r.label === 'string' ? { label: text('label', r.label) } : {}),
      };
    case 'shortcuts':
      return {
        kind: 'shortcuts',
        cw: text('shortcut', r.cw),
        ccw: text('shortcut', r.ccw),
        label: text('label', r.label),
      };
    default:
      return { kind: 'systemVolume' };
  }
}

function slotDto(raw: unknown): SlotDto {
  const r = record('slot', raw);
  if (typeof r.overridden !== 'boolean') throw new Error('Kivori: invalid overridden.');
  return {
    action: r.action === null ? null : actionSpec(r.action),
    label: r.label === null ? null : text('label', r.label),
    deviceLabel: text('device label', r.deviceLabel),
    overridden: r.overridden,
  };
}

function buttonDto(raw: unknown): ButtonDto {
  const r = record('button', raw);
  return { press: slotDto(r.press), hold: r.hold === 'pin' ? 'pin' : slotDto(r.hold) };
}

function profileDto(raw: unknown): ProfileConfigDto {
  const r = record('profile', raw);
  const rotate = record('rotate', r.rotate);
  if (typeof rotate.overridden !== 'boolean') throw new Error('Kivori: invalid overridden.');
  if (!Array.isArray(r.apps)) throw new Error('Kivori: invalid apps.');
  if (!Array.isArray(r.buttons) || r.buttons.length !== 3) {
    throw new Error('Kivori: invalid buttons.');
  }
  return {
    id: oneOf('profile', PROFILE_IDS, r.id),
    name: text('profile name', r.name),
    apps: r.apps.map((app) => text('app id', app)),
    rotate: {
      spec: rotateSpec(rotate.spec),
      deviceLabel: text('device label', rotate.deviceLabel),
      overridden: rotate.overridden,
    },
    press: slotDto(r.press),
    hold: slotDto(r.hold),
    buttons: [buttonDto(r.buttons[0]), buttonDto(r.buttons[1]), buttonDto(r.buttons[2])],
  };
}

/** Validates the action catalog, throwing on any unknown token or malformed value. */
export function parseCatalog(raw: unknown): ActionCatalogEntryDto[] {
  if (!Array.isArray(raw)) throw new Error('Kivori: invalid action catalog.');
  return raw.map((item: unknown) => {
    const r = record('catalog entry', item);
    if (typeof r.id !== 'string' || r.id === '') throw new Error('Kivori: invalid catalog id.');
    if (typeof r.runsWhenProtected !== 'boolean') {
      throw new Error('Kivori: invalid runsWhenProtected.');
    }
    if (r.reason !== null && typeof r.reason !== 'string') {
      throw new Error('Kivori: invalid catalog reason.');
    }
    return {
      id: r.id,
      slot: oneOf('catalog slot', CATALOG_SLOTS, r.slot),
      scope: oneOf('catalog scope', CATALOG_SCOPES, r.scope),
      verification: oneOf('catalog verification', CATALOG_VERIFICATIONS, r.verification),
      params: oneOf('catalog params', CATALOG_PARAMS, r.params),
      availability: oneOf('catalog availability', CATALOG_AVAILABILITIES, r.availability),
      reason: r.reason,
      runsWhenProtected: r.runsWhenProtected,
    };
  });
}

/** Validates a `ConfigDto`, throwing on any unknown token or malformed value. */
export function parseConfig(raw: unknown): ConfigDto {
  const r = record('config', raw);
  const display = record('display settings', r.display);
  const buddy = record('buddy settings', r.buddy);
  if (typeof r.version !== 'number' || !Number.isInteger(r.version)) {
    throw new Error('Kivori: invalid config version.');
  }
  if (typeof r.revision !== 'number' || !Number.isInteger(r.revision) || r.revision < 0) {
    throw new Error('Kivori: invalid config revision.');
  }
  if (typeof buddy.reactions !== 'boolean') throw new Error('Kivori: invalid reactions.');
  if (!Array.isArray(r.profiles)) throw new Error('Kivori: invalid profiles.');
  if (!Array.isArray(r.macros)) throw new Error('Kivori: invalid macros.');
  return {
    version: r.version,
    revision: r.revision,
    notice: r.notice === null ? null : oneOf('config notice', CONFIG_NOTICES, r.notice),
    profiles: r.profiles.map(profileDto),
    macros: r.macros.map(macroSpec),
    display: {
      defaultView: oneOf('display mode', DISPLAY_MODES, display.defaultView),
      secondaryView: oneOf('display mode', [...DISPLAY_MODES, 'cycle'], display.secondaryView),
    },
    buddy: {
      reactions: buddy.reactions,
      intensity: oneOf('intensity', INTENSITIES, buddy.intensity),
    },
  };
}

const ACTIVITY_ACTIONS: readonly string[] = [...MASCOT_ACTIONS, ...DESK_ACTIONS];

/** True when the event's type (and metadata action, if any) are in the closed vocabularies. */
export function isActivityEvent(raw: unknown): raw is ActivityEventDto {
  if (typeof raw !== 'object' || raw === null) return false;
  const e = raw as { type?: unknown; metadata?: { action?: unknown } | null };
  if (!(ACTIVITY_EVENT_TYPES as readonly unknown[]).includes(e.type)) return false;
  const action = e.metadata?.action;
  return action === undefined || (typeof action === 'string' && ACTIVITY_ACTIONS.includes(action));
}
