import Link from 'next/link';
import type { ReactElement } from 'react';
import { Badge } from '@kivori/ui/components/badge';
import { Card } from '@kivori/ui/components/card';
import { cn } from '@kivori/ui/lib/utils';
import { ProductStage } from '@/components/product-shot';
import type { Product } from '@/content/products/types';
import { renders } from '@/lib/media';
import { notifyHref, PRODUCT_STATUS_LABEL } from '@/lib/product-labels';

interface ProductCardProps {
  product: Product;
  /** `wide` lays image and text side by side (a lineup of one); `card` stacks them. */
  layout?: 'card' | 'wide';
}

/** Lineup card: the render, the edition, a one-line pitch and the two next steps. */
export function ProductCard({ product, layout = 'card' }: ProductCardProps): ReactElement {
  const wide = layout === 'wide';
  return (
    <Card
      className={cn(
        'relative h-full gap-0 rounded-3xl p-0 shadow-none ring-1 ring-border',
        wide && 'lg:flex-row lg:items-stretch',
      )}
    >
      <div className={cn('flex items-center justify-center p-6 sm:p-10', wide && 'lg:w-3/5')}>
        <ProductStage
          image={renders.heroTransparent}
          large
          sizes="(min-width: 1024px) 700px, 100vw"
        />
      </div>
      <div className={cn('flex flex-1 flex-col gap-3 p-8 sm:p-10', wide && 'lg:justify-center')}>
        <div className="flex items-center gap-3">
          <Badge variant="secondary">{PRODUCT_STATUS_LABEL[product.status]}</Badge>
          <span className="text-sm text-muted-foreground">{product.edition} edition</span>
        </div>
        <h2 className="display-3">
          <Link
            href={`/products/${product.slug}`}
            className="after:absolute after:inset-0 focus-visible:outline-none"
          >
            {product.name}
          </Link>
        </h2>
        <p className="text-base text-muted-foreground sm:text-lg">{product.summary}</p>
        <p className="mt-3 flex items-center gap-6 text-base font-semibold text-primary">
          <span>Learn more &rsaquo;</span>
          <Link href={notifyHref(product.slug)} className="relative z-10 hover:underline">
            Notify me &rsaquo;
          </Link>
        </p>
      </div>
    </Card>
  );
}
