import { getCloudflareContext } from '@opennextjs/cloudflare';

export const CACHE_TTL_SECONDS = 600;
export const MAX_STALE_SECONDS = 86400;
const TIMEOUT_MS = 8000;
interface Entry {
  body: string;
  fetchedAt: number;
}
const memory = new Map<string, Entry>();
interface EdgeCache {
  match(request: Request): Promise<Response | undefined>;
  put(request: Request, response: Response): Promise<void>;
}
function edgeCache(): EdgeCache | undefined {
  return (globalThis as { caches?: { default?: EdgeCache } }).caches?.default;
}
function waitUntil(promise: Promise<unknown>): void {
  try {
    getCloudflareContext().ctx.waitUntil(promise);
  } catch {
    void promise.catch(() => undefined);
  }
}
export function resetMemoryCache(): void {
  memory.clear();
}

/** Preserve origin fetch time across cache layers; stale data expires after one day. */
export async function cachedFetchText(
  url: string,
  headers: Record<string, string>,
  fetcher: typeof fetch = fetch,
): Promise<
  { ok: true; body: string; fetchedAt: number; stale: boolean } | { ok: false; status: number }
> {
  const now = Date.now();
  let hit = memory.get(url);
  const cache = edgeCache();
  const key = new Request(url);
  if (!hit && cache) {
    try {
      const response = await cache.match(key);
      const fetchedAt = Number(response?.headers.get('x-kivori-fetched-at'));
      if (response?.ok && fetchedAt > 0 && fetchedAt <= now) {
        hit = { body: await response.text(), fetchedAt };
        memory.set(url, hit);
      }
    } catch {
      /* cache is optional */
    }
  }
  if (hit && now - hit.fetchedAt < CACHE_TTL_SECONDS * 1000)
    return { ok: true, ...hit, stale: false };
  const controller = new AbortController();
  let timeout: ReturnType<typeof setTimeout>;
  const deadline = new Promise<never>((_, reject) => {
    timeout = setTimeout(() => {
      controller.abort();
      reject(new Error('timeout'));
    }, TIMEOUT_MS);
  });
  try {
    const body = await Promise.race([
      (async () => {
        const response = await fetcher(url, { headers, signal: controller.signal });
        if (!response.ok) throw response.status;
        return response.text();
      })(),
      deadline,
    ]);
    const entry = { body, fetchedAt: Date.now() };
    memory.set(url, entry);
    if (cache)
      waitUntil(
        cache.put(
          key,
          new Response(body, {
            headers: {
              'Cache-Control': `public, max-age=${MAX_STALE_SECONDS}`,
              'x-kivori-fetched-at': String(entry.fetchedAt),
            },
          }),
        ),
      );
    return { ok: true, ...entry, stale: false };
  } catch (error) {
    if (hit && Date.now() - hit.fetchedAt <= MAX_STALE_SECONDS * 1000)
      return { ok: true, ...hit, stale: true };
    return { ok: false, status: typeof error === 'number' ? error : 0 };
  } finally {
    clearTimeout(timeout!);
  }
}
