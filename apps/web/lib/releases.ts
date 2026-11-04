import { cachedFetchText, CACHE_TTL_SECONDS, MAX_STALE_SECONDS } from '@/lib/github-cache';
import { readReleaseSnapshot } from '@/lib/release-snapshot';

export const RELEASES_API_URL = 'https://api.github.com/repos/cunilab/Kivori/releases?per_page=100';
const API_HEADERS = {
  'User-Agent': 'kivori-web',
  Accept: 'application/vnd.github+json',
  'X-GitHub-Api-Version': '2022-11-28',
};
const MAX_METADATA_RELEASES = 4;
const TAG = /^v(\d+\.\d+\.\d+)(?:-([0-9A-Za-z.-]+))?$/;
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
  body?: string | null;
  assets?: RawAsset[];
}
export interface Installer {
  size: number;
  url: string;
}
export interface Firmware {
  product: string;
  model: string;
  version: string;
  boards: string[];
}
export interface Release {
  version: string;
  tag: string;
  date: string | null;
  prerelease: boolean;
  installer: Installer | null;
  macInstaller?: Installer | null;
  notes: string;
  firmware: Firmware[];
  sourceCommit?: string;
}
export type ReleasesResult =
  { status: 'ok'; releases: Release[]; fetchedAt?: number; stale?: boolean } | { status: 'error' };

export const MAX_SNAPSHOT_BYTES = 1_000_000;
export interface ReleaseSnapshot {
  schema_version: 1;
  synced_at: number;
  releases: Release[];
}

function validSnapshotInstaller(value: unknown, tag: string, pattern: RegExp): boolean {
  if (value === null) return true;
  if (!value || typeof value !== 'object') return false;
  const item = value as Installer;
  if (typeof item.url !== 'string') return false;
  try {
    const url = new URL(item.url);
    const name = decodeURIComponent(url.pathname.slice(url.pathname.lastIndexOf('/') + 1));
    return (
      pattern.test(name) &&
      validAsset({ name, size: item.size, browser_download_url: item.url }) &&
      url.pathname ===
        `/cunilab/Kivori/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`
    );
  } catch {
    return false;
  }
}

/** Validate persisted data before it can select a download or render public notes. */
export function parseReleaseSnapshot(payload: string): ReleaseSnapshot | null {
  if (new TextEncoder().encode(payload).byteLength > MAX_SNAPSHOT_BYTES) return null;
  try {
    const data = JSON.parse(payload) as ReleaseSnapshot;
    if (
      data.schema_version !== 1 ||
      !Number.isSafeInteger(data.synced_at) ||
      data.synced_at <= 0 ||
      !Array.isArray(data.releases) ||
      data.releases.length > 300
    )
      return null;
    const tags = new Set<string>();
    for (const value of data.releases) {
      if (!value || typeof value !== 'object') return null;
      const r = value as Release & { draft?: unknown };
      const tag = typeof r.tag === 'string' ? TAG.exec(r.tag) : null;
      if (
        !tag ||
        r.version !== tag[1] + (tag[2] ? `-${tag[2]}` : '') ||
        typeof r.prerelease !== 'boolean' ||
        (tag[2] && !r.prerelease) ||
        r.draft === true ||
        (r.date !== null && (typeof r.date !== 'string' || !Number.isFinite(Date.parse(r.date)))) ||
        typeof r.notes !== 'string' ||
        tags.has(r.tag) ||
        !Array.isArray(r.firmware) ||
        !validSnapshotInstaller(r.installer, r.tag, /-setup\.exe$/i) ||
        (r.macInstaller !== undefined &&
          !validSnapshotInstaller(r.macInstaller, r.tag, /(?:_|-)aarch64\.dmg$/i))
      )
        return null;
      tags.add(r.tag);
      if (
        r.sourceCommit !== undefined &&
        (typeof r.sourceCommit !== 'string' || !/^[a-f0-9]{40}$/i.test(r.sourceCommit))
      )
        return null;
      if (
        r.firmware.length &&
        (!r.sourceCommit ||
          !metadata(
            {
              schema_version: 1,
              desktop_version: r.version,
              commit: r.sourceCommit,
              firmware: r.firmware,
            },
            r.version,
          ))
      )
        return null;
    }
    data.releases.sort((a, b) => (Date.parse(b.date ?? '') || 0) - (Date.parse(a.date ?? '') || 0));
    return data;
  } catch {
    return null;
  }
}

