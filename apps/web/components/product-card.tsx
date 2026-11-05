import Link from 'next/link';
import type { ReactElement } from 'react';
import { ProductStage } from '@/components/product-shot';
import type { Product } from '@/content/products/types';
import { notifyHref, PRODUCT_STATUS_LABEL } from '@/lib/product-labels';

interface ProductCardProps {
  product: Product;
  layout?: 'card' | 'wide';
}

export function ProductCard({ product, layout = 'card' }: ProductCardProps): ReactElement {
  return (
    <article
      className={`grid w-full gap-8 rounded-lg bg-surface p-6 sm:p-10 ${layout === 'wide' ? 'lg:grid-cols-2 lg:items-center' : ''}`}
    >
      <div>
        <p className="text-sm font-semibold text-teal">
          {PRODUCT_STATUS_LABEL[product.status]} · {product.edition} edition
        </p>
        <h2 className="display-2 mt-4">
          <Link href={`/products/${product.slug}`} className="hover:underline">
            {product.name}
          </Link>
        </h2>
        <p className="lead mt-4 max-w-lg">{product.summary}</p>
        <div className="mt-6 flex flex-wrap gap-6 text-sm font-semibold">
          <Link
            href={`/products/${product.slug}`}
            className="text-teal underline underline-offset-4"
          >
            Explore {product.model}
          </Link>
          <Link href={notifyHref(product.slug)} className="underline underline-offset-4">
            Get updates
          </Link>
        </div>
      </div>
      <ProductStage image={product.hero.image} large sizes="(min-width: 1024px) 550px, 100vw" />
    </article>
  );
}
