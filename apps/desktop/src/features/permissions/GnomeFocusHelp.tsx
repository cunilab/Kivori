import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { AppWindow } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@kivori/ui/components/alert';
import { Button } from '@kivori/ui/components/button';
import { strings } from '@/lib/i18n/strings';
import { enableGnomeFocus, getGnomeFocusStatus, logoutGnome } from '@/lib/ipc/gnome';
import type { GnomeFocusStatus } from '@/lib/ipc/gnome';

const t = strings.gnomeFocus;

/** Opt-in only; GNOME Wayland is the only session that can show this card. */
export function GnomeFocusHelp(): ReactElement | null {
  const [status, setStatus] = useState<GnomeFocusStatus | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    let reading = false;
    const refresh = (): void => {
      if (reading) return;
      reading = true;
      void getGnomeFocusStatus()
        .then((next) => {
          if (!active) return;
          setStatus(next);
          setReadError(null);
        })
        .catch(() => {
          if (!active) return;
          setStatus('unavailable');
          setReadError(t.readFailed);
        })
        .finally(() => {
          reading = false;
        });
    };
    refresh();
    const timer = setInterval(refresh, 5000);
    window.addEventListener('focus', refresh);
    return () => {
      active = false;
      clearInterval(timer);
      window.removeEventListener('focus', refresh);
    };
  }, []);

  if (!status || status === 'notApplicable' || status === 'working') return null;
  const run = async (action: 'enable' | 'logout'): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      if (action === 'enable') setStatus(await enableGnomeFocus());
      else await logoutGnome();
    } catch {
      setError(action === 'logout' ? t.logoutFailed : updating ? t.updateFailed : t.installFailed);
    } finally {
      setBusy(false);
    }
  };
  const needsLogout = status === 'needsLogout' || status === 'cliMissing';
  const updating = status === 'updateAvailable';
  const title =
    status === 'cliMissing'
      ? t.cliMissingTitle
      : needsLogout
        ? t.logoutTitle
        : updating
          ? t.updateTitle
          : t.title;
  return (
    <Alert className="border-primary/30 bg-primary/5" data-testid="gnome-focus-help">
      <AppWindow className="text-primary" aria-hidden="true" />
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription className="space-y-3 [&_p:not(:last-child)]:mb-0">
        <p>{t[status]}</p>
        {status !== 'unsupported' ? (
          <div className="flex flex-wrap items-center gap-3">
            <Button
              size="sm"
              disabled={busy}
              onClick={() => void run(needsLogout ? 'logout' : 'enable')}
            >
              {busy ? t.busy : needsLogout ? t.logout : updating ? t.update : t.enable}
            </Button>
            {!needsLogout ? (
              <span className="text-xs text-muted-foreground">
                {updating ? t.updateOnce : t.once}
              </span>
            ) : null}
          </div>
        ) : null}
        <p className="text-xs text-muted-foreground">{t.privacy}</p>
        <details className="text-xs">
          <summary className="cursor-pointer rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-ring">
            {t.howItWorks}
          </summary>
          <ul className="mt-2 max-w-prose list-disc space-y-1.5 pl-4">
            <li>{t.details.sees}</li>
            <li>{t.details.approval}</li>
            <li>{t.details.remembered}</li>
            <li>{t.details.paused}</li>
            <li>{t.details.malware}</li>
          </ul>
          <p className="mt-2">{t.details.docs}</p>
        </details>
        {error || readError ? (
          <p role="alert" className="text-destructive">
            {error ?? readError}
          </p>
        ) : null}
      </AlertDescription>
    </Alert>
  );
}
