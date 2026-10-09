import { useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { CircleCheck, ShieldAlert } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { openAccessibilitySettings, requestAccessibility } from '@/lib/ipc';
import type { AccessibilityState } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { StepFrame } from './frame';

const t = strings.onboarding.permissions;

/** macOS only: asks for Accessibility, and turns green when macOS says it is on. */
export function Permissions({
  state,
  onChange,
}: {
  state: AccessibilityState | null;
  onChange: (next: AccessibilityState) => void;
}): ReactElement {
  const [busy, setBusy] = useState(false);
  const allow = (): void => {
    setBusy(true);
    requestAccessibility()
      .then(onChange)
      .catch(() => {})
      .finally(() => setBusy(false));
  };
  const openSettings = (): void => {
    openAccessibilitySettings().catch(() => toast.error(t.settingsFailed));
  };
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
        <div className="flex flex-wrap gap-2">
          <Button onClick={allow} disabled={busy}>
            {t.allow}
          </Button>
          <Button variant="outline" onClick={openSettings}>
            {t.openSettings}
          </Button>
        </div>
      ) : null}
      {state === 'missing' ? <p className="text-sm text-muted-foreground">{t.skipNote}</p> : null}
    </StepFrame>
  );
}
