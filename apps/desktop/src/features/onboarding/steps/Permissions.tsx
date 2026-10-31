import type { ReactElement } from 'react';
import { CircleCheck, ShieldAlert } from 'lucide-react';
import { Button } from '@kivori/ui/components/button';
import { Skeleton } from '@kivori/ui/components/skeleton';
import { strings } from '@/lib/i18n/strings';
import { StaleNote, useAccessibilityActions } from '../../permissions/AccessibilityActions';
import type { AccessibilityControl } from '../../permissions/use-accessibility';
import { StepFrame } from './frame';

const t = strings.onboarding.permissions;

/**
 * macOS only, and only while it is off: the one time Kivori asks by itself during setup. Allow
 * shows the macOS prompt once and opens the Accessibility list; the step turns green and moves on
 * by itself once macOS says it is on. "Not now" (the footer's skip) never blocks setup.
 */
export function Permissions({
  accessibility,
}: {
  accessibility: AccessibilityControl;
}): ReactElement {
  const actions = useAccessibilityActions(accessibility);
  const { state } = accessibility;
  return (
    <StepFrame title={t.title} body={t.body}>
      {state === null ? (
        <Skeleton className="h-10 w-full" />
      ) : (
        <div
          role="status"
          aria-live="polite"
          data-state={state}
          className="flex items-center gap-3 rounded-lg bg-muted px-3 py-2.5 text-sm font-medium data-[state=granted]:bg-success/10 data-[state=granted]:text-success"
        >
          {state === 'granted' ? (
            <CircleCheck className="size-4" aria-hidden="true" />
          ) : (
            <ShieldAlert className="size-4 text-warning" aria-hidden="true" />
          )}
          {state === 'granted' ? t.granted : t.missing}
        </div>
      )}
      <ol className="space-y-2 text-sm">
        {t.steps.map((step, index) => (
          <li key={step} className="flex items-start gap-3">
            <span
              aria-hidden="true"
              className="flex size-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary"
            >
              {index + 1}
            </span>
            <span className="pt-0.5">{step}</span>
          </li>
        ))}
      </ol>
      {state === 'missing' ? (
        <>
          <div className="flex flex-wrap gap-2">
            <Button onClick={actions.allowAndOpen} disabled={actions.busy}>
              {t.allow}
            </Button>
          </div>
          {accessibility.stale ? <StaleNote actions={actions} /> : null}
          <p className="text-sm text-muted-foreground">{t.skipNote}</p>
        </>
      ) : null}
    </StepFrame>
  );
}
