// Runtime validation for native payloads that carry closed vocabularies. TypeScript types vanish at
// runtime, so tokens are checked here: an unknown token is rejected, never passed through to the UI.

import {
  ACTIVITY_EVENT_TYPES,
  DESK_ACTIONS,
  DESK_RESULTS,
  DISPLAY_MODES,
  MASCOT_ACTIONS,
  MEDIA_STATUSES,
} from './types';
import type { ActivityEventDto, DeskStatusDto } from './types';

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
    lastAction,
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
