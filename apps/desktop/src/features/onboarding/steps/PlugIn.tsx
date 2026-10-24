import type { ReactElement } from 'react';
import { CircleCheck, LoaderCircle, TriangleAlert } from 'lucide-react';
import { FirmwareUpdate } from '@/features/device/FirmwareUpdate';
import { uiConnection } from '@/hooks/use-kivori';
import { sessionState } from '@/lib/device-support';
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
 * Continue requires a current usable identity, not just a transport connection. An
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
  const recovery = needsRecovery(connection);
  const session = sessionState(connection);
  const warning = ui === 'incompatible' || session === 'unknown' || session === 'attention';
  const message =
    session === 'ready'
      ? t.connected
      : session === 'pending'
        ? t.pending
        : session === 'unknown'
          ? t.unknown
          : session === 'attention'
            ? t.attention
            : ui === 'incompatible'
              ? t.incompatible
              : recovery
                ? t.stuck
                : t.waiting;

  return (
    <StepFrame title={t.title} body={t.body}>
      <div
        role="status"
        aria-live="polite"
        data-state={ui}
        className="flex items-center gap-3 rounded-lg bg-muted px-3 py-2.5 text-sm font-medium"
      >
        {session === 'ready' ? (
          <CircleCheck className="size-4 text-success" aria-hidden="true" />
        ) : warning ? (
          <TriangleAlert className="size-4 text-destructive" aria-hidden="true" />
        ) : (
          <LoaderCircle
            className="size-4 text-muted-foreground motion-safe:animate-spin"
            aria-hidden="true"
          />
        )}
        {message}
      </div>

      {updateAvailable && session === 'ready' ? (
        <p className="text-sm" role="status" data-testid="onboarding-update-hint">
          {t.updateHint}
        </p>
      ) : null}

      <FirmwareUpdate connected={session === 'ready'} incompatible={recovery} recoveryOnly />

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
    </StepFrame>
  );
}
