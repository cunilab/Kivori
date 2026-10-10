import { getCloudflareContext } from '@opennextjs/cloudflare';
import { NextResponse, type NextRequest } from 'next/server';
import { getProduct } from '@/content/products';
import {
  handleWaitlist,
  resolveTurnstileSecret,
  verifyTurnstileToken,
  type WaitlistDb,
} from '@/lib/waitlist';

export const dynamic = 'force-dynamic';

/** The bindings this route uses; any of them can be absent in plain `next dev`. */
interface WaitlistEnv {
  WAITLIST_DB?: WaitlistDb;
  WAITLIST_RATE_LIMITER?: { limit(options: { key: string }): Promise<{ success: boolean }> };
  SIGNUPS?: {
    writeDataPoint(point: { blobs: string[]; doubles: number[]; indexes: string[] }): void;
  };
  TURNSTILE_SITE_KEY?: string;
  TURNSTILE_SECRET_KEY?: string;
}

function readEnv(): WaitlistEnv {
  try {
    return getCloudflareContext().env as unknown as WaitlistEnv;
  } catch {
    return {};
  }
}

async function readBody(request: NextRequest): Promise<Record<string, unknown> | null> {
  try {
    if ((request.headers.get('content-type') ?? '').includes('application/json')) {
      const data: unknown = await request.json();
      return data && typeof data === 'object' && !Array.isArray(data)
        ? (data as Record<string, unknown>)
        : null;
    }
    const form = await request.formData();
    return Object.fromEntries([...form.entries()].filter(([, value]) => typeof value === 'string'));
  } catch {
    return null;
  }
}

export async function POST(request: NextRequest): Promise<NextResponse> {
  const input = await readBody(request);
  if (!input) {
    return NextResponse.json({ ok: false, message: 'Send JSON or form data.' }, { status: 400 });
  }

  const env = readEnv();
  // The IP is only the rate-limit key and Turnstile's optional hint; it is never stored.
  const ip = request.headers.get('cf-connecting-ip') ?? undefined;
  const secret = resolveTurnstileSecret(env.TURNSTILE_SITE_KEY, env.TURNSTILE_SECRET_KEY);

  const result = await handleWaitlist(
    input,
    {
      db: env.WAITLIST_DB,
      verifyTurnstile: (token) => verifyTurnstileToken(token, secret, ip),
      // Without the binding (dev) or an IP, do not block.
      rateLimit: async () => {
        if (!env.WAITLIST_RATE_LIMITER || !ip) return true;
        return (await env.WAITLIST_RATE_LIMITER.limit({ key: ip })).success;
      },
      now: () => new Date(),
      record: (product, utmSource) => {
        try {
          env.SIGNUPS?.writeDataPoint({
            blobs: [product, utmSource.slice(0, 64)],
            doubles: [1],
            indexes: [product],
          });
        } catch {
          // counting must never break a sign-up
        }
      },
    },
    (slug) => getProduct(slug) !== undefined,
  );
  return NextResponse.json(result.body, { status: result.status });
}
