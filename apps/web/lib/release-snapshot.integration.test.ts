import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { getPlatformProxy, type PlatformProxy } from 'wrangler';
import { afterAll, afterEach, beforeAll, beforeEach, expect, it, vi } from 'vitest';
import { syncWebReleases } from '../../../scripts/sync-web-releases.mjs';
import { loadReleases } from './releases';
import { resetMemoryCache } from './github-cache';

interface LocalStatement {
  bind(...values: unknown[]): LocalStatement;
  run(): Promise<unknown>;
  all(): Promise<{ results: Record<string, unknown>[] }>;
  first(): Promise<Record<string, unknown> | null>;
}
interface LocalDb {
  prepare(sql: string): LocalStatement;
}
const directory = mkdtempSync(join(tmpdir(), 'kivori-release-test-'));
let platform: PlatformProxy<{ WAITLIST_DB: LocalDb }>;
let db: LocalDb;
beforeAll(async () => {
  const configPath = join(directory, 'wrangler.json');
  writeFileSync(
    configPath,
    JSON.stringify({
      name: 'kivori-release-local-test',
      compatibility_date: '2026-09-01',
      d1_databases: [
        { binding: 'WAITLIST_DB', database_name: 'local-test', database_id: 'local-test' },
      ],
    }),
  );
  platform = await getPlatformProxy({
    configPath,
    persist: false,
    remoteBindings: false,
    envFiles: [],
  });
  db = platform.env.WAITLIST_DB;
  for (const file of ['0001_waitlist.sql', '0002_release_snapshots.sql']) {
    await db.prepare(readFileSync(new URL(`../migrations/${file}`, import.meta.url), 'utf8')).run();
  }
});
afterAll(async () => {
  try {
    await platform?.dispose();
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
beforeEach(async () => {
  await db.prepare('DELETE FROM waitlist').run();
  await db.prepare('DELETE FROM release_snapshots').run();
  resetMemoryCache();
  vi.stubGlobal(Symbol.for('__cloudflare-context__'), { env: { WAITLIST_DB: db } });
});
afterEach(() => vi.unstubAllGlobals());

it('executes the publisher upsert in D1 without touching waitlist rows or accepting old and failed syncs', async () => {
  await db
    .prepare(
      "INSERT INTO waitlist (email, product, created_at) VALUES ('keep@example.com', 'kivori', '2026-01-01')",
    )
    .run();
  const config = {
    accountId: 'a'.repeat(32),
    databaseId: '00000000-0000-0000-0000-000000000001',
    githubToken: 'gh',
    cloudflareToken: 'cf',
  };
  const record = {
    tag_name: 'v0.3.1',
    draft: false,
    prerelease: false,
    published_at: '2026-11-03T00:00:00Z',
    body: "Keep settings. '); DROP TABLE waitlist; --",
    assets: [],
  };
  let source: unknown = [record],
    failure = false;
  const fetcher = (async (input: RequestInfo | URL, init?: RequestInit) => {
    if (new URL(String(input)).hostname === 'api.github.com')
      return failure ? new Response('', { status: 403 }) : Response.json(source);
    const { sql, params } = JSON.parse(String(init?.body));
    await db
      .prepare(sql)
      .bind(...params)
      .run();
    return Response.json({ success: true, result: [{ success: true }] });
  }) as typeof fetch;
  const sync = async (now: number) => {
    resetMemoryCache();
    return syncWebReleases({ ...config, fetcher, now: () => now });
  };
  const time = Date.now();
  await sync(time);
  const initial = await db
    .prepare('SELECT payload, synced_at FROM release_snapshots WHERE id = 1')
    .first();
  expect(initial?.synced_at).toBe(time);
  await sync(time);
  expect(
    await db.prepare('SELECT payload, synced_at FROM release_snapshots WHERE id = 1').first(),
  ).toEqual(initial);
  source = [{ ...record, tag_name: 'v0.2.0' }];
  await sync(time - 1);
  expect(
    await db.prepare('SELECT payload, synced_at FROM release_snapshots WHERE id = 1').first(),
  ).toEqual(initial);
  failure = true;
  await expect(sync(time + 1)).rejects.toThrow('Unable to fetch a fresh');
  expect(
    await db.prepare('SELECT payload, synced_at FROM release_snapshots WHERE id = 1').first(),
  ).toEqual(initial);
  failure = false;
  source = [];
  await sync(time + 2);
  const updated = await db.prepare('SELECT payload FROM release_snapshots WHERE id = 1').first();
  expect(JSON.parse(updated!.payload as string).releases).toEqual([]);
  expect((await db.prepare('SELECT email FROM waitlist').all()).results).toEqual([
    { email: 'keep@example.com' },
  ]);
});

it('serves fresh and stale D1 snapshots and falls back for expired or invalid data', async () => {
  const now = Date.now();
  const release = {
    version: '0.3.1',
    tag: 'v0.3.1',
    date: null,
    prerelease: false,
    notes: '',
    firmware: [],
    installer: null,
  };
  const store = async (syncedAt: number, releases: unknown[]) => {
    await db
      .prepare(
        'INSERT OR REPLACE INTO release_snapshots (id, payload, synced_at) VALUES (1, ?1, ?2)',
      )
      .bind(JSON.stringify({ schema_version: 1, synced_at: syncedAt, releases }), syncedAt)
      .run();
  };
  const failing = vi.fn(async () => new Response('', { status: 403 })) as unknown as typeof fetch;
  await store(now, [release]);
  expect(await loadReleases(failing, 'https://api.test/cold')).toMatchObject({
    status: 'ok',
    stale: false,
    releases: [{ version: '0.3.1' }],
  });
  expect(failing).not.toHaveBeenCalled();
  await store(now - 601_000, [release]);
  expect(await loadReleases(failing, 'https://api.test/cold')).toMatchObject({
    status: 'ok',
    stale: true,
  });
  expect(failing).not.toHaveBeenCalled();
  await store(now - 86_400_001, [release]);
  expect(await loadReleases(failing, 'https://api.test/cold')).toEqual({ status: 'error' });
  await store(now, [
    { ...release, installer: { size: 10, url: 'https://evil.test/file-setup.exe' } },
  ]);
  expect(await loadReleases(failing, 'https://api.test/cold')).toEqual({ status: 'error' });
  expect(failing).toHaveBeenCalledTimes(2);
});
