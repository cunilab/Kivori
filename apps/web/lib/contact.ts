import { getCloudflareContext } from '@opennextjs/cloudflare';

/** The public support address from the `CONTACT_EMAIL` var, or '' until one is set. Read per request. */
export function contactEmail(): string {
  let value: unknown;
  try {
    value = (getCloudflareContext().env as { CONTACT_EMAIL?: unknown }).CONTACT_EMAIL;
  } catch {
    // outside a Worker (next dev, build): fall back to the process env
  }
  if (typeof value !== 'string' || !value.trim()) value = process.env.CONTACT_EMAIL;
  return typeof value === 'string' ? value.trim() : '';
}
