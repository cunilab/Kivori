import { useState } from 'react';
import type { ReactElement } from 'react';
import { LoaderCircle } from 'lucide-react';
import { Button } from '@kivori/ui/components/button';
import { Label } from '@kivori/ui/components/label';
import { Switch } from '@kivori/ui/components/switch';
import { useUpdateStatus } from '@/hooks/use-kivori';
import { checkForUpdates, setAutoUpdateCheck } from '@/lib/ipc';
import { format, strings } from '@/lib/i18n/strings';
import { requestUpdateDialog } from './dialog-request';

const t = strings.updates;

function when(iso: string | null): string {
  if (!iso) return t.never;
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? t.never
    : date.toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
}

/**
 * The Settings page's app update rows (ADR-0015): last check, the automatic-check switch, "Check
 * now", and the offer when there is one. A failed or offline check shows nothing (product.md).
 */
export function AppUpdates(): ReactElement | null {
  const status = useUpdateStatus();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  if (!status) return null;

  const off = status.state === 'disabled' || status.state === 'unsupported';
  const checking = status.state === 'checking';

  const toggle = (enabled: boolean): void => {
    setBusy(true);
    setError(false);
    setAutoUpdateCheck(enabled)
      .catch(() => setError(true))
      .finally(() => setBusy(false));
  };
  const check = (): void => {
    // Nothing to say on failure: the result (or its absence) arrives as `updater://status`.
    checkForUpdates().catch(() => {});
  };

  return (
    <div className="space-y-3 pt-3" data-testid="app-updates">
      {off ? (
        <p className="text-xs text-muted-foreground">
          {status.state === 'unsupported' ? t.unsupported : t.disabled[status.reason ?? 'devBuild']}
        </p>
      ) : null}
      <div className="flex items-center justify-between gap-4 text-sm">
        <span className="text-muted-foreground">{t.lastChecked}</span>
        <span className="text-right font-medium" data-testid="update-last-checked">
          {when(status.lastChecked)}
        </span>
      </div>
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-0.5">
          <Label htmlFor="auto-update-check">{t.autoCheck}</Label>
          <p id="auto-update-check-hint" className="text-xs text-muted-foreground">
            {t.autoCheckHint}
          </p>
        </div>
        <Switch
          id="auto-update-check"
          aria-describedby="auto-update-check-hint"
          checked={status.autoCheck}
          disabled={busy || off}
          onCheckedChange={toggle}
        />
      </div>
      {error ? (
        <p role="alert" className="text-xs text-destructive">
          {t.failedToChange}
        </p>
      ) : null}
      <div className="flex flex-wrap items-center justify-between gap-2">
        {status.update ? (
          <>
            <p className="text-sm font-medium" role="status">
              {format(t.available, { version: status.update.version })}
            </p>
            <Button size="sm" onClick={requestUpdateDialog}>
              {t.view}
            </Button>
          </>
        ) : (
          <>
            <p className="text-xs text-muted-foreground" role="status">
              {checking ? t.checking : status.lastChecked && !off ? t.upToDate : null}
            </p>
            <Button size="sm" variant="outline" disabled={status.state !== 'idle'} onClick={check}>
              {checking ? <LoaderCircle className="animate-spin" aria-hidden="true" /> : null}
              {t.checkNow}
            </Button>
          </>
        )}
      </div>
    </div>
  );
}
