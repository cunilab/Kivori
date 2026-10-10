import { getCloudflareContext } from '@opennextjs/cloudflare';
import type { ReactElement } from 'react';
import { WaitlistForm } from '@/components/waitlist-form';
import { getProducts } from '@/content/products';
import { TURNSTILE_TEST_SITE_KEY } from '@/lib/waitlist';

interface WaitlistSectionProps {
  initialProduct?: string | undefined;
  idPrefix?: string | undefined;
}

/** The site key is read per request from the Worker `vars`, never inlined at build time. */
function siteKey(): string {
  try {
    const env = getCloudflareContext().env as unknown as { TURNSTILE_SITE_KEY?: string };
    return env.TURNSTILE_SITE_KEY || TURNSTILE_TEST_SITE_KEY;
  } catch {
    return TURNSTILE_TEST_SITE_KEY;
  }
}

/** Server wrapper: hands the runtime site key and the product list to the client form. */
export function WaitlistSection({ initialProduct, idPrefix }: WaitlistSectionProps): ReactElement {
  const products = getProducts().map(({ slug, name }) => ({ slug, name }));
  return (
    <WaitlistForm
      siteKey={siteKey()}
      products={products}
      initialProduct={initialProduct}
      idPrefix={idPrefix}
    />
  );
}
