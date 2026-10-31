import { useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { Button } from '@kivori/ui/components/button';
import { openAccessibilitySettings, requestAccessibility, resetAccessibility } from '@/lib/ipc';
import { strings } from '@/lib/i18n/strings';
import type { AccessibilityControl } from './use-accessibility';

const t = strings.accessibility;

/**
 * What the buttons do. Each one asks at most once per press; nothing here runs by itself.
 * `allow` shows the macOS prompt, `openSettings` opens the Accessibility list, `allowAndOpen`
 * does both in that order (the one-button ask), `reset` clears this app's old approval.
 */
export function useAccessibilityActions(accessibility: AccessibilityControl): {
  busy: boolean;
  allow: () => void;
  openSettings: () => void;
  allowAndOpen: () => void;
  reset: () => void;
} {
  const [busy, setBusy] = useState(false);
  const openSettings = (): void => {
    accessibility.asked();
    openAccessibilitySettings().catch(() => toast.error(t.settingsFailed));
  };
  const allow = (): void => {
    setBusy(true);
    accessibility.asked();
    requestAccessibility()
      .then(accessibility.refresh)
      .catch(() => {})
      .finally(() => setBusy(false));
  };
  const allowAndOpen = (): void => {
    setBusy(true);
    accessibility.asked();
    requestAccessibility()
      .then(accessibility.refresh)
      .catch(() => {})
      .finally(() => {
        setBusy(false);
        openAccessibilitySettings().catch(() => toast.error(t.settingsFailed));
      });
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
  return { busy, allow, openSettings, allowAndOpen, reset };
}

/** Allow access and Open System Settings, side by side. */
export function AccessibilityButtons({
  actions,
}: {
  actions: ReturnType<typeof useAccessibilityActions>;
}): ReactElement {
  return (
    <div className="flex flex-wrap gap-2">
      <Button size="sm" onClick={actions.allow} disabled={actions.busy}>
        {t.allow}
      </Button>
      <Button size="sm" variant="outline" onClick={actions.openSettings}>
        {t.openSettings}
      </Button>
    </div>
  );
}

/** Offered once the user came back and macOS still says off: reset the old approval and ask again. */
export function StaleNote({
  actions,
}: {
  actions: ReturnType<typeof useAccessibilityActions>;
}): ReactElement {
  return (
    <div role="note" className="space-y-2 rounded-lg border border-border px-3 py-2.5 text-sm">
      <p className="font-medium">{t.staleTitle}</p>
      <p>{t.staleBody}</p>
      <Button size="sm" variant="outline" onClick={actions.reset} disabled={actions.busy}>
        {t.reset}
      </Button>
    </div>
  );
}
