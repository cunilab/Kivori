import type { DeviceBuildDto } from '@/lib/ipc/types';

export interface BuildNotices {
  /** The unit runs firmware built with uncommitted changes. */
  dirty: boolean;
  /** The unit's firmware source is not the one this app was built with. */
  differs: boolean;
}

/**
 * What to tell about a device's firmware source (a hash of the committed firmware input files, so
 * identical inputs always give the same id, whatever the commit).
 *
 * `dirty` shows to everyone: release firmware is always built from a clean tree, so it appears only
 * for a build someone made by hand, and there it is the useful fact. `differs` shows in developer
 * mode only: after an app update a release user's unit legitimately runs older firmware than the
 * app bundles (firmware has its own version and update advice), so for them it would be a false
 * alarm; a developer expects the two to match. Nothing shows when either id is unknown.
 */
export function buildNotices(
  build: DeviceBuildDto | null | undefined,
  appSourceId: string | null | undefined,
  devMode: boolean,
): BuildNotices {
  const sourceId = build?.sourceId ?? null;
  return {
    dirty: sourceId !== null && build?.dirty === true,
    differs: devMode && sourceId !== null && !!appSourceId && sourceId !== appSourceId,
  };
}
