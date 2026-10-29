import { describe, expect, it, vi } from 'vitest';
import { downloadResponse, isValidVersion } from './download';
import type { ReleasesResult } from './releases';

const UPSTREAM = 'https://objects.example.test/Kivori_0.2.0_x64-setup.exe';

function loaded(withInstaller: boolean): () => Promise<ReleasesResult> {
  return async () => ({
    status: 'ok',
    releases: [
      {
        version: '0.2.0',
        tag: 'v0.2.0',
        date: '2026-02-01T00:00:00Z',
        prerelease: true,
        installer: withInstaller ? { size: 4, url: UPSTREAM } : null,
      },
    ],
  });
}

function request(headers: Record<string, string> = {}): Request {
  return new Request('https://kivori.example/download/windows/latest', { headers });
}

describe('downloadResponse', () => {
  it('streams the installer as an attachment with no outside location', async () => {
    const fetcher = vi.fn(
      async () => new Response('data', { status: 200, headers: { 'content-length': '4' } }),
    );
    const record = vi.fn();
    const res = await downloadResponse(request(), 'windows', 'latest', {
      load: loaded(true),
      fetcher: fetcher as unknown as typeof fetch,
      record,
    });
    expect(res.status).toBe(200);
    expect(res.headers.get('content-disposition')).toBe(
      'attachment; filename="Kivori-Setup-0.2.0.exe"',
    );
    expect(res.headers.get('content-type')).toBe('application/octet-stream');
    expect(res.headers.get('content-length')).toBe('4');
    expect(res.headers.get('cache-control')).toBe('no-store');
    expect(res.headers.get('location')).toBeNull();
    expect(await res.text()).toBe('data');
    expect(fetcher).toHaveBeenCalledWith(UPSTREAM, { headers: {}, redirect: 'follow' });
    expect(record).toHaveBeenCalledTimes(1);
  });

  it('forwards Range and the 206 content-range', async () => {
    const fetcher = vi.fn(
      async () =>
        new Response('at', {
          status: 206,
          headers: { 'content-length': '2', 'content-range': 'bytes 2-3/4' },
        }),
    );
    const record = vi.fn();
    const res = await downloadResponse(request({ range: 'bytes=2-3' }), 'windows', '0.2.0', {
      load: loaded(true),
      fetcher: fetcher as unknown as typeof fetch,
      record,
    });
    expect(fetcher).toHaveBeenCalledWith(UPSTREAM, {
      headers: { range: 'bytes=2-3' },
      redirect: 'follow',
    });
    expect(res.status).toBe(206);
    expect(res.headers.get('content-range')).toBe('bytes 2-3/4');
    expect(res.headers.get('location')).toBeNull();
    expect(record).not.toHaveBeenCalled();
  });

  it('redirects to /download, internally, when there is no asset', async () => {
    const fetcher = vi.fn();
    for (const version of ['latest', '0.2.0', '9.9.9']) {
      const res = await downloadResponse(request(), 'windows', version, {
        load: loaded(version === 'latest' ? false : version === '9.9.9'),
        fetcher: fetcher as unknown as typeof fetch,
        record: vi.fn(),
      });
      expect(res.status).toBe(302);
      expect(res.headers.get('location')).toBe('https://kivori.example/download');
    }
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('redirects internally when the upstream fails', async () => {
    const res = await downloadResponse(request(), 'windows', 'latest', {
      load: loaded(true),
      fetcher: (async () => new Response('x', { status: 404 })) as typeof fetch,
      record: vi.fn(),
    });
    expect(res.headers.get('location')).toBe('https://kivori.example/download');
  });

  it('404s for unsupported systems and malformed versions', async () => {
    const deps = { load: loaded(true), fetcher: vi.fn() as unknown as typeof fetch };
    expect((await downloadResponse(request(), 'linux', 'latest', deps)).status).toBe(404);
    expect((await downloadResponse(request(), 'windows', '../x', deps)).status).toBe(404);
  });
});

describe('isValidVersion', () => {
  it('accepts latest and semver only', () => {
    expect(isValidVersion('latest')).toBe(true);
    expect(isValidVersion('1.2.3')).toBe(true);
    expect(isValidVersion('1.2.3-beta.1')).toBe(true);
    expect(isValidVersion('1.2')).toBe(false);
    expect(isValidVersion('https://x')).toBe(false);
  });
});

it('never falls back to Windows when the Mac artifact is absent', async () => {
  const fetcher = vi.fn();
  expect(
    (await downloadResponse(request(), 'macos', 'latest', { load: loaded(true), fetcher })).status,
  ).toBe(302);
  expect(fetcher).not.toHaveBeenCalled();
});
it('streams the Apple Silicon dmg with macOS analytics', async () => {
  const record = vi.fn();
  const load = async (): Promise<ReleasesResult> => {
    const result = await loaded(true)();
    if (result.status === 'ok')
      result.releases[0].macInstaller = { size: 4, url: 'https://example.test/mac.dmg' };
    return result;
  };
  const fetcher = vi.fn(async () => new Response('data'));
  const response = await downloadResponse(request(), 'macos', 'latest', { load, fetcher, record });
  expect(response.status).toBe(200);
  expect(response.headers.get('content-type')).toBe('application/x-apple-diskimage');
  expect(response.headers.get('content-disposition')).toContain('Kivori-0.2.0-aarch64.dmg');
  expect(fetcher).toHaveBeenCalledWith('https://example.test/mac.dmg', {
    headers: {},
    redirect: 'follow',
  });
  expect(record).toHaveBeenCalledWith(expect.objectContaining({ os: 'macos' }));
});
