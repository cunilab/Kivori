import { kivori } from './kivori';
import type { Product } from './types';

// Adding an edition = adding a file and listing it here. The lineup, comparison table, JSON-LD and
// OG images all derive from this list.
const PRODUCTS: readonly Product[] = [kivori];

export function getProducts(): Product[] {
  return [...PRODUCTS];
}

export function getProduct(slug: string): Product | undefined {
  return PRODUCTS.find((product) => product.slug === slug);
}

export type { Product } from './types';
