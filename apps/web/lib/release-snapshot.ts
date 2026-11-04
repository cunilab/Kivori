import { getCloudflareContext } from '@opennextjs/cloudflare';

interface SnapshotDb {
  prepare(sql: string): { first<T>(): Promise<T | null> };
}

/** Server-only: the published snapshot is separate from waitlist records in the existing database. */
export async function readReleaseSnapshot(): Promise<string | null> {
  try {
    const db = (getCloudflareContext().env as { WAITLIST_DB?: SnapshotDb }).WAITLIST_DB;
    if (!db) return null;
    const row = await db
      .prepare('SELECT payload FROM release_snapshots WHERE id = 1')
      .first<{ payload: string }>();
    return row?.payload ?? null;
  } catch {
    return null;
  }
}
