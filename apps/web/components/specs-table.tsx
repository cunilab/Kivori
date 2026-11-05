import type { ReactElement } from 'react';
import type { SpecGroup } from '@/content/products/types';

export function SpecsTable({ groups, name }: { groups: SpecGroup[]; name: string }): ReactElement {
  return (
    <div aria-label={`${name} tech specs`} className="grid gap-x-12 gap-y-7 md:grid-cols-2">
      {groups.map((group) => (
        <section
          key={group.group}
          aria-labelledby={`spec-${group.group.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`}
        >
          <h3
            id={`spec-${group.group.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`}
            className="border-b border-border pb-2 text-base font-semibold"
          >
            {group.group}
          </h3>
          <dl>
            {group.rows.map((row) => (
              <div
                key={row.label}
                className="grid grid-cols-[minmax(6rem,0.9fr)_minmax(0,1.3fr)] gap-3 border-b border-border/70 py-2 text-sm leading-5"
              >
                <dt className="font-medium">{row.label}</dt>
                <dd className="text-muted-foreground">{row.value}</dd>
              </div>
            ))}
          </dl>
        </section>
      ))}
    </div>
  );
}
