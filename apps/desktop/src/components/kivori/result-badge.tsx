import type { ReactElement } from 'react';
import { CircleCheck, CircleHelp, LoaderCircle, TriangleAlert } from 'lucide-react';
import { Badge } from '@kivori/ui/components/badge';
import type { DeskResult } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { cn } from '@kivori/ui/lib/utils';

// `tone` is the honest-result contract: only a confirmed/started outcome is "success" (green, check).
// Unverified is amber with a "?" like the device badge, never a check; an error is red.
export const RESULT_STYLE = {
  processing: {
    tone: 'neutral',
    Icon: LoaderCircle,
    className: 'border-transparent bg-muted text-muted-foreground',
  },
  stateConfirmed: {
    tone: 'success',
    Icon: CircleCheck,
    className: 'border-transparent bg-success/12 text-success',
  },
  executionConfirmed: {
    tone: 'success',
    Icon: CircleCheck,
    className: 'border-transparent bg-success/12 text-success',
  },
  unverified: {
    tone: 'neutral',
    Icon: CircleHelp,
    className: 'border-warning/40 bg-warning/10 text-warning',
  },
  error: {
    tone: 'error',
    Icon: TriangleAlert,
    className: 'border-transparent bg-destructive/12 text-destructive',
  },
} as const;

export function ResultBadge({ result }: { result: DeskResult }): ReactElement {
  const { tone, Icon, className } = RESULT_STYLE[result];
  return (
    <Badge
      variant="outline"
      data-result={result}
      data-tone={tone}
      className={cn('h-6 gap-1.5 px-2.5', className)}
    >
      <Icon
        data-icon="inline-start"
        aria-hidden="true"
        className={result === 'processing' ? 'animate-spin' : undefined}
      />
      {strings.results[result]}
    </Badge>
  );
}
