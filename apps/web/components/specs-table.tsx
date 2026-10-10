import type { ReactElement } from 'react';
import {
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableRow,
} from '@kivori/ui/components/table';
import type { SpecGroup } from '@/content/products/types';

/** One product's selling-level spec sheet, grouped. */
export function SpecsTable({ groups, name }: { groups: SpecGroup[]; name: string }): ReactElement {
  return (
    <div className="overflow-x-auto rounded-3xl bg-card ring-1 ring-foreground/10">
      <Table>
        <TableCaption className="sr-only">{name} tech specs</TableCaption>
        {groups.map((group) => (
          <TableBody key={group.group}>
            <TableRow className="hover:bg-transparent">
              <TableHead
                colSpan={2}
                scope="colgroup"
                className="h-auto bg-surface px-6 py-2.5 text-xs font-semibold tracking-wide text-muted-foreground uppercase"
              >
                {group.group}
              </TableHead>
            </TableRow>
            {group.rows.map((row) => (
              <TableRow key={row.label}>
                <TableHead
                  scope="row"
                  className="h-auto w-2/5 px-6 py-4 align-top text-base font-medium"
                >
                  {row.label}
                </TableHead>
                <TableCell className="px-6 py-4 text-base whitespace-normal text-muted-foreground">
                  {row.value}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        ))}
      </Table>
    </div>
  );
}
