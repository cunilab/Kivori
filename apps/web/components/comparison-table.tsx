import Link from 'next/link';
import type { ReactElement } from 'react';
import {
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@kivori/ui/components/table';
import type { Comparison } from '@/lib/comparison';

/** Spec comparison generated from product data. Scrolls sideways on narrow screens (focusable for keyboards). */
export function ComparisonTable({ comparison }: { comparison: Comparison }): ReactElement {
  const { columns, groups } = comparison;
  return (
    <div
      className="rounded-xl border border-border bg-card"
      role="region"
      aria-label="Product comparison"
      tabIndex={0}
    >
      <Table className="min-w-120">
        <TableCaption className="sr-only">
          Specifications compared across the Kivori lineup
        </TableCaption>
        <TableHeader>
          <TableRow>
            <TableCell className="w-1/3 p-4" />
            {columns.map((column) => (
              <TableHead key={column.slug} scope="col" className="h-auto p-4 align-bottom">
                <Link
                  href={`/products/${column.slug}`}
                  className="font-display text-base hover:underline"
                >
                  {column.name}
                </Link>
                <span className="block text-xs font-normal text-muted-foreground">
                  {column.edition} edition
                </span>
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        {groups.map((group) => (
          <TableBody key={group.group}>
            <TableRow>
              <TableHead
                scope="colgroup"
                colSpan={columns.length + 1}
                className="h-auto bg-secondary px-4 py-2 text-xs font-semibold tracking-wide text-muted-foreground uppercase"
              >
                {group.group}
              </TableHead>
            </TableRow>
            {group.rows.map((row) => (
              <TableRow key={row.label}>
                <TableHead scope="row" className="h-auto p-4 align-top font-medium">
                  {row.label}
                </TableHead>
                {row.values.map((value, index) => (
                  <TableCell
                    key={columns[index].slug}
                    className="p-4 align-top whitespace-normal text-muted-foreground"
                  >
                    {value ?? <span aria-label="Not applicable">&mdash;</span>}
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        ))}
      </Table>
    </div>
  );
}
