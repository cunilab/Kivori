import type { ReactElement, ReactNode } from 'react';
import { cn } from '@kivori/ui/lib/utils';

/** A small tinted square behind an icon: the one accent used for card and section headings. */
export function IconChip({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}): ReactElement {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'flex size-7 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary [&_svg]:size-4',
        className,
      )}
    >
      {children}
    </span>
  );
}
