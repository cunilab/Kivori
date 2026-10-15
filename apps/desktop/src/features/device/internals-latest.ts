import type { DeviceInternalsDto } from '@/lib/ipc/types';

// Five missed 1 Hz polls: cached native measurements must not look newly measured on each read.
export const INTERNALS_FRESH_MS = 5000;
let latest: { data: DeviceInternalsDto; readAt: number } | null = null;

export function setLatestInternals(value: DeviceInternalsDto | null): void {
  latest = value ? { data: value, readAt: performance.now() } : null;
}

export function getLatestInternals(): DeviceInternalsDto | null {
  return latest?.data ?? null;
}

/** Copy only an already observed, current-session measurement; never request one here. */
export function internalsForBundle(
  generation: number,
  connected: boolean,
): {
  status: 'fresh' | 'stale' | 'unavailable';
  ageMs: number | null;
  data: DeviceInternalsDto | null;
} {
  const entry = latest;
  if (
    !connected ||
    !entry ||
    entry.data.connectionGeneration !== generation ||
    !entry.data.connected ||
    !entry.data.supported ||
    !entry.data.snapshot ||
    entry.data.snapshotAgeMs === null
  ) {
    return { status: 'unavailable', ageMs: null, data: null };
  }
  const ageMs =
    entry.data.snapshotAgeMs + Math.max(0, Math.floor(performance.now() - entry.readAt));
  return ageMs > INTERNALS_FRESH_MS
    ? { status: 'stale', ageMs, data: null }
    : { status: 'fresh', ageMs, data: { ...entry.data, snapshotAgeMs: ageMs } };
}
