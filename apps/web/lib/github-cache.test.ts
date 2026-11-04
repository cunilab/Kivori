import { afterEach, describe, expect, it, vi } from 'vitest';
import { cachedFetchText, resetMemoryCache } from './github-cache';

afterEach(() => {
  resetMemoryCache();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
describe('release cache', () => {
  it('preserves the edge fetch time and bounds stale fallback', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(2_000_000_000_000);
    let stored: Response | undefined;
    vi.stubGlobal('caches', {
      default: {
        match: async () => stored?.clone(),
        put: async (_: Request, r: Response) => {
          stored = r.clone();
        },
      },
    });
    const fetcher = vi.fn(async () => new Response('first')) as unknown as typeof fetch;
    const first = await cachedFetchText('https://api.test/cache', {}, fetcher);
    expect(first.ok && first.fetchedAt).toBe(Date.now());
    resetMemoryCache();
    vi.advanceTimersByTime(599_000);
    const edge = await cachedFetchText('https://api.test/cache', {}, fetcher);
    expect(edge.ok && edge.fetchedAt).toBe(2_000_000_000_000);
    vi.advanceTimersByTime(2_000);
    const failed = vi.fn(async () => new Response('', { status: 503 })) as unknown as typeof fetch;
    expect(await cachedFetchText('https://api.test/cache', {}, failed)).toMatchObject({
      ok: true,
      stale: true,
      fetchedAt: 2_000_000_000_000,
    });
    vi.advanceTimersByTime(86_400_000);
    expect(await cachedFetchText('https://api.test/cache', {}, failed)).toEqual({
      ok: false,
      status: 503,
    });
  });
});

it('bounds an upstream request that never settles', async () => {
  vi.useFakeTimers();
  const pending = cachedFetchText(
    'https://api.test/hung',
    {},
    (() => new Promise(() => {})) as typeof fetch,
  );
  await vi.advanceTimersByTimeAsync(8000);
  expect(await pending).toEqual({ ok: false, status: 0 });
});
