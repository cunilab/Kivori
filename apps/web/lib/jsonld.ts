import type { FaqItem, Product } from '@/content/products/types';
import { absoluteUrl, GITHUB_URL, ogImagePath, SITE_NAME } from '@/lib/site';

export type JsonLd = Record<string, unknown>;

export function organizationJsonLd(origin?: URL): JsonLd {
  return {
    '@context': 'https://schema.org',
    '@type': 'Organization',
    name: 'Cunilab',
    url: absoluteUrl('/', origin),
    sameAs: [GITHUB_URL],
  };
}

/** `offers` is added only when the product has a price; until then there is nothing to offer. */
export function productJsonLd(product: Product, origin?: URL): JsonLd {
  const data: JsonLd = {
    '@context': 'https://schema.org',
    '@type': 'Product',
    name: product.name,
    description: product.summary,
    brand: { '@type': 'Brand', name: SITE_NAME },
    image: absoluteUrl(ogImagePath(product.slug), origin),
    url: absoluteUrl(`/products/${product.slug}`, origin),
  };
  if (product.price) {
    data.offers = {
      '@type': 'Offer',
      price: product.price,
      url: product.buyUrl ?? absoluteUrl(`/products/${product.slug}`, origin),
    };
  }
  return data;
}

export function faqJsonLd(faq: readonly FaqItem[]): JsonLd {
  return {
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: faq.map(({ question, answer }) => ({
      '@type': 'Question',
      name: question,
      acceptedAnswer: { '@type': 'Answer', text: answer },
    })),
  };
}

/** Serialises for an inline `<script>`; `<` is escaped so content can never close the tag. */
export function serializeJsonLd(data: JsonLd): string {
  return JSON.stringify(data).replace(/</g, '\\u003c');
}
