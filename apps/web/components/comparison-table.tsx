import Link from 'next/link';
import type { ReactElement } from 'react';
import type { Comparison } from '@/lib/comparison';

/** Spec comparison generated from product data. Scrolls sideways on narrow screens (focusable for keyboards). */
export function ComparisonTable({ comparison }: { comparison: Comparison }): ReactElement {
  const { columns, groups } = comparison;
  return (
    <div
      className="overflow-x-auto rounded-xl border border-border bg-card"
      role="region"
      aria-label="Product comparison"
      tabIndex={0}
    >
      <table className="w-full min-w-120 border-collapse text-left text-sm">
        <caption className="sr-only">Specifications compared across the Kivori lineup</caption>
        <thead>
          <tr className="border-b border-border">
            <td className="w-1/3 p-4" />
            {columns.map((column) => (
              <th key={column.slug} scope="col" className="p-4 align-bottom font-display text-base">
                <Link href={`/products/${column.slug}`} className="hover:underline">
                  {column.name}
                </Link>
                <span className="block text-xs font-normal text-muted-foreground">
                  {column.edition} edition
                </span>
              </th>
            ))}
          </tr>
        </thead>
        {groups.map((group) => (
          <tbody key={group.group} className="border-b border-border last:border-b-0">
            <tr>
              <th
                scope="colgroup"
                colSpan={columns.length + 1}
                className="bg-secondary px-4 py-2 text-xs font-semibold tracking-wide text-muted-foreground uppercase"
              >
                {group.group}
              </th>
            </tr>
            {group.rows.map((row) => (
              <tr key={row.label} className="border-t border-border">
                <th scope="row" className="p-4 align-top font-medium">
                  {row.label}
                </th>
                {row.values.map((value, index) => (
                  <td key={columns[index].slug} className="p-4 align-top text-muted-foreground">
                    {value ?? <span aria-label="Not applicable">&mdash;</span>}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        ))}
      </table>
    </div>
  );
}
