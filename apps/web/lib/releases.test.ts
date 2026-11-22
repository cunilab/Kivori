import { beforeEach, describe, expect, it } from 'vitest';
import { resetMemoryCache } from './github-cache';
import {
  parseReleaseSnapshot,
  findInstaller,
  findMacInstaller,
  loadReleases,
  pickLatest,
  toRelease,
  type RawRelease,
} from './releases';

function release(over: Partial<RawRelease>): RawRelease {
  return {
    tag_name: 'v0.1.0',
    draft: false,
    prerelease: false,
    published_at: '2026-01-01T00:00:00Z',
    assets: [],
    ...over,
  };
}

const installerAsset = {
  name: 'Kivori_0.2.0_x64-setup.exe',
  size: 1234,
  browser_download_url:
    'https://github.com/cunilab/Kivori/releases/download/v0.2.0/Kivori_0.2.0_x64-setup.exe',
};
const otherAsset = {
  name: 'notes.txt',
  size: 100,
  browser_download_url: 'https://example.test/notes.txt',
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
    expect(findInstaller([otherAsset, installerAsset])).toEqual({
      size: 1234,
      url: installerAsset.browser_download_url,
    });
    expect(findInstaller([{ ...installerAsset, name: 'KIVORI-SETUP.EXE' }])?.size).toBe(1234);
  });

  it('ignores other assets and a missing list', () => {
    expect(findInstaller([otherAsset, { ...installerAsset, name: 'kivori.dmg' }])).toBeNull();
    expect(findInstaller(undefined)).toBeNull();
  });
});

