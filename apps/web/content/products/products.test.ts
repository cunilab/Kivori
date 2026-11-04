import { describe, expect, it } from 'vitest';
import { getProduct, getProducts } from './index';
import { catalogProduct } from './catalog';

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
    expect(p.hero.image.src).toMatch(/^\//);
    expect(p.hero.image.alt).toContain(p.model);
    expect(p.inTheBox ?? []).not.toContain(expect.stringMatching(/TBC/));
    expect(p.faq.length).toBeGreaterThan(0);
  });

  it('keeps Kivori coming soon with no price or buy link', () => {
    const kivori = getProduct('kv-p1');
    expect(kivori?.status).toBe('coming-soon');
    expect(kivori?.price).toBeUndefined();
    expect(kivori?.buyUrl).toBeUndefined();
  });
});

describe('getProduct', () => {
  it('finds a product by slug', () => {
    expect(getProduct('kv-p1')?.name).toBe('Kivori KV-P1');
  });

  it('preserves the historic product alias used by waitlist links', () => {
    expect(getProduct('kivori')).toEqual(getProduct('kv-p1'));
  });

  it('uses the shared catalog and rejects unknown products', () => {
    expect(catalogProduct('pip')).toMatchObject({ id: 'pip', wire: 1, model: 'KV-P1' });
    expect(getProduct('kv-p1')).toMatchObject({ id: 'pip', model: 'KV-P1' });
    expect(() => catalogProduct('unknown')).toThrow('Unknown catalog product');
  });

  it('returns undefined for an unknown slug, which the page turns into notFound()', () => {
    expect(getProduct('nope')).toBeUndefined();
    expect(getProduct('')).toBeUndefined();
  });
});
