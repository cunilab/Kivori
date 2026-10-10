import { describe, expect, it, vi } from 'vitest';
import {
  handleWaitlist,
  resolveTurnstileSecret,
  TURNSTILE_TEST_SECRET_KEY,
  TURNSTILE_TEST_SITE_KEY,
  validateSignup,
  verifyTurnstileToken,
  type WaitlistDb,
  type WaitlistDeps,
} from './waitlist';

const known = (slug: string): boolean => slug === 'kivori';
const valid = { email: ' Ada@Example.COM ', product: 'kivori', turnstileToken: 'tok' };

/** An in-memory D1 stand-in that honours UNIQUE(email, product). */
function fakeDb(): { db: WaitlistDb; rows: unknown[][] } {
  const rows: unknown[][] = [];
  const db: WaitlistDb = {
    prepare: () => ({
      bind: (...values: unknown[]) => ({
        run: async (): Promise<void> => {
          if (!rows.some((row) => row[0] === values[0] && row[1] === values[1])) rows.push(values);
        },
      }),
    }),
  };
  return { db, rows };
}

function deps(overrides: Partial<WaitlistDeps> = {}): WaitlistDeps {
  return {
    db: fakeDb().db,
    verifyTurnstile: async () => true,
    rateLimit: async () => true,
    now: () => new Date('2026-10-10T00:00:00.000Z'),
    ...overrides,
  };
}

describe('validateSignup', () => {
  it('normalises the email and defaults the product to general', () => {
    const result = validateSignup({ email: ' Ada@Example.COM ' }, known);
    expect(result).toMatchObject({
      ok: true,
      value: { email: 'ada@example.com', product: 'general' },
    });
  });

  it.each(['', 'nope', 'a@b', 'a b@c.de', 'a@@c.de', 'a@c..de'])('rejects email %j', (email) => {
    expect(validateSignup({ email }, known).ok).toBe(false);
  });

  it('rejects unknown products but accepts general', () => {
    expect(validateSignup({ email: 'a@b.co', product: 'nope' }, known).ok).toBe(false);
    expect(validateSignup({ email: 'a@b.co', product: 'general' }, known).ok).toBe(true);
  });

  it('enforces length limits', () => {
    expect(validateSignup({ email: `${'a'.repeat(250)}@b.co` }, known).ok).toBe(false);
    expect(validateSignup({ email: 'a@b.co', use_case: 'x'.repeat(281) }, known).ok).toBe(false);
    expect(validateSignup({ email: 'a@b.co', use_case: 'x'.repeat(280) }, known).ok).toBe(true);
    expect(validateSignup({ email: 'a@b.co', utm_source: 'x'.repeat(101) }, known).ok).toBe(false);
    expect(validateSignup({ email: 'a@b.co', referrer: 'x'.repeat(501) }, known).ok).toBe(false);
  });
});

