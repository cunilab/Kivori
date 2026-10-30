import { getCloudflareContext } from '@opennextjs/cloudflare';
import type { ReactElement } from 'react';
import { WaitlistForm } from '@/components/waitlist-form';
import { getProducts } from '@/content/products';
import { readTurnstileValue, type TurnstileValue } from '@/lib/turnstile-env';
import { TURNSTILE_TEST_SITE_KEY } from '@/lib/waitlist';

interface WaitlistSectionProps {
  initialProduct?: string | undefined;
  idPrefix?: string | undefined;
  variant?: 'card' | 'inline' | undefined;
}

/** Rendering outside a Worker uses test keys; a failed Worker binding never falls back to them. */
async function siteKey(): Promise<string> {
  let env: { TURNSTILE_SITE_KEY?: TurnstileValue };
  try {
    env = getCloudflareContext().env as unknown as typeof env;
  } catch {
    return TURNSTILE_TEST_SITE_KEY;
  }
  const value = await readTurnstileValue(env.TURNSTILE_SITE_KEY);
  if (!value) throw new Error('Missing site key');
  return value;
}

/** Server wrapper: hands the runtime site key and the product list to the client form. */
export async function WaitlistSection({
  initialProduct,
  idPrefix,
  variant,
}: WaitlistSectionProps): Promise<ReactElement> {
  let key: string;
  try {
    key = await siteKey();
  } catch {
    return <p role="status">Signup is temporarily unavailable. Please try again later.</p>;
  }
  const products = getProducts().map(({ slug, name }) => ({ slug, name }));
  return (
    <WaitlistForm
      siteKey={key}
      products={products}
      initialProduct={initialProduct}
      idPrefix={idPrefix}
      variant={variant}
    />
  );
}
