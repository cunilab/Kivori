import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { ComparisonTable } from '@/components/comparison-table';
import { ProductCard } from '@/components/product-card';
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

  return (
    <div className="px-5 py-14 sm:py-20">
      <div className="mx-auto max-w-5xl">
        <h1 className="font-display text-4xl font-semibold sm:text-5xl">Products</h1>
        <p className="mt-4 max-w-2xl text-lg text-muted-foreground">
          Every Kivori is a desk buddy with a physical knob and a screen. Pick the one that fits
          your desk.
        </p>

        <ul className="mt-10 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {products.map((product) => (
            <li key={product.slug} className="flex">
              <ProductCard product={product} headingLevel="h2" />
            </li>
          ))}
        </ul>

        <h2 className="mt-16 font-display text-2xl font-semibold sm:text-3xl">Compare</h2>
        <div className="mt-6">
          <ComparisonTable comparison={comparison} />
        </div>
      </div>
    </div>
  );
}
