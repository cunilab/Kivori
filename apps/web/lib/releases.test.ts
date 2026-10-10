import { beforeEach, describe, expect, it } from 'vitest';
import { resetMemoryCache } from './github-cache';
import {
  findInstaller,
  loadReleases,
  parseSha256Sums,
  pickLatest,
  toRelease,
  type GithubRelease,
} from './releases';

const HEX = 'a'.repeat(64);
const OTHER = 'b'.repeat(64);

function release(over: Partial<GithubRelease>): GithubRelease {
  return {
    tag_name: 'v0.1.0',
    name: 'v0.1.0',
    draft: false,
    prerelease: false,
    published_at: '2026-01-01T00:00:00Z',
    body: 'notes',
    html_url: 'https://github.com/cunilab/Kivori/releases/tag/v0.1.0',
    assets: [],
    ...over,
  };
}

const installerAsset = {
  name: 'Kivori_0.2.0_x64-setup.exe',
  size: 1234,
  browser_download_url: 'https://example.test/Kivori_0.2.0_x64-setup.exe',
};
const sumsAsset = {
  name: 'SHA256SUMS',
  size: 100,
  browser_download_url: 'https://example.test/SHA256SUMS',
};

beforeEach(() => resetMemoryCache());

describe('pickLatest', () => {
  it('skips drafts and keeps pre-releases', () => {
    const picked = pickLatest([
      release({ tag_name: 'v0.3.0', draft: true, published_at: '2026-03-01T00:00:00Z' }),
      release({ tag_name: 'v0.2.0', prerelease: true, published_at: '2026-02-01T00:00:00Z' }),
      release({ tag_name: 'v0.1.0' }),
    ]);
    expect(picked?.tag_name).toBe('v0.2.0');
  });

  it('sorts by date, not by array order', () => {
    const picked = pickLatest([
      release({ tag_name: 'v0.1.0', published_at: '2026-01-01T00:00:00Z' }),
      release({ tag_name: 'v0.2.0', published_at: '2026-02-01T00:00:00Z' }),
    ]);
    expect(picked?.tag_name).toBe('v0.2.0');
  });

  it('returns undefined when there is nothing published', () => {
    expect(pickLatest([release({ draft: true })])).toBeUndefined();
    expect(pickLatest([])).toBeUndefined();
  });
});

describe('findInstaller', () => {
  it('matches the -setup.exe asset case-insensitively', () => {
    expect(findInstaller([sumsAsset, installerAsset])).toEqual({
      name: installerAsset.name,
      size: 1234,
      url: installerAsset.browser_download_url,
    });
    expect(findInstaller([{ ...installerAsset, name: 'KIVORI-SETUP.EXE' }])?.name).toBe(
      'KIVORI-SETUP.EXE',
    );
  });

  it('ignores other assets and a missing list', () => {
    expect(findInstaller([sumsAsset, { ...installerAsset, name: 'kivori.dmg' }])).toBeNull();
    expect(findInstaller(undefined)).toBeNull();
  });
});

describe('parseSha256Sums', () => {
  const text = `${HEX}  Kivori_0.2.0_x64-setup.exe\r\n${OTHER.toUpperCase()} *other.dmg\n`;

  it('finds the hex for a file', () => {
    expect(parseSha256Sums(text, 'Kivori_0.2.0_x64-setup.exe')).toBe(HEX);
    expect(parseSha256Sums(text, 'other.dmg')).toBe(OTHER);
  });

  it('returns null for an unknown file or malformed lines', () => {
    expect(parseSha256Sums(text, 'missing.exe')).toBeNull();
    expect(parseSha256Sums('nothex  a-setup.exe', 'a-setup.exe')).toBeNull();
  });
});

describe('toRelease', () => {
  it('maps the empty state: no assets, no installer', () => {
    const mapped = toRelease(release({ prerelease: true }));
    expect(mapped).toMatchObject({
      version: '0.1.0',
      tag: 'v0.1.0',
      prerelease: true,
      body: 'notes',
      installer: null,
      sha256: null,
      sumsUrl: null,
    });
  });
});

describe('loadReleases', () => {
  function respond(routes: Record<string, string | number>): typeof fetch {
    return (async (input: RequestInfo | URL) => {
      const hit = routes[String(input)];
      if (typeof hit === 'number' || hit === undefined) {
        return new Response('nope', { status: hit ?? 404 });
      }
      return new Response(hit);
    }) as typeof fetch;
  }

  it('maps releases and reads the installer checksum', async () => {
    const api = 'https://api.test/releases';
    const fetcher = respond({
      [api]: JSON.stringify([
        release({ tag_name: 'v0.2.0', assets: [installerAsset, sumsAsset] }),
        release({ tag_name: 'v0.1.0', published_at: '2025-12-01T00:00:00Z' }),
      ]),
      [sumsAsset.browser_download_url]: `${HEX}  ${installerAsset.name}\n`,
    });
    const result = await loadReleases(fetcher, api);
    expect(result.status).toBe('ok');
    if (result.status !== 'ok') return;
    expect(result.releases.map((r) => r.version)).toEqual(['0.2.0', '0.1.0']);
    expect(result.releases[0].sha256).toBe(HEX);
    expect(result.releases[0].installer?.size).toBe(1234);
  });

  it('keeps the empty state for a release without assets', async () => {
    const api = 'https://api.test/empty';
    const result = await loadReleases(respond({ [api]: JSON.stringify([release({})]) }), api);
    expect(result.status === 'ok' && result.releases[0].installer).toBeNull();
  });

  it('falls back instead of throwing when the API fails', async () => {
    const failing = (async () => {
      throw new Error('offline');
    }) as typeof fetch;
    expect(await loadReleases(failing, 'https://api.test/down')).toEqual({
      status: 'error',
      releasesUrl: 'https://github.com/cunilab/Kivori/releases',
    });
    expect(
      await loadReleases(respond({ 'https://api.test/403': 403 }), 'https://api.test/403'),
    ).toMatchObject({ status: 'error' });
    expect(
      await loadReleases(respond({ 'https://api.test/bad': 'not json' }), 'https://api.test/bad'),
    ).toMatchObject({ status: 'error' });
  });
});
