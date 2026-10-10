import { PRODUCT_STATUS_LABEL } from '@/lib/product-labels';
import type { Product } from '@/content/products/types';

export interface ComparisonColumn {
  slug: string;
  name: string;
  edition: string;
}

export interface ComparisonRow {
  label: string;
  /** One cell per column; `null` when that product has no such row. */
  values: (string | null)[];
}

export interface ComparisonGroup {
  group: string;
  rows: ComparisonRow[];
}

export interface Comparison {
  columns: ComparisonColumn[];
  groups: ComparisonGroup[];
}

/**
 * Builds the lineup comparison from each product's `specs`. Groups and rows keep the order they are
 * first seen in, so products sharing a spec line up in one row and a row only one product has shows a
 * gap for the others. A leading "Overview" group adds status and price.
 */
export function buildComparison(products: readonly Product[]): Comparison {
  const columns = products.map(({ slug, name, edition }) => ({ slug, name, edition }));
  const groups: ComparisonGroup[] = [
    {
      group: 'Overview',
      rows: [
        { label: 'Status', values: products.map((p) => PRODUCT_STATUS_LABEL[p.status]) },
        { label: 'Price', values: products.map((p) => p.price ?? 'Coming soon') },
      ],
    },
  ];

  products.forEach((product, column) => {
    for (const spec of product.specs) {
      let group = groups.find((g) => g.group === spec.group);
      if (!group) {
        group = { group: spec.group, rows: [] };
        groups.push(group);
      }
      for (const { label, value } of spec.rows) {
        let row = group.rows.find((r) => r.label === label);
        if (!row) {
          row = { label, values: products.map(() => null) };
          group.rows.push(row);
        }
        row.values[column] = value;
      }
    }
  });

  return { columns, groups };
}
