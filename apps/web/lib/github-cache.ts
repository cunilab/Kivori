import { getCloudflareContext } from '@opennextjs/cloudflare';

export const CACHE_TTL_SECONDS = 600;

interface MemoryEntry {
  expires: number;
  body: string;
}

// Per-isolate layer: always available (dev, Node, tests) and also covers workers.dev, where the
// Workers Cache API accepts writes but never stores them.
const memory = new Map<string, MemoryEntry>();

interface EdgeCache {
  match(request: Request): Promise<Response | undefined>;
  put(request: Request, response: Response): Promise<void>;
}

function edgeCache(): EdgeCache | undefined {
  const store = (globalThis as { caches?: { default?: EdgeCache } }).caches;
  return store?.default;
}

function waitUntil(promise: Promise<unknown>): void {
  try {
    getCloudflareContext().ctx.waitUntil(promise);
  } catch {
    void promise.catch(() => undefined);
  }
}

/** Clears the per-isolate layer; for tests. */
export function resetMemoryCache(): void {
  memory.clear();
}

/**
 * GET text through a 600 s cache: per-isolate memory, then the Workers Cache API (`caches.default`)
 * when running in a Worker. Only 2xx responses are cached. When the network fails or answers non-2xx,
 * a stale in-memory copy is served if there is one. Returns `{ ok: false }` rather than throwing.
 */
export async function cachedFetchText(
  url: string,
  headers: Record<string, string>,
  fetcher: typeof fetch = fetch,
): Promise<{ ok: true; body: string } | { ok: false; status: number }> {
  const now = Date.now();
  const hit = memory.get(url);
  if (hit && hit.expires > now) return { ok: true, body: hit.body };

  const cache = edgeCache();
  const key = new Request(url, { method: 'GET' });
  if (cache) {
    try {
      const cached = await cache.match(key);
      if (cached?.ok) {
        const body = await cached.text();
        memory.set(url, { expires: now + CACHE_TTL_SECONDS * 1000, body });
        return { ok: true, body };
      }
    } catch {
      // a broken edge cache must not break the page
    }
  }

  try {
    const response = await fetcher(url, { headers });
    if (!response.ok) {
      return hit ? { ok: true, body: hit.body } : { ok: false, status: response.status };
    }
    const body = await response.text();
    memory.set(url, { expires: now + CACHE_TTL_SECONDS * 1000, body });
    if (cache) {
      const stored = new Response(body, {
        headers: { 'Cache-Control': `public, max-age=${CACHE_TTL_SECONDS}` },
      });
      waitUntil(cache.put(key, stored));
    }
    return { ok: true, body };
  } catch {
    return hit ? { ok: true, body: hit.body } : { ok: false, status: 0 };
  }
}
