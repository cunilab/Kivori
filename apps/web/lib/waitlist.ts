export const GENERAL_PRODUCT = 'general';

/** Cloudflare's official always-pass Turnstile test keys. Never use them for a real widget. */
export const TURNSTILE_TEST_SITE_KEY = '1x00000000000000000000AA';
export const TURNSTILE_TEST_SECRET_KEY = '1x0000000000000000000000000000000AA';
export const TURNSTILE_VERIFY_URL = 'https://challenges.cloudflare.com/turnstile/v0/siteverify';

export const LIMITS = {
  email: 254,
  product: 64,
  useCase: 280,
  utm: 100,
  referrer: 500,
  token: 2048,
  honeypot: 200,
} as const;

/** Name of the honeypot field: a plausible text input that people never see or fill in. */
export const HONEYPOT_FIELD = 'website';

const EMAIL_PATTERN = /^[^\s@]+@[^\s@.]+(\.[^\s@.]+)+$/;

export interface SignupInput {
  email: string;
  product: string;
  useCase: string | null;
  utmSource: string | null;
  utmMedium: string | null;
  utmCampaign: string | null;
  referrer: string | null;
  turnstileToken: string;
}

export type ValidationResult = { ok: true; value: SignupInput } | { ok: false; message: string };

export function normalizeEmail(email: string): string {
  return email.trim().toLowerCase();
}

function text(input: Record<string, unknown>, key: string): string {
  const value = input[key];
  return typeof value === 'string' ? value : '';
}

/**
 * Which secret to verify Turnstile tokens with. A real site key without a configured secret yields
 * `undefined` (fail closed); the test secret is only a fallback for the test site key.
 */
export function resolveTurnstileSecret(
  siteKey: string | undefined,
  secret: string | undefined,
): string | undefined {
  if (secret) return secret;
  if (!siteKey || siteKey === TURNSTILE_TEST_SITE_KEY) return TURNSTILE_TEST_SECRET_KEY;
  return undefined;
}

export function isHoneypotTripped(input: Record<string, unknown>): boolean {
  return text(input, HONEYPOT_FIELD).trim() !== '';
}

/** Validates a parsed JSON/form body: lengths, email format, product and use case. */
export function validateSignup(
  input: Record<string, unknown>,
  isKnownProduct: (slug: string) => boolean,
): ValidationResult {
  const email = normalizeEmail(text(input, 'email'));
  const product = text(input, 'product').trim() || GENERAL_PRODUCT;
  const useCase = text(input, 'use_case').trim();
  const utmSource = text(input, 'utm_source').trim();
  const utmMedium = text(input, 'utm_medium').trim();
  const utmCampaign = text(input, 'utm_campaign').trim();
  const referrer = text(input, 'referrer').trim();
  const token = text(input, 'turnstileToken') || text(input, 'cf-turnstile-response');

  const tooLong =
    email.length > LIMITS.email ||
    product.length > LIMITS.product ||
    utmSource.length > LIMITS.utm ||
    utmMedium.length > LIMITS.utm ||
    utmCampaign.length > LIMITS.utm ||
    referrer.length > LIMITS.referrer ||
    token.length > LIMITS.token ||
    text(input, HONEYPOT_FIELD).length > LIMITS.honeypot;
  if (tooLong) return { ok: false, message: 'One of the fields is too long.' };
  if (!email || !EMAIL_PATTERN.test(email)) {
    return { ok: false, message: 'Enter a valid email address.' };
  }
  if (product !== GENERAL_PRODUCT && !isKnownProduct(product)) {
    return { ok: false, message: 'Choose a product from the list.' };
  }
  if (useCase.length > LIMITS.useCase) {
    return { ok: false, message: `Keep the use case to ${LIMITS.useCase} characters or fewer.` };
  }
  return {
    ok: true,
    value: {
      email,
      product,
      useCase: useCase || null,
      utmSource: utmSource || null,
      utmMedium: utmMedium || null,
      utmCampaign: utmCampaign || null,
      referrer: referrer || null,
      turnstileToken: token,
    },
  };
}

/** The slice of D1 the handler uses. */
export interface WaitlistDb {
  prepare(sql: string): { bind(...values: unknown[]): { run(): Promise<unknown> } };
}

export interface WaitlistDeps {
  /** Absent when the D1 binding is missing (plain `next dev`). */
  db: WaitlistDb | undefined;
  verifyTurnstile(token: string): Promise<boolean>;
  /** True when the caller is within the limit. The client IP is closed over by the caller. */
  rateLimit(): Promise<boolean>;
  now(): Date;
  /** Counts a new or repeat sign-up without the email. */
  record?(product: string, utmSource: string): void;
}

export interface WaitlistResponse {
  status: number;
  body: { ok: boolean; message: string };
}

const SUCCESS: WaitlistResponse = {
  status: 200,
  body: { ok: true, message: "You're on the list." },
};

function failure(status: number, message: string): WaitlistResponse {
  return { status, body: { ok: false, message } };
}

export const INSERT_SQL =
  'INSERT INTO waitlist (email, product, use_case, utm_source, utm_medium, utm_campaign, referrer, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(email, product) DO NOTHING';

/**
 * The whole sign-up flow with injected dependencies. New and repeat sign-ups return the identical
 * success response, so the endpoint cannot be used to find out who is on the list. A filled honeypot
 * also gets that response, but nothing is stored.
 */
export async function handleWaitlist(
  input: Record<string, unknown>,
  deps: WaitlistDeps,
  isKnownProduct: (slug: string) => boolean,
): Promise<WaitlistResponse> {
  if (isHoneypotTripped(input)) return SUCCESS;

  const result = validateSignup(input, isKnownProduct);
  if (!result.ok) return failure(400, result.message);
  const signup = result.value;

  if (!deps.db) {
    return failure(503, 'The waitlist database is not available. Try again later.');
  }
  if (!signup.turnstileToken || !(await deps.verifyTurnstile(signup.turnstileToken))) {
    return failure(403, 'The human check failed. Reload the page and try again.');
  }
  if (!(await deps.rateLimit())) {
    return failure(429, 'Too many sign-ups from your network. Wait a minute and try again.');
  }

  try {
    await deps.db
      .prepare(INSERT_SQL)
      .bind(
        signup.email,
        signup.product,
        signup.useCase,
        signup.utmSource,
        signup.utmMedium,
        signup.utmCampaign,
        signup.referrer,
        deps.now().toISOString(),
      )
      .run();
  } catch {
    return failure(503, 'We could not save your sign-up. Try again later.');
  }
  deps.record?.(signup.product, signup.utmSource ?? '');
  return SUCCESS;
}

/** Calls Turnstile `siteverify`. Any network or parse failure counts as a failed check. */
export async function verifyTurnstileToken(
  token: string,
  secret: string | undefined,
  remoteIp: string | undefined,
  fetcher: typeof fetch = fetch,
): Promise<boolean> {
  if (!secret) return false;
  try {
    const form = new FormData();
    form.set('secret', secret);
    form.set('response', token);
    if (remoteIp) form.set('remoteip', remoteIp);
    const response = await fetcher(TURNSTILE_VERIFY_URL, { method: 'POST', body: form });
    const data = (await response.json()) as { success?: boolean };
    return data.success === true;
  } catch {
    return false;
  }
}
