import type { Installer, Release, ReleasesResult } from '@/lib/releases';

export type DownloadState =
  /** The newest release has an installer: the download button can go straight to it. */
  | { kind: 'ready'; release: Release; installer: Installer }
  /** A stable release exists but its Windows installer is unavailable. */
  | { kind: 'missing'; release: Release }
  /** No stable release exists yet. */
  | { kind: 'empty'; releases: Release[] }
  /** The release list could not be loaded. */
  | { kind: 'unavailable' };

/** What the download page shows, from the release list. `/download/windows/latest` selects the same stable release. */
export function downloadState(result: ReleasesResult): DownloadState {
  if (result.status === 'error') return { kind: 'unavailable' };
  const latest = result.releases.find(
    (release) => !release.prerelease && !release.version.includes('-'),
  );
  if (latest?.installer) return { kind: 'ready', release: latest, installer: latest.installer };
  if (latest) return { kind: 'missing', release: latest };
  return { kind: 'empty', releases: result.releases };
}
