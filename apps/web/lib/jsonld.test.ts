import { describe, expect, it } from 'vitest';
import { kivori } from '@/content/products/kivori';
import { faqJsonLd, organizationJsonLd, productJsonLd, serializeJsonLd } from './jsonld';

const origin = new URL('https://kivori.example');

describe('organizationJsonLd', () => {
  it('names Cunilab and links GitHub', () => {
    const org = organizationJsonLd(origin);
    expect(org['@type']).toBe('Organization');
    expect(org.name).toBe('Cunilab');
    expect(org.url).toBe('https://kivori.example/');
    expect(org.sameAs).toEqual(['https://github.com/cunilab/Kivori']);
  });
});

describe('productJsonLd', () => {
  it('describes the product with its brand and share image', () => {
    const data = productJsonLd(kivori, origin);
    expect(data['@type']).toBe('Product');
    expect(data.name).toBe('Kivori');
    expect(data.brand).toEqual({ '@type': 'Brand', name: 'Kivori' });
    expect(data.image).toBe('https://kivori.example/generated/og/kivori.png');
    expect(data.url).toBe('https://kivori.example/products/kivori');
  });

  it('has no offers without a price', () => {
    expect(productJsonLd(kivori, origin)).not.toHaveProperty('offers');
    expect(productJsonLd({ ...kivori, buyUrl: 'https://buy.example' }, origin)).not.toHaveProperty(
      'offers',
    );
  });

  it('adds an offer once there is a price', () => {
    const data = productJsonLd({ ...kivori, price: '$99' }, origin);
    expect(data.offers).toMatchObject({ '@type': 'Offer', price: '$99' });
  });
});

describe('faqJsonLd', () => {
  it('maps each item to a Question with an accepted Answer', () => {
    const data = faqJsonLd([{ question: 'Q?', answer: 'A.' }]);
    expect(data['@type']).toBe('FAQPage');
    expect(data.mainEntity).toEqual([
      { '@type': 'Question', name: 'Q?', acceptedAnswer: { '@type': 'Answer', text: 'A.' } },
    ]);
  });
});

describe('serializeJsonLd', () => {
  it('escapes < so the script tag cannot be closed from content', () => {
    const out = serializeJsonLd({ text: '</script><b>' });
    expect(out).not.toContain('<');
    expect(JSON.parse(out).text).toBe('</script><b>');
  });
});
