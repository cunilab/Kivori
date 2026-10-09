// Runtime validation for native payloads that carry closed vocabularies. TypeScript types vanish at
// runtime, so tokens are checked here: an unknown token is rejected, never passed through to the UI.

import {
  ACTIVITY_EVENT_TYPES,
  CONFIG_NOTICES,
  DESK_ACTIONS,
  DESK_RESULTS,
  DISPLAY_MODES,
  DOUBLE_PRESS_ACTIONS,
  INTENSITIES,
  MASCOT_ACTIONS,
  MEDIA_STATUSES,
} from './types';
import type { ActivityEventDto, ConfigDto, DeskStatusDto } from './types';

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

function buttonActions(value: unknown): DeskStatusDto['buttonActions'] {
  if (!Array.isArray(value) || value.length !== 3)
    throw new Error('Kivori: invalid buttonActions.');
  const one = (v: unknown) => (v === null ? null : oneOf('desk action', DESK_ACTIONS, v));
  return [one(value[0]), one(value[1]), one(value[2])];
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
    pressAction: oneOf('desk action', DESK_ACTIONS, r.pressAction),
    holdAction: oneOf('desk action', DESK_ACTIONS, r.holdAction),
    doublePressAction: oneOf('double-press action', DOUBLE_PRESS_ACTIONS, r.doublePressAction),
    buttonActions: buttonActions(r.buttonActions),
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
  return {
    version: r.version,
    revision: r.revision,
    notice: r.notice === null ? null : oneOf('config notice', CONFIG_NOTICES, r.notice),
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