describe('toRelease', () => {
  it('maps the empty state: no assets, no installer', () => {
    const mapped = toRelease(release({ prerelease: true }));
    expect(mapped).toMatchObject({
      version: '0.1.0',
      tag: 'v0.1.0',
      prerelease: true,
      installer: null,
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

  it('maps releases and their installers', async () => {
    const api = 'https://api.test/releases';
    const fetcher = respond({
      [api]: JSON.stringify([
        release({ tag_name: 'v0.2.0', assets: [installerAsset, otherAsset] }),
        release({ tag_name: 'v0.1.0', published_at: '2025-12-01T00:00:00Z' }),
      ]),
    });
    const result = await loadReleases(fetcher, api);
    expect(result.status).toBe('ok');
    if (result.status !== 'ok') return;
    expect(result.releases.map((r) => r.version)).toEqual(['0.2.0', '0.1.0']);
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
    expect(await loadReleases(failing, 'https://api.test/down')).toEqual({ status: 'error' });
    expect(
      await loadReleases(respond({ 'https://api.test/403': 403 }), 'https://api.test/403'),
    ).toMatchObject({ status: 'error' });
    expect(
      await loadReleases(respond({ 'https://api.test/bad': 'not json' }), 'https://api.test/bad'),
    ).toMatchObject({ status: 'error' });
  });
});

it('selects only an unambiguous Apple Silicon dmg', () => {
  const asset = { ...installerAsset, name: 'Kivori_0.2.0_aarch64.dmg' };
  expect(findMacInstaller([asset])).toEqual({ size: asset.size, url: asset.browser_download_url });
  expect(findMacInstaller([{ ...asset, name: 'Kivori_x64.dmg' }])).toBeNull();
  expect(findMacInstaller([{ ...asset, name: 'Kivori.dmg' }])).toBeNull();
  expect(findMacInstaller([asset, asset])).toBeNull();
});

it('rejects unsafe and ambiguous installer URLs', () => {
  expect(
    findInstaller([{ ...installerAsset, browser_download_url: 'https://evil.test/file' }]),
  ).toBeNull();
  expect(findInstaller([installerAsset, installerAsset])).toBeNull();
});
it('filters firmware tags and unlabelled prerelease tags', async () => {
  const fetcher = (async () =>
    new Response(
      JSON.stringify([
        release({ tag_name: 'pip-v0.1.4' }),
        release({ tag_name: 'v0.3.0-beta.1' }),
        release({ tag_name: 'v0.3.0-beta.2', prerelease: true }),
        release({ tag_name: 'v0.2.0' }),
      ]),
    )) as typeof fetch;
  const result = await loadReleases(fetcher, 'https://api.test/filter');
  expect(result.status === 'ok' && result.releases.map((r) => r.version)).toEqual([
    '0.3.0-beta.2',
    '0.2.0',
  ]);
});
it('rejects malformed API records', async () => {
  const fetcher = (async () => new Response('[{"tag_name":"v1.0.0"}]')) as typeof fetch;
  expect(await loadReleases(fetcher, 'https://api.test/shape')).toEqual({ status: 'error' });
});
it('maps valid release metadata without hiding releases when metadata is invalid', async () => {
  const url = 'https://github.com/cunilab/Kivori/releases/download/v0.2.0/release-metadata.json';
  const item = release({
    tag_name: 'v0.2.0',
    assets: [{ name: 'release-metadata.json', size: 1, browser_download_url: url }],
  });
  const data = {
    schema_version: 1,
    desktop_version: '0.2.0',
    commit: 'a'.repeat(40),
    firmware: [{ product: 'pip', model: 'KV-P1', version: '0.1.4', boards: ['rp2040'] }],
  };
  const fetcher = (async (input: RequestInfo | URL) =>
    new Response(JSON.stringify(String(input) === url ? data : [item]))) as typeof fetch;
  const result = await loadReleases(fetcher, 'https://api.test/metadata');
  expect(result.status === 'ok' && result.releases[0]).toMatchObject({
    sourceCommit: 'a'.repeat(40),
    firmware: data.firmware,
  });
  resetMemoryCache();
  data.firmware[0].boards.push('rp2040');
  const invalid = await loadReleases(fetcher, 'https://api.test/metadata');
  expect(invalid.status === 'ok' && invalid.releases[0].firmware).toEqual([]);
});

it('rejects non-public or forbidden firmware model names', async () => {
  const url = 'https://github.com/cunilab/Kivori/releases/download/v0.2.0/release-metadata.json';
  const item = release({
    tag_name: 'v0.2.0',
    assets: [{ name: 'release-metadata.json', size: 1, browser_download_url: url }],
  });
  for (const model of ['ESP32', 'HW-040', 'github.com/cunilab/Kivori', 'KV-1 TBC']) {
    resetMemoryCache();
    const data = {
      schema_version: 1,
      desktop_version: '0.2.0',
      commit: 'a'.repeat(40),
      firmware: [{ product: 'pip', model, version: '0.1.4', boards: ['board'] }],
    };
    const fetcher = (async (input: RequestInfo | URL) =>
      new Response(JSON.stringify(String(input) === url ? data : [item]))) as typeof fetch;
    const result = await loadReleases(fetcher, 'https://api.test/model');
    expect(result.status === 'ok' && result.releases[0].firmware).toEqual([]);
  }
});

it('bounds metadata enrichment and skips it for download resolution', async () => {
  const items = Array.from({ length: 12 }, (_, index) => {
    const version = `1.0.${index}`;
    return release({
      tag_name: `v${version}`,
      published_at: `2026-01-${String(index + 1).padStart(2, '0')}T00:00:00Z`,
      assets: [
        {
          name: 'release-metadata.json',
          size: 100,
          browser_download_url: `https://github.com/cunilab/Kivori/releases/download/v${version}/release-metadata.json`,
        },
      ],
    });
  });
  let calls = 0,
    active = 0,
    maximum = 0;
  const fetcher = (async (input: RequestInfo | URL) => {
    if (String(input).startsWith('https://api.test/')) return new Response(JSON.stringify(items));
    calls++;
    active++;
    maximum = Math.max(maximum, active);
    await new Promise<void>((resolve) => queueMicrotask(resolve));
    active--;
    return new Response('{}');
  }) as typeof fetch;
  const result = await loadReleases(fetcher, 'https://api.test/budget');
  expect(result.status === 'ok' && result.releases.length).toBe(12);
  expect(calls).toBe(4);
  expect(maximum).toBeLessThanOrEqual(2);
  resetMemoryCache();
  calls = 0;
  await loadReleases(fetcher, 'https://api.test/budget', { includeMetadata: false });
  expect(calls).toBe(0);
});

it('reads later release pages when firmware-only tags fill the first page', async () => {
  const seen: string[] = [];
  const fetcher = (async (input: RequestInfo | URL) => {
    const url = String(input);
    seen.push(url);
    return new Response(
      JSON.stringify(
        new URL(url).searchParams.has('page')
          ? [release({ tag_name: 'v0.3.1' })]
          : Array.from({ length: 100 }, () => release({ tag_name: 'fw-pip-v0.1.4' })),
      ),
    );
  }) as typeof fetch;
  const result = await loadReleases(fetcher, 'https://api.test/pages?per_page=100');
  expect(result.status === 'ok' && result.releases.map((item) => item.version)).toEqual(['0.3.1']);
  expect(seen).toHaveLength(2);
});

it('keeps separate product and board firmware mappings in one desktop release', async () => {
  const url = 'https://github.com/cunilab/Kivori/releases/download/v0.2.0/release-metadata.json';
  const item = release({
    tag_name: 'v0.2.0',
    assets: [{ name: 'release-metadata.json', size: 100, browser_download_url: url }],
  });
  const firmware = [
    { product: 'dial', model: 'KV-D1', version: '0.1.4', boards: ['board-a', 'board-b'] },
    { product: 'keys', model: 'KV-K2', version: '1.2.0', boards: ['board-a'] },
  ];
  const fetcher = (async (input: RequestInfo | URL) =>
    new Response(
      JSON.stringify(
        String(input) === url
          ? { schema_version: 1, desktop_version: '0.2.0', commit: 'a'.repeat(40), firmware }
          : [item],
      ),
    )) as typeof fetch;
  const result = await loadReleases(fetcher, 'https://api.test/products');
  expect(result.status === 'ok' && result.releases[0].firmware).toEqual(firmware);
});

it('discovers Linux packages and validates them through the published snapshot', () => {
  const assets = ['Kivori_0.1.0_amd64.deb', 'Kivori_0.1.0_amd64.AppImage'].map((name) => ({
    name,
    size: 42,
    browser_download_url: `https://github.com/cunilab/Kivori/releases/download/v0.1.0/${name}`,
  }));
  const linux = toRelease(release({ assets }));
  expect(linux.linuxDeb?.url).toContain('_amd64.deb');
  expect(linux.linuxAppImage?.url).toContain('_amd64.AppImage');
  const snapshot = (r: typeof linux) =>
    JSON.stringify({ schema_version: 1, synced_at: 1234, releases: [r] });
  expect(parseReleaseSnapshot(snapshot(linux))?.releases[0].linuxDeb).toEqual(linux.linuxDeb);
  expect(
    parseReleaseSnapshot(
      snapshot({ ...linux, linuxDeb: { size: 42, url: 'https://evil.test/package.deb' } }),
    ),
  ).toBeNull();
  expect(toRelease(release({ assets: [assets[0], assets[0]] })).linuxDeb).toBeNull();
  expect(
    toRelease(release({ assets: [{ ...assets[0], name: 'Kivori_0.1.0_arm64.deb' }] })).linuxDeb,
  ).toBeNull();
});
