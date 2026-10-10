import type { ReactElement, ReactNode } from 'react';
import { Tooltip, TooltipContent, TooltipTrigger } from '@kivori/ui/components/tooltip';
import { strings } from '@/lib/i18n/strings';
import { cn } from '@kivori/ui/lib/utils';

/**
 * Honest value: renders `children(value)` when known, otherwise an explicit "—" whose tooltip says
 * why it is unknown. Never substitutes 0, "off" or a guess. (Composition of shadcn Tooltip.)
 */
export function Known<T>({
  value,
  why,
  children,
  className,
}: {
  value: T | null | undefined;
  why: string;
  children: (value: T) => ReactNode;
  className?: string;
}): ReactElement {
  if (value !== null && value !== undefined) return <>{children(value)}</>;
  return (
    <Tooltip>
      <TooltipTrigger
        aria-label={`${strings.unknown} ${why}`}
        className={cn(
          'cursor-help rounded-sm text-muted-foreground underline decoration-dotted underline-offset-4 outline-none focus-visible:ring-2 focus-visible:ring-ring',
          className,
        )}
      >
        {strings.unknown}
      </TooltipTrigger>
      <TooltipContent>{why}</TooltipContent>
    </Tooltip>
  );
}
