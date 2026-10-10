import { cachedFetchText } from '@/lib/github-cache';

// Server-only: this module talks to the release host and must never be imported by a client component.
// Pages show versions and dates only; the asset URL is used by the download route to stream the file.
export const RELEASES_API_URL = 'https://api.github.com/repos/cunilab/Kivori/releases?per_page=20';
const INSTALLER_PATTERN = /-setup\.exe$/i;

const API_HEADERS = {
  'User-Agent': 'kivori-web',
  Accept: 'application/vnd.github+json',
  'X-GitHub-Api-Version': '2022-11-28',
};

interface RawAsset {
  name: string;
  size: number;
  browser_download_url: string;
}

export interface RawRelease {
  tag_name: string;
  draft: boolean;
  prerelease: boolean;
  published_at: string | null;
  created_at?: string | null;
  assets?: RawAsset[];
}

export interface Installer {
  size: number;
  /** Upstream asset URL. Never rendered or redirected to; the download route streams it. */
  url: string;
}

export interface Release {
  version: string;
  tag: string;
  date: string | null;
  prerelease: boolean;
  installer: Installer | null;
}

export type ReleasesResult = { status: 'ok'; releases: Release[] } | { status: 'error' };

function releaseTime(release: RawRelease): number {
  const time = Date.parse(release.published_at ?? release.created_at ?? '');
  return Number.isNaN(time) ? 0 : time;
}

/** Non-draft releases (pre-releases included), newest first. */
export function publishedReleases(raw: readonly RawRelease[]): RawRelease[] {
  return raw.filter((release) => !release.draft).sort((a, b) => releaseTime(b) - releaseTime(a));
}

/** The newest non-draft release; `/releases/latest` would skip pre-releases, so pick it ourselves. */
export function pickLatest(raw: readonly RawRelease[]): RawRelease | undefined {
  return publishedReleases(raw)[0];
}

export function findInstaller(assets: readonly RawAsset[] | undefined): Installer | null {
  const asset = assets?.find((candidate) => INSTALLER_PATTERN.test(candidate.name));
  return asset ? { size: asset.size, url: asset.browser_download_url } : null;
}

export function versionFromTag(tag: string): string {
  return tag.replace(/^v/i, '');
}

export function toRelease(raw: RawRelease): Release {
  return {
    version: versionFromTag(raw.tag_name),
    tag: raw.tag_name,
    date: raw.published_at ?? raw.created_at ?? null,
    prerelease: raw.prerelease,
    installer: findInstaller(raw.assets),
  };
}

/** Fetches and maps releases; never throws. */
export async function loadReleases(
  fetcher: typeof fetch = fetch,
  apiUrl: string = process.env.RELEASES_API_URL || RELEASES_API_URL,
): Promise<ReleasesResult> {
  const failure: ReleasesResult = { status: 'error' };
  try {
    const response = await cachedFetchText(apiUrl, API_HEADERS, fetcher);
    if (!response.ok) return failure;
    const parsed: unknown = JSON.parse(response.body);
    if (!Array.isArray(parsed)) return failure;
    return {
      status: 'ok',
      releases: publishedReleases(parsed as RawRelease[]).map((item) => toRelease(item)),
    };
  } catch {
    return failure;
  }
}
