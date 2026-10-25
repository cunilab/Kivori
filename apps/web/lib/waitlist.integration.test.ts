import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { getPlatformProxy, type PlatformProxy } from 'wrangler';
import { NextRequest } from 'next/server';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { POST } from '@/app/api/waitlist/route';
import PrivacyPage from '@/app/privacy/page';
import { TURNSTILE_TEST_SECRET_KEY, TURNSTILE_TEST_SITE_KEY } from './waitlist';

const context = vi.hoisted(() => ({ env: {} as Record<string, unknown>, unavailable: false }));
vi.mock('@opennextjs/cloudflare', () => ({
  getCloudflareContext: () => {
    if (context.unavailable) throw new Error('Outside a Worker');
    return { env: context.env };
  },
}));

describe('local waitlist rehearsal (local D1, mocked Turnstile and Worker context)', () => {
  const directory = mkdtempSync(join(tmpdir(), 'kivori-waitlist-test-'));
  let platform: PlatformProxy<{ WAITLIST_DB: D1Database }> | undefined;
  let db: D1Database;
  const verify = vi.fn<typeof fetch>(async () => Response.json({ success: true }));
  const limit = vi.fn(async () => ({ success: true }));
  const record = vi.fn();

  beforeAll(async () => {
    const configPath = join(directory, 'wrangler.json');
    writeFileSync(
      configPath,
      JSON.stringify({
        name: 'kivori-waitlist-local-test',
        compatibility_date: '2026-09-01',
        d1_databases: [
          { binding: 'WAITLIST_DB', database_name: 'local-test', database_id: 'local-test' },
        ],
      }),
    );
    platform = await getPlatformProxy<{ WAITLIST_DB: D1Database }>({
      configPath,
      persist: false,
      remoteBindings: false,
      envFiles: [],
    });
    db = platform.env.WAITLIST_DB;
    await db
      .prepare(readFileSync(new URL('../migrations/0001_waitlist.sql', import.meta.url), 'utf8'))
      .run();
  });

  afterAll(async () => {
    try {
      await platform?.dispose();
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  beforeEach(async () => {
    vi.clearAllMocks();
    await db.prepare('DELETE FROM waitlist').run();
    context.unavailable = false;
    context.env = {
      WAITLIST_DB: db,
      WAITLIST_RATE_LIMITER: { limit },
      SIGNUPS: { writeDataPoint: record },
      TURNSTILE_SITE_KEY: TURNSTILE_TEST_SITE_KEY,
      CONTACT_EMAIL: 'privacy@example.com',
    };
    vi.stubEnv('CONTACT_EMAIL', '');
    vi.stubGlobal('fetch', verify);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  function signup(overrides: Record<string, unknown> = {}): Promise<Response> {
    return POST(
      new NextRequest('http://localhost/api/waitlist', {
        method: 'POST',
        headers: { 'content-type': 'application/json', 'cf-connecting-ip': '192.0.2.1' },
        body: JSON.stringify({
          email: ' Rehearsal@Example.COM ',
          product: 'kivori',
          turnstileToken: 'local-mock-token',
          utm_campaign: 'x'.repeat(101),
          ...overrides,
        }),
      }),
    );
  }

  it('stores and deduplicates signups, then deletes only the requested address across products', async () => {
    const first = await signup();
    const duplicate = await signup({ email: 'rehearsal@example.com' });
    expect(first.status).toBe(200);
    expect(duplicate.status).toBe(200);
    expect(await duplicate.json()).toEqual(await first.json());
    expect(
      (await db.prepare('SELECT email, product, utm_campaign FROM waitlist').all()).results,
    ).toEqual([{ email: 'rehearsal@example.com', product: 'kivori', utm_campaign: null }]);
    expect((await signup({ product: 'general' })).status).toBe(200);
    expect((await signup({ email: 'keep@example.com' })).status).toBe(200);

    const address = 'rehearsal@example.com';
    const count = db.prepare('SELECT COUNT(*) AS count FROM waitlist WHERE email = ?1');
    expect(await count.bind(address).first()).toEqual({ count: 2 });
    expect(
      (await db.prepare('DELETE FROM waitlist WHERE email = ?1').bind(address).run()).meta.changes,
    ).toBe(2);
    expect(await count.bind(address).first()).toEqual({ count: 0 });
    expect((await db.prepare('SELECT email FROM waitlist').all()).results).toEqual([
      { email: 'keep@example.com' },
    ]);

    const form = verify.mock.calls[0]?.[1]?.body as FormData;
    expect(form.get('secret')).toBe(TURNSTILE_TEST_SECRET_KEY);
    expect(form.get('response')).toBe('local-mock-token');
    expect(limit).toHaveBeenCalledWith({ key: '192.0.2.1' });
    expect(record).toHaveBeenCalledTimes(4);
    for (const [point] of record.mock.calls) {
      expect(point).toMatchObject({ doubles: [1], indexes: [expect.any(String)] });
      expect(JSON.stringify(point)).not.toMatch(/@|192\.0\.2\.1|local-mock-token/);
    }
    const columns = (await db.prepare('PRAGMA table_info(waitlist)').all()).results.map(
      (column) => column.name,
    );
    expect(columns).not.toContain('ip');
    expect(columns).not.toContain('turnstileToken');
  });

  it.each([
    'missing database',
    'missing real secret',
    'failed human check',
    'rate limited',
    'honeypot',
  ])('%s does not store a row', async (scenario) => {
    if (scenario === 'missing database') delete context.env.WAITLIST_DB;
    if (scenario === 'missing real secret')
      context.env.TURNSTILE_SITE_KEY = 'real-widget-placeholder';
    if (scenario === 'failed human check')
      verify.mockResolvedValueOnce(Response.json({ success: false }));
    if (scenario === 'rate limited') limit.mockResolvedValueOnce({ success: false });
    const response = await signup(
      scenario === 'honeypot' ? { website: 'https://example.com' } : {},
    );
    const expected = {
      'missing database': 503,
      'missing real secret': 403,
      'failed human check': 403,
      'rate limited': 429,
      honeypot: 200,
    };
    expect(response.status).toBe(expected[scenario as keyof typeof expected]);
    expect(await db.prepare('SELECT COUNT(*) AS count FROM waitlist').first()).toEqual({
      count: 0,
    });
    expect(record).not.toHaveBeenCalled();
    if (['missing database', 'missing real secret', 'honeypot'].includes(scenario)) {
      expect(verify).not.toHaveBeenCalled();
    }
  });

  it('renders the Worker contact, local fallback and unconfigured privacy message', () => {
    expect(renderToStaticMarkup(PrivacyPage())).toContain('href="mailto:privacy@example.com"');
    context.unavailable = true;
    vi.stubEnv('CONTACT_EMAIL', ' local-privacy@example.com ');
    expect(renderToStaticMarkup(PrivacyPage())).toContain(
      'href="mailto:local-privacy@example.com"',
    );
    vi.stubEnv('CONTACT_EMAIL', '');
    const unconfigured = renderToStaticMarkup(PrivacyPage());
    expect(unconfigured).toContain('Contact details will be published here before launch.');
    expect(unconfigured).not.toContain('mailto:');
  });
});
