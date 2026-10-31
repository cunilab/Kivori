import type { DeviceBuildDto } from '@/lib/ipc/types';

/** Git's customary short form of a commit. */
export const shortCommit = (commit: string): string => commit.slice(0, 7);

export interface BuildNotices {
  /** The unit runs firmware built with uncommitted changes. */
  dirty: boolean;
  /** The unit's commit is not the commit this app was built from. */
  differs: boolean;
}

/**
 * What to tell about a device's firmware build.
 *
 * `dirty` shows to everyone: release firmware is always built from a clean commit, so it appears
 * only for a build someone made by hand, and there it is the useful fact. `differs` shows in
 * developer mode only: a release user's unit legitimately runs an older firmware commit than the
 * app after an app update (firmware has its own version and update advice), so for them it would be
 * a false alarm; a developer expects the two to match. Nothing shows when either commit is unknown.
 */
export function buildNotices(
  build: DeviceBuildDto | null | undefined,
  appCommit: string | null | undefined,
  devMode: boolean,
): BuildNotices {
  const commit = build?.commit ?? null;
  return {
    dirty: commit !== null && build?.dirty === true,
    differs: devMode && commit !== null && !!appCommit && commit !== appCommit,
  };
}
