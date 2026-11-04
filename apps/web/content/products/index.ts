import { kivori } from './kivori';
import type { Product } from './types';

// Adding an edition = adding a file and listing it here. The lineup, comparison table, JSON-LD and
// OG images all derive from this list.
const PRODUCTS: readonly Product[] = [kivori];

export function getProducts(): Product[] {
  if (
    new Set(PRODUCTS.map((product) => product.id)).size !== PRODUCTS.length ||
    new Set(PRODUCTS.map((product) => product.slug)).size !== PRODUCTS.length
  ) {
    throw new Error('Duplicate product identity or slug');
  }
  return [...PRODUCTS];
}

export function getProduct(slug: string): Product | undefined {
  // Keep historic waitlist attribution and URLs valid while using the model slug going forward.
  return getProducts().find((product) => product.slug === (slug === 'kivori' ? kivori.slug : slug));
}

export type { Product } from './types';
