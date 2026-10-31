import { useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { ShieldAlert } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@kivori/ui/components/alert';
import { Button } from '@kivori/ui/components/button';
import { openAccessibilitySettings, requestAccessibility, resetAccessibility } from '@/lib/ipc';
import { strings } from '@/lib/i18n/strings';
import { useAccessibility } from './use-accessibility';

const t = strings.accessibility;

/**
 * Asks for the macOS Accessibility permission where it is needed, and only then: it renders
 * nothing unless macOS says the permission is off (so nothing on other systems, or once granted).
 * Each button asks at most once per press. When the user came back and it still reads off, it
 * offers to reset this app's old approval and ask again.
 */
export function AccessibilityHelp({
  title,
  body,
}: {
  title: string;
  body: string;
}): ReactElement | null {
  const accessibility = useAccessibility(true);
  const [busy, setBusy] = useState(false);
  if (accessibility.state !== 'missing') return null;

  const allow = (): void => {
    setBusy(true);
    accessibility.asked();
    requestAccessibility()
      .then(accessibility.refresh)
      .catch(() => {})
      .finally(() => setBusy(false));
  };
  const openSettings = (): void => {
    accessibility.asked();
    openAccessibilitySettings().catch(() => toast.error(t.settingsFailed));
  };
  const reset = (): void => {
    setBusy(true);
    resetAccessibility()
      .then((next) => {
        accessibility.reset();
        accessibility.refresh(next);
      })
      .catch(() => toast.error(t.resetFailed))
      .finally(() => setBusy(false));
  };
  return (
    <Alert className="border-warning/40 bg-warning/5" data-testid="accessibility-help">
      <ShieldAlert className="text-warning" aria-hidden="true" />
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription className="space-y-3">
        <p>{body}</p>
        <p>{t.steps}</p>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={allow} disabled={busy}>
            {t.allow}
          </Button>
          <Button size="sm" variant="outline" onClick={openSettings}>
            {t.openSettings}
          </Button>
        </div>
        {accessibility.stale ? (
          <div role="note" className="space-y-2 rounded-lg border border-border px-3 py-2.5">
            <p className="font-medium">{t.staleTitle}</p>
            <p>{t.staleBody}</p>
            <Button size="sm" variant="outline" onClick={reset} disabled={busy}>
              {t.reset}
            </Button>
          </div>
        ) : null}
      </AlertDescription>
    </Alert>
  );
}
