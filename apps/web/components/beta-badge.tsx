import type { ReactElement } from 'react';

export function BetaBadge(): ReactElement {
  return (
    <span className="rounded-full border border-primary/40 bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary">
      Beta
    </span>
  );
}
