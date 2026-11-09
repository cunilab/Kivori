import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { AppWindow } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@kivori/ui/components/alert';
import { Button } from '@kivori/ui/components/button';
import { strings } from '@/lib/i18n/strings';
import { enableGnomeFocus, getGnomeFocusStatus, logoutGnome } from '@/lib/ipc/gnome';
import type { GnomeFocusStatus } from '@/lib/ipc/gnome';
import { errorText } from '@/lib/utils';

const t = strings.gnomeFocus;

/** Opt-in only; GNOME Wayland is the only session that can show this card. */
export function GnomeFocusHelp(): ReactElement | null {
  const [status, setStatus] = useState<GnomeFocusStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    let reading = false;
    const refresh = (): void => {
      if (reading) return;
      reading = true;
      void getGnomeFocusStatus()
        .then((next) => active && setStatus(next))
        .catch(() => {})
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
    } catch (reason) {
      setError(errorText(reason));
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
      <AlertDescription className="space-y-3">
        <p>{t[status]}</p>
        <p>{t.privacy}</p>
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
              <span className="text-xs">{updating ? t.updateOnce : t.once}</span>
            ) : null}
          </div>
        ) : null}
        {error ? (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        ) : null}
      </AlertDescription>
    </Alert>
  );
}
