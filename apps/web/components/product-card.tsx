import Link from 'next/link';
import type { ReactElement } from 'react';
import { DeviceIllustration } from '@/components/device-illustration';
import { StatusPill } from '@/components/status-pill';
import type { Product } from '@/content/products/types';

interface ProductCardProps {
  product: Product;
  /** Heading level for the name, so the page outline stays in order. */
  headingLevel?: 'h2' | 'h3';
}

export function ProductCard({ product, headingLevel = 'h3' }: ProductCardProps): ReactElement {
  const Heading = headingLevel;
  return (
    <article className="relative flex w-full flex-col overflow-hidden rounded-2xl border border-border bg-card transition-colors focus-within:border-primary hover:border-primary">
      <div className="flex items-center justify-center bg-secondary px-8 py-8">
        <DeviceIllustration className="w-full max-w-64" />
      </div>
      <div className="flex flex-1 flex-col gap-3 p-6">
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm text-muted-foreground">{product.edition} edition</p>
          <StatusPill status={product.status} />
        </div>
        <Heading className="font-display text-2xl font-semibold">
          <Link
            href={`/products/${product.slug}`}
            className="after:absolute after:inset-0 focus-visible:outline-none"
          >
            {product.name}
          </Link>
        </Heading>
        <p className="text-muted-foreground">{product.summary}</p>
        <p className="mt-auto pt-2 text-sm font-medium text-primary">See specs and details</p>
      </div>
    </article>
  );
}
