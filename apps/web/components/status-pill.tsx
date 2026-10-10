import type { ReactElement } from 'react';
import type { ProductStatus } from '@/content/products/types';
import { PRODUCT_STATUS_LABEL } from '@/lib/product-labels';

export function StatusPill({ status }: { status: ProductStatus }): ReactElement {
  return (
    <span className="rounded-full border border-border bg-accent px-3 py-1 text-xs font-medium tracking-wide text-accent-foreground uppercase">
      {PRODUCT_STATUS_LABEL[status]}
    </span>
  );
}
