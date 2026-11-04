import { beforeEach, expect, it } from 'vitest';
import { syncWebReleases } from '../../../scripts/sync-web-releases.mjs';
import { resetMemoryCache } from './github-cache';
import { parseReleaseSnapshot } from './releases';

const config = {
  accountId: 'a'.repeat(32),
  databaseId: '00000000-0000-0000-0000-000000000001',
  githubToken: 'test-github',
  cloudflareToken: 'test-cloudflare',
};
const asset = 'https://github.com/cunilab/Kivori/releases/download/v0.3.1/release-metadata.json';
const published = {
  tag_name: 'v0.3.1',
  draft: false,
  prerelease: false,
  published_at: '2026-11-03T00:00:00Z',
  body: "Preserve settings. '); DROP TABLE waitlist; --",
  assets: [{ name: 'release-metadata.json', size: 100, browser_download_url: asset }],
};

beforeEach(() => resetMemoryCache());

it('authenticates each provider separately and persists only published data using bound parameters', async () => {
  let write: { sql: string; params: string[] } | undefined;
  const fetcher = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input),
      headers = new Headers(init?.headers);
    if (url.startsWith('https://api.github.com/')) {
      expect(headers.get('Authorization')).toBe('Bearer test-github');
      return Response.json([
        published,
        { ...published, tag_name: 'v0.4.0', draft: true, body: 'unpublished notes' },
      ]);
    }
    if (url === asset) {
      expect(headers.has('Authorization')).toBe(false);
      return Response.json({
        schema_version: 1,
        desktop_version: '0.3.1',
        commit: 'a'.repeat(40),
        firmware: [{ product: 'pip', model: 'KV-P1', version: '0.1.4', boards: ['board'] }],
      });
    }
    expect(new URL(url).hostname).toBe('api.cloudflare.com');
    expect(headers.get('Authorization')).toBe('Bearer test-cloudflare');
    write = JSON.parse(String(init?.body));
    return Response.json({ success: true, result: [{ success: true }] });
  }) as typeof fetch;
  expect(await syncWebReleases({ ...config, fetcher })).toMatchObject({ releases: 1 });
  expect(write?.sql).not.toContain('DROP TABLE');
  expect(write?.params[0]).toContain('DROP TABLE');
  expect(write?.params[0]).not.toContain('unpublished notes');
  const snapshot = parseReleaseSnapshot(write!.params[0]!);
  expect(snapshot?.releases[0]?.firmware[0]).toMatchObject({ model: 'KV-P1', version: '0.1.4' });
});

it('does not overwrite the last snapshot after an upstream failure', async () => {
  let writes = 0;
  const fetcher = (async (input: RequestInfo | URL) => {
    if (String(input).startsWith('https://api.cloudflare.com/')) writes++;
    return new Response('', { status: 403 });
  }) as typeof fetch;
  await expect(syncWebReleases({ ...config, fetcher })).rejects.toThrow('Unable to fetch a fresh');
  expect(writes).toBe(0);
});

it('publishes a successful empty result so withdrawn releases disappear', async () => {
  let payload: string | undefined;
  const fetcher = (async (input: RequestInfo | URL, init?: RequestInit) => {
    if (String(input).startsWith('https://api.github.com/')) return Response.json([]);
    payload = JSON.parse(String(init?.body)).params[0];
    return Response.json({ success: true, result: [{ success: true }] });
  }) as typeof fetch;
  expect(await syncWebReleases({ ...config, fetcher })).toMatchObject({ releases: 0 });
  expect(parseReleaseSnapshot(payload!)?.releases).toEqual([]);
});

it('rejects malformed, cross-repository, and draft snapshot records', () => {
  const base = {
    version: '0.3.1',
    tag: 'v0.3.1',
    date: null,
    prerelease: false,
    notes: '',
    firmware: [],
    installer: null,
  };
  const encode = (releases: unknown[]) =>
    JSON.stringify({ schema_version: 1, synced_at: Date.now(), releases });
  expect(parseReleaseSnapshot(encode([base]))).not.toBeNull();
  for (const record of [
    { ...base, draft: true },
    { ...base, version: '0.3.2' },
    { ...base, installer: { size: 100, url: 'https://evil.example/file-setup.exe' } },
    {
      ...base,
      firmware: [{ product: 'pip', model: 'KV-P1', version: '0.1.4', boards: ['board'] }],
    },
  ])
    expect(parseReleaseSnapshot(encode([record]))).toBeNull();
  expect(parseReleaseSnapshot(encode([base, base]))).toBeNull();
  expect(parseReleaseSnapshot(encode([{ ...base, notes: 'x'.repeat(1_000_001) }]))).toBeNull();
});

it.each(['denied', 'malformed'])(
  'retains the snapshot when advertised metadata is %s',
  async (failure) => {
    let writes = 0;
    const fetcher = (async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.startsWith('https://api.github.com/')) return Response.json([published]);
      if (url === asset)
        return failure === 'denied'
          ? new Response('', { status: 403 })
          : Response.json({ schema_version: 99 });
      writes++;
      return Response.json({ success: true, result: [{ success: true }] });
    }) as typeof fetch;
    await expect(syncWebReleases({ ...config, fetcher })).rejects.toThrow(
      'Unable to fetch a fresh',
    );
    expect(writes).toBe(0);
  },
);
