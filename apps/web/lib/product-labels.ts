import type { CompatibilityStatus, ProductStatus } from '@/content/products/types';

export const PRODUCT_STATUS_LABEL: Record<ProductStatus, string> = {
  'in-development': 'In development',
  'coming-soon': 'Coming soon',
  available: 'Available',
};

export const COMPATIBILITY_LABEL: Record<CompatibilityStatus, string> = {
  'supported-beta': 'Supported in the beta',
  supported: 'Supported',
  later: 'Later',
  'not-planned': 'Not planned',
};

/** The notify link carries the product so the waitlist form (W4) can preselect it. */
export function notifyHref(slug: string): string {
  return `/?product=${encodeURIComponent(slug)}#waitlist`;
}
