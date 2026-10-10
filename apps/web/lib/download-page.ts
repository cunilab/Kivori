import type { Installer, Release, ReleasesResult } from '@/lib/releases';

export type DownloadState =
  /** The newest release has an installer: the download button can go straight to it. */
  | { kind: 'ready'; release: Release; installer: Installer }
  /** No release has an installer yet (or none exist): "First release coming soon" + Notify me. */
  | { kind: 'empty'; releases: Release[] }
  /** The release list could not be loaded. */
  | { kind: 'unavailable' };

/** What the download page shows, from the release list. `/download/windows/latest` serves releases[0]. */
export function downloadState(result: ReleasesResult): DownloadState {
  if (result.status === 'error') return { kind: 'unavailable' };
  const latest = result.releases[0];
  if (latest?.installer) return { kind: 'ready', release: latest, installer: latest.installer };
  return { kind: 'empty', releases: result.releases };
}
