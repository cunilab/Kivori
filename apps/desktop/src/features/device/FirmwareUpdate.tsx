import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { Download, LoaderCircle, Plug, TriangleAlert } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { flashFirmware, getFirmwareStatus } from '@/lib/ipc';
import type { FirmwareStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { cn } from '@/lib/utils';

const PHASES = ['preparing', 'flashing', 'reconnecting', 'succeeded'] as const;

/** Native state keeps an update observable when Overview is left and reopened. */
export function FirmwareUpdate({ connected }: { connected: boolean }): ReactElement {
  const [status, setStatus] = useState<FirmwareStatusDto | null>(null);
  const [requesting, setRequesting] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [requestError, setRequestError] = useState<string | null>(null);
  const [pollError, setPollError] = useState<string | null>(null);
  const pending = useRef(false);
  const mounted = useRef(false);
  const t = strings.firmwareUpdate;

  useEffect(() => {
    mounted.current = true;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async (): Promise<void> => {
      try {
        const snapshot = await getFirmwareStatus();
        if (active) {
          setStatus(snapshot);
          setPollError(null);
        }
      } catch (error) {
        if (active) setPollError(error instanceof Error ? error.message : String(error));
      } finally {
        if (active) timer = setTimeout(() => void poll(), 750);
      }
    };
    void poll();
    return () => {
      mounted.current = false;
      active = false;
      clearTimeout(timer);
    };
  }, []);

  const busy =
    requesting ||
    status?.phase === 'preparing' ||
    status?.phase === 'flashing' ||
    status?.phase === 'reconnecting';
  const canFlash = connected && status?.available && !busy && !pollError;
  const error = requestError ?? pollError;

  const start = async (): Promise<void> => {
    if (!canFlash || pending.current) return;
    pending.current = true;
    setRequesting(true);
    setRequestError(null);
    try {
      await flashFirmware();
      const snapshot = await getFirmwareStatus();
      if (mounted.current) setStatus(snapshot);
    } catch (failure) {
      if (mounted.current) {
        setRequestError(failure instanceof Error ? failure.message : String(failure || t.failed));
      }
    } finally {
      pending.current = false;
      if (mounted.current) setRequesting(false);
    }
  };

  const phase = status?.phase ?? 'idle';
  const stepIndex = PHASES.indexOf(phase as (typeof PHASES)[number]);

  return (
    <Card>
      <CardHeader>
        <CardTitle id="firmware-heading" role="heading" aria-level={2}>
          {t.heading}
        </CardTitle>
        <CardDescription>{t.description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {stepIndex >= 0 || phase === 'failed' ? (
          <ol className="grid grid-cols-4 gap-2" aria-label={t.heading}>
            {PHASES.map((step, index) => {
              const done = phase === 'succeeded' || index < stepIndex;
              const current = index === stepIndex && phase !== 'succeeded';
              return (
                <li
                  key={step}
                  aria-current={current ? 'step' : undefined}
                  className="flex flex-col gap-1.5 text-xs"
                >
                  <span
                    className={cn(
                      'h-1.5 rounded-full bg-muted transition-colors',
                      done && 'bg-success',
                      current && 'bg-primary motion-safe:animate-pulse',
                    )}
                  />
                  <span
                    className={cn('text-muted-foreground', (done || current) && 'text-foreground')}
                  >
                    {t.phases[step]}
                  </span>
                </li>
              );
            })}
          </ol>
        ) : null}

        <p
          aria-live="polite"
          className={cn(
            'text-sm',
            phase === 'failed' ? 'text-destructive' : 'text-muted-foreground',
          )}
        >
          {requesting ? t.preparing : (status?.message ?? t.checking)}
        </p>
        {!connected && !busy ? <p className="text-sm text-muted-foreground">{t.connect}</p> : null}

        <AlertDialog open={confirming} onOpenChange={setConfirming}>
          <AlertDialogTrigger
            render={<Button disabled={!canFlash} aria-describedby="firmware-caution" />}
          >
            {busy ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" />
            ) : (
              <Download aria-hidden="true" />
            )}
            {busy ? t.working : t.action}
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>{t.confirmTitle}</AlertDialogTitle>
              <AlertDialogDescription>{t.caution}</AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>{t.cancel}</AlertDialogCancel>
              <AlertDialogAction
                onClick={() => {
                  setConfirming(false);
                  void start();
                }}
              >
                {t.confirm}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
        <p id="firmware-caution" className="flex items-start gap-2 text-xs text-muted-foreground">
          <Plug className="mt-px size-3.5 shrink-0" aria-hidden="true" />
          {t.caution}
        </p>
        {error ? (
          <Alert variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}
      </CardContent>
    </Card>
  );
}
