import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { ComparisonTable } from '@/components/comparison-table';
import { ProductCard } from '@/components/product-card';
import { Eyebrow, Section } from '@/components/section';
import { getProducts } from '@/content/products';
import { buildComparison } from '@/lib/comparison';
import { pageMetadata } from '@/lib/metadata';

export const metadata: Metadata = pageMetadata({
  title: 'Products',
  description:
    'The Kivori lineup: desk buddies with a knob, three buttons and a screen, with specs side by side.',
  path: '/products',
});

export default function ProductsPage(): ReactElement {
  const products = getProducts();
  const comparison = buildComparison(products);
  const single = products.length === 1;

  return (
    <>
      <section
        aria-labelledby="products-title"
        className="px-5 pt-20 pb-12 text-center sm:px-8 sm:pt-28"
      >
        <div className="mx-auto max-w-3xl">
          <Eyebrow>Lineup</Eyebrow>
          <h1 id="products-title" className="display-1 mt-3">
            Which Kivori is right for you?
          </h1>
          <p className="lead mx-auto mt-6 max-w-xl">
            Every Kivori is a desk buddy with a physical knob and a screen. Pick the one that fits
            your desk.
          </p>
        </div>
      </section>

      <section aria-label="Lineup" className="px-5 pb-24 sm:px-8 sm:pb-32">
        <ul
          className={
            single
              ? 'mx-auto max-w-[1200px]'
              : 'mx-auto grid max-w-[1200px] gap-6 sm:grid-cols-2 xl:grid-cols-3'
          }
        >
          {products.map((product) => (
            <li key={product.slug} className="flex">
              <ProductCard product={product} layout={single ? 'wide' : 'card'} />
            </li>
          ))}
        </ul>
      </section>

      <Section id="compare" tone="surface" eyebrow="Compare" title="Side by side.">
        <ComparisonTable comparison={comparison} />
      </Section>
    </>
  );
}
