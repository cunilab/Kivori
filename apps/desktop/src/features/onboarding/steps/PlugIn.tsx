import { useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { CircleCheck, LoaderCircle, TriangleAlert } from 'lucide-react';
import { Alert, AlertDescription } from '@kivori/ui/components/alert';
import { Button } from '@kivori/ui/components/button';
import { RecoveryDialog } from '@/features/device/RecoveryDialog';
import { uiConnection } from '@/hooks/use-kivori';
import { restoreFirmware } from '@/lib/ipc';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { StepFrame } from './frame';

const t = strings.onboarding.plug;

/** Reconnect attempts after which "it is still trying" becomes "it is stuck". */
export const STUCK_AFTER_ATTEMPTS = 3;

/** Whether the person should be offered the BOOT-button restore. */
export function needsRecovery(connection: ConnectionStatusDto | null): boolean {
  const ui = uiConnection(connection);
  if (ui === 'incompatible') return true;
  return (ui === 'connecting' || ui === 'reconnecting') && connection !== null
    ? connection.retryCount >= STUCK_AFTER_ATTEMPTS
    : false;
}

/**
 * Follows the real connection status: the step offers Continue only once Kivori is connected. An
 * incompatible or stuck device leads to the same BOOT-button recovery the Device page has.
 */
export function PlugIn({
  connection,
  updateAvailable,
}: {
  connection: ConnectionStatusDto | null;
  updateAvailable: boolean;
}): ReactElement {
  const ui = uiConnection(connection);
  const [recovering, setRecovering] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const recovery = needsRecovery(connection);

  const restore = (): void => {
    setRecovering(false);
    setRestoring(true);
    restoreFirmware().catch(() => {
      setRestoring(false);
      toast.error(t.restoreFailed);
    });
  };

  return (
    <StepFrame title={t.title} body={t.body}>
      <div
        role="status"
        aria-live="polite"
        data-state={ui}
        className="flex items-center gap-3 rounded-lg bg-muted px-3 py-2.5 text-sm font-medium"
      >
        {ui === 'connected' ? (
          <>
            <CircleCheck className="size-4 text-success" aria-hidden="true" />
            {t.connected}
          </>
        ) : ui === 'incompatible' ? (
          <>
            <TriangleAlert className="size-4 text-destructive" aria-hidden="true" />
            {t.incompatible}
          </>
        ) : (
          <>
            <LoaderCircle
              className="size-4 text-muted-foreground motion-safe:animate-spin"
              aria-hidden="true"
            />
            {restoring ? t.restoring : recovery ? t.stuck : t.waiting}
          </>
        )}
      </div>

      {updateAvailable ? (
        <p className="text-sm" role="status" data-testid="onboarding-update-hint">
          {t.updateHint}
        </p>
      ) : null}

      {recovery && !restoring ? (
        <Alert variant="destructive">
          <TriangleAlert aria-hidden="true" />
          <AlertDescription className="space-y-2">
            <span>{ui === 'incompatible' ? t.incompatible : t.stuck}</span>
            <span className="block">
              <Button variant="outline" size="sm" onClick={() => setRecovering(true)}>
                {t.restore}
              </Button>
            </span>
          </AlertDescription>
        </Alert>
      ) : null}

      {ui !== 'connected' ? (
        <div className="space-y-2">
          <h2 className="text-sm font-semibold">{t.tipsHeading}</h2>
          <ul className="list-disc space-y-1 pl-5 text-sm text-muted-foreground">
            {t.tips.map((tip) => (
              <li key={tip}>{tip}</li>
            ))}
          </ul>
        </div>
      ) : null}

      <RecoveryDialog open={recovering} onOpenChange={setRecovering} onRestore={restore} />
    </StepFrame>
  );
}
