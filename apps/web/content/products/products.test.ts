import { describe, expect, it } from 'vitest';
import { getProduct, getProducts } from './index';

describe('product registry', () => {
  const products = getProducts();

  it('has at least one product with a unique slug each', () => {
    expect(products.length).toBeGreaterThan(0);
    const slugs = products.map((p) => p.slug);
    expect(new Set(slugs).size).toBe(slugs.length);
  });

  it.each(products.map((p) => [p.slug, p] as const))('%s has every required field', (_slug, p) => {
    expect(p.slug).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
    for (const text of [p.name, p.edition, p.tagline, p.summary, p.hero.alt]) {
      expect(text.trim()).not.toBe('');
    }
    expect(['in-development', 'coming-soon', 'available']).toContain(p.status);
    expect(p.features.length).toBeGreaterThan(0);
    expect(p.specs.length).toBeGreaterThan(0);
    for (const group of p.specs) expect(group.rows.length).toBeGreaterThan(0);
    expect(p.compatibility.length).toBeGreaterThan(0);
    expect(p.inTheBox ?? []).not.toContain(expect.stringMatching(/TBC/));
    expect(p.faq.length).toBeGreaterThan(0);
  });

  it('keeps Kivori coming soon with no price or buy link', () => {
    const kivori = getProduct('kivori');
    expect(kivori?.status).toBe('coming-soon');
    expect(kivori?.price).toBeUndefined();
    expect(kivori?.buyUrl).toBeUndefined();
  });
});

describe('getProduct', () => {
  it('finds a product by slug', () => {
    expect(getProduct('kivori')?.name).toBe('Kivori');
  });

  it('returns undefined for an unknown slug, which the page turns into notFound()', () => {
    expect(getProduct('nope')).toBeUndefined();
    expect(getProduct('')).toBeUndefined();
  });
});
