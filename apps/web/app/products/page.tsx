import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { ComparisonTable } from '@/components/comparison-table';
import { ProductCard } from '@/components/product-card';
import { Section } from '@/components/section';
import { getProducts } from '@/content/products';
import { buildComparison } from '@/lib/comparison';
import { pageMetadata } from '@/lib/metadata';

export const metadata: Metadata = pageMetadata({
  title: 'Products',
  description:
    'Explore Kivori devices, including the KV-P1, and the shared desktop app that powers them.',
  path: '/products',
});

export default function ProductsPage(): ReactElement {
  const products = getProducts();
  const comparison = buildComparison(products);
  const single = products.length === 1;

  return (
    <>
      <section aria-labelledby="products-title" className="px-5 pt-14 pb-8 sm:px-8 sm:pt-20">
        <div className="mx-auto max-w-[1180px]">
          <h1 id="products-title" className="display-1">
            Products
          </h1>
          <p className="lead mt-4 max-w-xl">
            Physical controls and a desktop app designed to work together.
          </p>
        </div>
      </section>

      <section aria-label="Lineup" className="px-5 pb-16 sm:px-8 sm:pb-24">
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

      {products.length > 1 ? (
        <Section id="compare" eyebrow="Compare" title="Side by side.">
          <ComparisonTable comparison={comparison} />
        </Section>
      ) : null}
    </>
  );
}
