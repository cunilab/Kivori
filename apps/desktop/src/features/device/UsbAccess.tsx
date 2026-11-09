import { useState } from 'react';
import type { ReactElement } from 'react';
import { ShieldAlert } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@kivori/ui/components/alert';
import { Button } from '@kivori/ui/components/button';
import { fixSerialAccess } from '@/lib/ipc';
import { useSerialAccess } from '@/hooks/use-kivori';
import { strings } from '@/lib/i18n/strings';

const t = strings.device.usbAccess;

/** A reason the native side gave, else the generic line. */
function reason(error: unknown): string {
  if (typeof error === 'string' && error) return error;
  if (error instanceof Error && error.message) return error.message;
  return t.fixFailed;
}

/**
 * "Kivori needs permission to use the USB device": shown only while the desktop's last attempt to
 * open the device was refused for permission (Linux). With `pkexec` there is one button; without it
 * the same fix is a command to copy. It disappears by itself once the device opens.
 */
export function UsbAccess(): ReactElement | null {
  const access = useSerialAccess();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  if (!access?.denied) return null;

  const fix = (): void => {
    setBusy(true);
    setError(null);
    fixSerialAccess()
      .catch((failure: unknown) => setError(reason(failure)))
      .finally(() => setBusy(false));
  };
  const copy = (command: string): void => {
    void navigator.clipboard
      .writeText(command)
      .then(() => setCopied(true))
      .catch(() => setCopied(false));
  };

  return (
    <Alert data-testid="usb-access" role="alert">
      <ShieldAlert aria-hidden="true" />
      <AlertTitle>{t.title}</AlertTitle>
      <AlertDescription className="space-y-3">
        <p>{t.body}</p>
        {access.canFix ? (
          <Button
            type="button"
            size="sm"
            onClick={fix}
            disabled={busy}
            data-testid="usb-access-fix"
          >
            {busy ? t.fixing : t.fix}
          </Button>
        ) : access.manualCommand ? (
          <div className="space-y-2">
            <p>{t.manualBody}</p>
            <code
              className="block break-all rounded-md bg-muted px-2 py-1.5 font-mono text-xs"
              data-testid="usb-access-command"
            >
              {access.manualCommand}
            </code>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => copy(access.manualCommand ?? '')}
            >
              {copied ? t.copied : t.copy}
            </Button>
          </div>
        ) : null}
        {error ? (
          <p className="text-destructive" role="status" data-testid="usb-access-error">
            {error}
          </p>
        ) : null}
      </AlertDescription>
    </Alert>
  );
}
