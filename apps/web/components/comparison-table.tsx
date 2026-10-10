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
      className="overflow-x-auto rounded-2xl bg-card ring-1 ring-border"
      role="region"
      aria-label="Product comparison"
      tabIndex={0}
    >
      <Table className="min-w-[30rem]">
        <TableCaption className="sr-only">
          Specifications compared across the Kivori lineup
        </TableCaption>
        <TableHeader>
          <TableRow className="hover:bg-transparent">
            <TableCell className="w-1/3 p-6" />
            {columns.map((column) => (
              <TableHead key={column.slug} scope="col" className="h-auto p-6 align-bottom">
                <Link
                  href={`/products/${column.slug}`}
                  className="font-display text-xl font-bold tracking-tight hover:underline"
                >
                  {column.name}
                </Link>
                <span className="block text-sm font-normal text-muted-foreground">
                  {column.edition} edition
                </span>
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        {groups.map((group) => (
          <TableBody key={group.group}>
            <TableRow className="hover:bg-transparent">
              <TableHead
                scope="colgroup"
                colSpan={columns.length + 1}
                className="h-auto bg-surface px-6 py-2.5 pixel text-[0.7rem] font-normal tracking-wider text-mint"
              >
                {group.group}
              </TableHead>
            </TableRow>
            {group.rows.map((row) => (
              <TableRow key={row.label}>
                <TableHead scope="row" className="h-auto px-6 py-4 align-top text-base font-medium">
                  {row.label}
                </TableHead>
                {row.values.map((value, index) => (
                  <TableCell
                    key={columns[index].slug}
                    className="px-6 py-4 align-top text-base whitespace-normal text-muted-foreground"
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
