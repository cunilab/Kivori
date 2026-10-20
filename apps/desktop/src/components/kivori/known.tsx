import type { ReactElement, ReactNode } from 'react';
import { CircleHelp } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipTrigger } from '@kivori/ui/components/tooltip';
import { strings } from '@/lib/i18n/strings';
import { cn } from '@kivori/ui/lib/utils';

/**
 * Honest value: renders `children(value)` when known, otherwise the word "Unknown" with a help icon
 * whose tooltip says why. Never substitutes 0, "off" or a guess. (Composition of shadcn Tooltip.)
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
        aria-label={`${strings.unknownValue}. ${why}`}
        className={cn(
          'inline-flex cursor-help items-center gap-1 rounded-sm text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring',
          className,
        )}
      >
        {strings.unknownValue}
        <CircleHelp aria-hidden="true" className="size-3.5 opacity-70" />
      </TooltipTrigger>
      <TooltipContent>{why}</TooltipContent>
    </Tooltip>
  );
}
