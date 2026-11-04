// Authenticated publication/reconciliation writes one atomic public snapshot; tokens stay in CI.
import { readFileSync } from 'node:fs';
import {
  loadUpstreamReleases,
  parseReleaseSnapshot,
  RELEASES_API_URL,
} from '../apps/web/lib/releases.ts';

export async function syncWebReleases({
  accountId,
  databaseId,
  githubToken,
  cloudflareToken,
  fetcher = fetch,
  now = Date.now,
}) {
  if (
    !/^[a-f0-9]{32}$/i.test(accountId) ||
    !/^[a-f0-9]{8}-(?:[a-f0-9]{4}-){3}[a-f0-9]{12}$/i.test(databaseId) ||
    !githubToken ||
    !cloudflareToken
  )
    throw new Error('Missing or invalid release publisher configuration');
  const startedAt = now();
  const authenticatedFetch = async (input, init = {}) => {
    const url = new URL(String(input));
    const headers = new Headers(init.headers);
    if (url.origin === 'https://api.github.com')
      headers.set('Authorization', `Bearer ${githubToken}`);
    else headers.delete('Authorization');
    return fetcher(input, { ...init, headers });
  };
  const result = await loadUpstreamReleases(authenticatedFetch, RELEASES_API_URL, {
    strictMetadata: true,
  });
  if (result.status !== 'ok' || result.stale)
    throw new Error('Unable to fetch a fresh published release list');
  const payload = JSON.stringify({
    schema_version: 1,
    synced_at: startedAt,
    releases: result.releases,
  });
  if (!parseReleaseSnapshot(payload))
    throw new Error('Invalid or oversized published release snapshot');
  const response = await fetcher(
    `https://api.cloudflare.com/client/v4/accounts/${accountId}/d1/database/${databaseId}/query`,
    {
      method: 'POST',
      headers: { Authorization: `Bearer ${cloudflareToken}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({
        sql: `INSERT INTO release_snapshots (id, payload, synced_at) VALUES (1, ?1, ?2)
        ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, synced_at = excluded.synced_at
        WHERE excluded.synced_at >= release_snapshots.synced_at`,
        params: [payload, String(startedAt)],
      }),
      signal: AbortSignal.timeout(15_000),
    },
  );
  if (!response.ok) throw new Error(`Release snapshot write failed (${response.status})`);
  const written = await response.json();
  if (
    written.success !== true ||
    !Array.isArray(written.result) ||
    written.result.length !== 1 ||
    written.result[0].success !== true
  )
    throw new Error('Release snapshot write failed');
  return { releases: result.releases.length, syncedAt: startedAt };
}

if (import.meta.main) {
  try {
    const config = Bun.JSONC.parse(
      readFileSync(new URL('../apps/web/wrangler.jsonc', import.meta.url), 'utf8'),
    );
    const databaseId = config.d1_databases.find(
      (database) => database.binding === 'WAITLIST_DB',
    )?.database_id;
    const result = await syncWebReleases({
      accountId: process.env.CLOUDFLARE_ACCOUNT_ID,
      databaseId,
      githubToken: process.env.GH_TOKEN,
      cloudflareToken: process.env.CLOUDFLARE_API_TOKEN,
    });
    console.log(`Published release snapshot refreshed: ${result.releases} releases`);
  } catch (error) {
    // Provider response bodies and credentials are deliberately excluded.
    const safe =
      /^(?:Missing or invalid release publisher configuration|Unable to fetch a fresh published release list|Invalid or oversized published release snapshot|Release snapshot write failed(?: \([1-5][0-9]{2}\))?)$/;
    console.error(
      error instanceof Error && safe.test(error.message)
        ? error.message
        : 'Release snapshot refresh failed',
    );
    process.exitCode = 1;
  }
}
