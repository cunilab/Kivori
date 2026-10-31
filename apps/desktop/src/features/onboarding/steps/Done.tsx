import type { ReactElement } from 'react';
import { CircleAlert, CircleCheck } from 'lucide-react';
import { strings } from '@/lib/i18n/strings';
import { StepFrame } from './frame';

const t = strings.onboarding.done;

/** The last step. Skipped steps that are still not ready are listed instead of claiming all is set. */
export function Done({
  error,
  open = [],
}: {
  error: string | null;
  open?: readonly 'plug'[];
}): ReactElement {
  const complete = open.length === 0;
  return (
    <StepFrame title={complete ? t.title : t.openTitle} body={complete ? t.body : t.openBody}>
      {complete ? (
        <CircleCheck className="size-10 text-success" aria-hidden="true" />
      ) : (
        <ul className="space-y-2" data-testid="onboarding-open-steps">
          {open.map((id) => (
            <li key={id} className="flex items-start gap-3 rounded-xl bg-muted p-3 text-sm">
              <CircleAlert className="mt-0.5 size-4 shrink-0 text-warning" aria-hidden="true" />
              <span>{t.open[id]}</span>
            </li>
          ))}
        </ul>
      )}
      {error ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
    </StepFrame>
  );
}
