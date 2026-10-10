import { cachedFetchText } from '@/lib/github-cache';
import { GITHUB_URL } from '@/lib/site';

export const RELEASES_API_URL = 'https://api.github.com/repos/cunilab/Kivori/releases?per_page=20';
export const RELEASES_PAGE_URL = `${GITHUB_URL}/releases`;
const INSTALLER_PATTERN = /-setup\.exe$/i;
const SUMS_NAME = 'SHA256SUMS';

const GITHUB_HEADERS = {
  'User-Agent': 'kivori-web (+https://github.com/cunilab/Kivori)',
  Accept: 'application/vnd.github+json',
  'X-GitHub-Api-Version': '2022-11-28',
};

interface GithubAsset {
  name: string;
  size: number;
  browser_download_url: string;
}

export interface GithubRelease {
  tag_name: string;
  name: string | null;
  draft: boolean;
  prerelease: boolean;
  published_at: string | null;
  created_at?: string | null;
  body: string | null;
  html_url: string;
  assets?: GithubAsset[];
}

export interface Installer {
  name: string;
  size: number;
  url: string;
}

export interface Release {
  version: string;
  tag: string;
  name: string;
  date: string | null;
  prerelease: boolean;
  /** Markdown release notes (untrusted; render without raw HTML). */
  body: string;
  htmlUrl: string;
  installer: Installer | null;
  /** Lowercase hex SHA-256 of the installer; null when there is no checksum file or entry. */
  sha256: string | null;
  sumsUrl: string | null;
}

export type ReleasesResult =
  { status: 'ok'; releases: Release[] } | { status: 'error'; releasesUrl: string };

function releaseTime(release: GithubRelease): number {
  const time = Date.parse(release.published_at ?? release.created_at ?? '');
  return Number.isNaN(time) ? 0 : time;
}

/** Non-draft releases (pre-releases included), newest first. */
export function publishedReleases(raw: readonly GithubRelease[]): GithubRelease[] {
  return raw.filter((release) => !release.draft).sort((a, b) => releaseTime(b) - releaseTime(a));
}

/** The newest non-draft release; `/releases/latest` would skip pre-releases, so pick it ourselves. */
export function pickLatest(raw: readonly GithubRelease[]): GithubRelease | undefined {
  return publishedReleases(raw)[0];
}

export function findInstaller(assets: readonly GithubAsset[] | undefined): Installer | null {
  const asset = assets?.find((candidate) => INSTALLER_PATTERN.test(candidate.name));
  return asset ? { name: asset.name, size: asset.size, url: asset.browser_download_url } : null;
}

/** Reads `<hex>  <file>` lines (sha256sum format, optional `*` binary marker) for one file. */
export function parseSha256Sums(text: string, fileName: string): string | null {
  for (const line of text.split(/\r?\n/)) {
    const match = /^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$/.exec(line.trim());
    if (match && match[2] === fileName) return match[1].toLowerCase();
  }
  return null;
}

export function versionFromTag(tag: string): string {
  return tag.replace(/^v/i, '');
}

export function toRelease(raw: GithubRelease, sha256: string | null = null): Release {
  const sums = raw.assets?.find((asset) => asset.name === SUMS_NAME);
  return {
    version: versionFromTag(raw.tag_name),
    tag: raw.tag_name,
    name: raw.name?.trim() || raw.tag_name,
    date: raw.published_at ?? raw.created_at ?? null,
    prerelease: raw.prerelease,
    body: raw.body ?? '',
    htmlUrl: raw.html_url,
    installer: findInstaller(raw.assets),
    sha256,
    sumsUrl: sums?.browser_download_url ?? null,
  };
}

/** Fetches and maps releases; never throws. Only the newest release's checksum is fetched. */
export async function loadReleases(
  fetcher: typeof fetch = fetch,
  apiUrl: string = process.env.RELEASES_API_URL || RELEASES_API_URL,
): Promise<ReleasesResult> {
  const failure: ReleasesResult = { status: 'error', releasesUrl: RELEASES_PAGE_URL };
  try {
    const response = await cachedFetchText(apiUrl, GITHUB_HEADERS, fetcher);
    if (!response.ok) return failure;
    const parsed: unknown = JSON.parse(response.body);
    if (!Array.isArray(parsed)) return failure;
    const releases = publishedReleases(parsed as GithubRelease[]).map((item) => toRelease(item));
    const latest = releases[0];
    if (latest?.installer && latest.sumsUrl) {
      const sums = await cachedFetchText(latest.sumsUrl, GITHUB_HEADERS, fetcher);
      if (sums.ok) latest.sha256 = parseSha256Sums(sums.body, latest.installer.name);
    }
    return { status: 'ok', releases };
  } catch {
    return failure;
  }
}