/** Production reads the authenticated publisher snapshot before attempting the public API. */
export async function loadReleases(
  fetcher: typeof fetch = fetch,
  apiUrl: string = process.env.RELEASES_API_URL || RELEASES_API_URL,
  options: { includeMetadata?: boolean } = {},
): Promise<ReleasesResult> {
  const payload = await readReleaseSnapshot();
  const snapshot = payload ? parseReleaseSnapshot(payload) : null;
  const age = snapshot ? Date.now() - snapshot.synced_at : Infinity;
  if (snapshot && age >= -60_000 && age <= MAX_STALE_SECONDS * 1000) {
    return {
      status: 'ok',
      releases: snapshot.releases,
      fetchedAt: snapshot.synced_at,
      stale: age > CACHE_TTL_SECONDS * 1000,
    };
  }
  return loadUpstreamReleases(fetcher, apiUrl, options);
}

function releaseTime(release: RawRelease): number {
  return Date.parse(release.published_at ?? release.created_at ?? '') || 0;
}
export function publishedReleases(raw: readonly RawRelease[]): RawRelease[] {
  return raw
    .filter((r) => !r.draft && TAG.test(r.tag_name) && (!TAG.exec(r.tag_name)?.[2] || r.prerelease))
    .sort((a, b) => releaseTime(b) - releaseTime(a));
}
export function pickLatest(raw: readonly RawRelease[]): RawRelease | undefined {
  return publishedReleases(raw)[0];
}
function validAsset(asset: RawAsset): boolean {
  if (
    typeof asset.name !== 'string' ||
    !Number.isSafeInteger(asset.size) ||
    asset.size <= 0 ||
    typeof asset.browser_download_url !== 'string'
  )
    return false;
  try {
    const url = new URL(asset.browser_download_url);
    return (
      url.protocol === 'https:' &&
      url.hostname === 'github.com' &&
      !url.username &&
      !url.password &&
      !url.port &&
      url.pathname.startsWith('/cunilab/Kivori/releases/download/') &&
      !url.search &&
      !url.hash
    );
  } catch {
    return false;
  }
}
function findAsset(assets: readonly RawAsset[] | undefined, pattern: RegExp): Installer | null {
  const matches = assets?.filter((a) => pattern.test(a.name)) ?? [];
  return matches.length === 1 && validAsset(matches[0])
    ? { size: matches[0].size, url: matches[0].browser_download_url }
    : null;
}
export function findInstaller(assets: readonly RawAsset[] | undefined): Installer | null {
  return findAsset(assets, /-setup\.exe$/i);
}
export function findMacInstaller(assets: readonly RawAsset[] | undefined): Installer | null {
  return findAsset(assets, /(?:_|-)aarch64\.dmg$/i);
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
    macInstaller: findMacInstaller(raw.assets),
    notes: raw.body ?? '',
    firmware: [],
  };
}
function validRelease(value: unknown): value is RawRelease {
  if (!value || typeof value !== 'object') return false;
  const r = value as Record<string, unknown>;
  return (
    typeof r.tag_name === 'string' &&
    typeof r.draft === 'boolean' &&
    typeof r.prerelease === 'boolean' &&
    (r.published_at === null || typeof r.published_at === 'string') &&
    (r.created_at === undefined || r.created_at === null || typeof r.created_at === 'string') &&
    (r.body === undefined || r.body === null || typeof r.body === 'string') &&
    (r.assets === undefined ||
      (Array.isArray(r.assets) &&
        r.assets.every(
          (a: unknown) =>
            !!a &&
            typeof a === 'object' &&
            typeof (a as RawAsset).name === 'string' &&
            typeof (a as RawAsset).size === 'number' &&
            typeof (a as RawAsset).browser_download_url === 'string',
        )))
  );
}
function metadata(
  value: unknown,
  version: string,
): { firmware: Firmware[]; sourceCommit: string } | null {
  if (!value || typeof value !== 'object') return null;
  const m = value as Record<string, unknown>;
  if (
    m.schema_version !== 1 ||
    m.desktop_version !== version ||
    typeof m.commit !== 'string' ||
    !/^[a-f0-9]{40}$/i.test(m.commit) ||
    !Array.isArray(m.firmware)
  )
    return null;
  const identities = new Set<string>(),
    models = new Set<string>(),
    targets = new Set<string>();
  for (const item of m.firmware) {
    if (
      !item ||
      typeof item !== 'object' ||
      typeof item.product !== 'string' ||
      !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(item.product) ||
      typeof item.model !== 'string' ||
      !/^[A-Z]{2}-[A-Z0-9]+$/.test(item.model) ||
      /ESP32|ST7789|HW-040|SuperMini|blueprint|\bTBC\b/i.test(item.model) ||
      typeof item.version !== 'string' ||
      !/^\d+\.\d+\.\d+$/.test(item.version) ||
      !Array.isArray(item.boards) ||
      !item.boards.length ||
      item.boards.some(
        (b: unknown) => typeof b !== 'string' || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(b),
      )
    )
      return null;
    if (identities.has(item.product) || models.has(item.model)) return null;
    identities.add(item.product);
    models.add(item.model);
    for (const board of item.boards) {
      const target = `${item.product}\0${board}`;
      if (targets.has(target)) return null;
      targets.add(target);
    }
  }
  return { firmware: m.firmware as Firmware[], sourceCommit: m.commit };
}
/** Fetches up to 300 releases. shortcut: older history beyond 300 releases is omitted, add pagination UI when needed. */
export async function loadUpstreamReleases(
  fetcher: typeof fetch = fetch,
  apiUrl: string = process.env.RELEASES_API_URL || RELEASES_API_URL,
  {
    includeMetadata = true,
    strictMetadata = false,
  }: { includeMetadata?: boolean; strictMetadata?: boolean } = {},
): Promise<ReleasesResult> {
  try {
    const raw: RawRelease[] = [];
    let fetchedAt = Date.now(),
      stale = false;
    for (let page = 1; page <= 3; page++) {
      const url =
        page === 1
          ? apiUrl
          : (() => {
              const u = new URL(apiUrl);
              u.searchParams.set('page', String(page));
              return u.href;
            })();
      const response = await cachedFetchText(url, API_HEADERS, fetcher);
      if (!response.ok) {
        console.error('release list unavailable', response.status);
        return { status: 'error' };
      }
      const parsed: unknown = JSON.parse(response.body);
      if (!Array.isArray(parsed) || !parsed.every(validRelease))
        throw new Error('invalid release list');
      raw.push(...parsed);
      fetchedAt = Math.min(fetchedAt, response.fetchedAt);
      stale ||= response.stale;
      if (parsed.length < 100) break;
    }
    const published = publishedReleases(raw);
    const releases = published.map((r) => {
      const assetsForTag = r.assets?.filter((a) => {
        if (!validAsset(a)) return false;
        const url = new URL(a.browser_download_url);
        return (
          url.pathname ===
          `/cunilab/Kivori/releases/download/${encodeURIComponent(r.tag_name)}/${encodeURIComponent(a.name)}`
        );
      });
      const result = toRelease({ ...r, assets: assetsForTag ?? [] });
      return result;
    });
    if (includeMetadata) {
      // shortcut: enrich four recent bundles; load metadata on demand if older firmware history is needed.
      const recent = releases.slice(0, MAX_METADATA_RELEASES);
      const stable = releases.find((r) => !r.prerelease);
      if (stable && !recent.includes(stable)) recent.splice(MAX_METADATA_RELEASES - 1, 1, stable);
      await Promise.all(
        [0, 1].map(async (lane) => {
          for (let index = lane; index < recent.length; index += 2) {
            const result = recent[index];
            const rawRelease = published.find((r) => r.tag_name === result.tag);
            const assets =
              rawRelease?.assets?.filter((a) => a.name === 'release-metadata.json') ?? [];
            if (!assets.length) continue;
            if (assets.length !== 1 || !validAsset(assets[0])) {
              if (strictMetadata) throw new Error('Invalid advertised metadata');
              continue;
            }
            const expected = `/cunilab/Kivori/releases/download/${encodeURIComponent(result.tag)}/release-metadata.json`;
            if (new URL(assets[0].browser_download_url).pathname !== expected) {
              if (strictMetadata) throw new Error('Invalid advertised metadata URL');
              continue;
            }
            try {
              const response = await cachedFetchText(
                assets[0].browser_download_url,
                API_HEADERS,
                fetcher,
              );
              if (response.ok) {
                const m = metadata(JSON.parse(response.body), result.version);
                if (strictMetadata && (!m || response.stale))
                  throw new Error('Metadata unavailable');
                if (m) Object.assign(result, m);
                fetchedAt = Math.min(fetchedAt, response.fetchedAt);
                stale ||= response.stale;
              } else if (strictMetadata) throw new Error('Metadata unavailable');
            } catch {
              if (strictMetadata) throw new Error('Metadata unavailable');
              /* optional metadata must not hide history */
            }
          }
        }),
      );
    }
    return { status: 'ok', releases, fetchedAt, stale };
  } catch {
    console.error('release list invalid or unavailable');
    return { status: 'error' };
  }
}