describe('handleWaitlist', () => {
  it('stores a new sign-up with a normalised email', async () => {
    const { db, rows } = fakeDb();
    const result = await handleWaitlist(valid, deps({ db }), known);
    expect(result.status).toBe(200);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.[0]).toBe('ada@example.com');
    expect(rows[0]?.[7]).toBe('2026-10-10T00:00:00.000Z');
  });

  it('returns the identical response for a duplicate, without a second row', async () => {
    const { db, rows } = fakeDb();
    const first = await handleWaitlist(valid, deps({ db }), known);
    const second = await handleWaitlist(
      { ...valid, email: 'ada@example.com' },
      deps({ db }),
      known,
    );
    expect(second).toEqual(first);
    expect(rows).toHaveLength(1);
  });

  it('answers a filled honeypot like a success but stores nothing and skips every check', async () => {
    const { db, rows } = fakeDb();
    const verifyTurnstile = vi.fn(async () => true);
    const result = await handleWaitlist(
      { ...valid, website: 'http://spam' },
      deps({ db, verifyTurnstile }),
      known,
    );
    expect(result.status).toBe(200);
    expect(rows).toHaveLength(0);
    expect(verifyTurnstile).not.toHaveBeenCalled();
  });

  it('returns 400 for invalid input before any Turnstile call', async () => {
    const verifyTurnstile = vi.fn(async () => true);
    const result = await handleWaitlist(
      { ...valid, email: 'bad' },
      deps({ verifyTurnstile }),
      known,
    );
    expect(result.status).toBe(400);
    expect(verifyTurnstile).not.toHaveBeenCalled();
  });

  it('returns 403 when Turnstile fails or the token is missing', async () => {
    const failing = await handleWaitlist(
      valid,
      deps({ verifyTurnstile: async () => false }),
      known,
    );
    expect(failing.status).toBe(403);
    const missing = await handleWaitlist({ ...valid, turnstileToken: '' }, deps(), known);
    expect(missing.status).toBe(403);
  });

  it('returns 429 when rate limited and stores nothing', async () => {
    const { db, rows } = fakeDb();
    const result = await handleWaitlist(valid, deps({ db, rateLimit: async () => false }), known);
    expect(result.status).toBe(429);
    expect(rows).toHaveLength(0);
  });

  it('returns 503 with a clear message when the database is missing', async () => {
    const result = await handleWaitlist(valid, deps({ db: undefined }), known);
    expect(result.status).toBe(503);
    expect(result.body.message).toMatch(/database/i);
  });

  it('returns 503 when the insert throws', async () => {
    const db: WaitlistDb = {
      prepare: () => ({
        bind: () => ({
          run: async (): Promise<void> => {
            throw new Error('d1');
          },
        }),
      }),
    };
    expect((await handleWaitlist(valid, deps({ db }), known)).status).toBe(503);
  });

  it('records an analytics point without the email', async () => {
    const record = vi.fn();
    await handleWaitlist({ ...valid, utm_source: 'reddit' }, deps({ record }), known);
    expect(record).toHaveBeenCalledWith('kivori', 'reddit');
  });
});

describe('resolveTurnstileSecret', () => {
  it('falls back to the test secret only for the test site key', () => {
    expect(resolveTurnstileSecret(TURNSTILE_TEST_SITE_KEY, undefined)).toBe(
      TURNSTILE_TEST_SECRET_KEY,
    );
    expect(resolveTurnstileSecret(undefined, undefined)).toBe(TURNSTILE_TEST_SECRET_KEY);
  });

  it('fails closed for a real site key without a secret', () => {
    expect(resolveTurnstileSecret('0x4AAAAAAAreal', undefined)).toBeUndefined();
    expect(resolveTurnstileSecret('0x4AAAAAAAreal', '')).toBeUndefined();
  });

  it('prefers a configured secret', () => {
    expect(resolveTurnstileSecret(TURNSTILE_TEST_SITE_KEY, 'real')).toBe('real');
  });
});

describe('verifyTurnstileToken', () => {
  it('posts secret, response and remoteip to siteverify', async () => {
    const fetcher = vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
      const body = init?.body as FormData;
      expect([body.get('secret'), body.get('response'), body.get('remoteip')]).toEqual([
        's',
        't',
        '1.2.3.4',
      ]);
      return Response.json({ success: true });
    });
    expect(await verifyTurnstileToken('t', 's', '1.2.3.4', fetcher as typeof fetch)).toBe(true);
  });

  it('fails closed without a secret, on failure and on network errors', async () => {
    expect(await verifyTurnstileToken('t', undefined, undefined)).toBe(false);
    const no = (async () => Response.json({ success: false })) as typeof fetch;
    expect(await verifyTurnstileToken('t', 's', undefined, no)).toBe(false);
    const boom = (async () => {
      throw new Error('net');
    }) as typeof fetch;
    expect(await verifyTurnstileToken('t', 's', undefined, boom)).toBe(false);
  });
});
