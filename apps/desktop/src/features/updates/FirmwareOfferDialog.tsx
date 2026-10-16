import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { CircleCheck, TriangleAlert } from 'lucide-react';
import { Alert, AlertDescription } from '@kivori/ui/components/alert';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@kivori/ui/components/alert-dialog';
import { Button } from '@kivori/ui/components/button';
import { Progress } from '@kivori/ui/components/progress';
import { useFirmwareOffer, useUpdateStatus } from '@/hooks/use-kivori';
import {
  acceptFirmwareOffer,
  dismissFirmwareOffer,
  getFirmwareStatus,
  onFirmwareOfferRequested,
} from '@/lib/ipc';
import type { FirmwareOfferDto, FirmwareStatusDto } from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { failureMessage } from '../device/firmware-copy';

const t = strings.updates.firmwareOffer;

// Offers whose dialog already opened by itself in this window: once per key (device, bundled
// version and connection). The tray item can always reopen it.
const shown = new Set<string>();

/** Tests only. */
export function resetFirmwareOfferDialog(): void {
  shown.clear();
}

const PHASE_STEPS = ['preparing', 'flashing', 'reconnecting', 'succeeded'] as const;
const POLL_MS = 750;

/** Whether the offer is a question the user answers. */
function asks(offer: FirmwareOfferDto | null): boolean {
  return offer?.kind === 'offerOptional' || offer?.kind === 'required';
}

type Stage = 'ask' | 'running' | 'done';

/**
 * The firmware update offer (ADR-0015, step 6): "Update your Kivori?", the product, current and new
 * firmware, and Update now / Later ("Not now" for a required update, which only closes it). The
 * native side decides when to offer (connected, idle for a few seconds, never during a flash or a
 * gesture, and only after a pending app update). "Update now" runs the ordinary flash and this
 * dialog follows its phases until it succeeds or fails.
 */
export function FirmwareOfferDialog(): ReactElement | null {
  const offer = useFirmwareOffer();
  const updates = useUpdateStatus();
  const [open, setOpen] = useState(false);
  const [stage, setStage] = useState<Stage>('ask');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState(false);
  const [flash, setFlash] = useState<FirmwareStatusDto | null>(null);
  // What the dialog is about: kept while an update runs, when the live offer is already gone.
  const [subject, setSubject] = useState<FirmwareOfferDto | null>(null);
  const offerRef = useRef(offer);
  offerRef.current = offer;

  const key = asks(offer) ? offer?.key : null;
  // App first: while an app update is offered, or the updater's status is not known yet, the
  // firmware dialog does not open by itself (the native side already holds the offer back as
  // `deferredForAppUpdate`; this keeps the window from racing it).
  const appFirst = updates === null || Boolean(updates.update);

  useEffect(() => {
    if (key && !appFirst && !shown.has(key)) {
      shown.add(key);
      setSubject(offerRef.current);
      setStage('ask');
      setError(false);
      setOpen(true);
    }
  }, [key, appFirst]);

  // While still asking, the dialog follows the offer: withdrawn (unplugged, Later from elsewhere)
  // closes it; held back for an app update closes it and lets it open again afterwards.
  useEffect(() => {
    if (stage !== 'ask') return;
    if (asks(offer)) {
      setSubject(offer);
      return;
    }
    setOpen(false);
    if (offer?.kind === 'deferredForAppUpdate' && subject?.key) shown.delete(subject.key);
  }, [offer, stage, subject?.key]);

  useEffect(() => {
    const reopen = (): void => {
      if (!asks(offerRef.current)) return;
      setSubject(offerRef.current);
      setStage('ask');
      setError(false);
      setOpen(true);
    };
    let stop: () => void = () => {};
    let active = true;
    void onFirmwareOfferRequested(reopen)
      .then((unlisten) => {
        if (active) stop = unlisten;
        else unlisten();
      })
      .catch(() => {});
    return () => {
      active = false;
      stop();
    };
  }, []);

  // Follows the flash "Update now" started, until it succeeds or fails.
  useEffect(() => {
    if (stage !== 'running') return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async (): Promise<void> => {
      try {
        const status = await getFirmwareStatus();
        if (!active) return;
        setFlash(status);
        if (status.phase === 'succeeded' || status.phase === 'failed') {
          setStage('done');
          return;
        }
      } catch {
        // A missed poll is retried; the native flash carries on regardless.
      }
      if (active) timer = setTimeout(() => void poll(), POLL_MS);
    };
    void poll();
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [stage]);

  if (!subject || !asks(subject)) return null;
  const required = subject.kind === 'required';

  const update = (): void => {
    setPending(true);
    setError(false);
    acceptFirmwareOffer()
      .then(() => {
        setFlash(null);
        setStage('running');
      })
      .catch(() => setError(true))
      .finally(() => setPending(false));
  };
  const later = (): void => {
    setPending(true);
    setError(false);
    dismissFirmwareOffer()
      .then(() => setOpen(false))
      .catch(() => setError(true))
      .finally(() => setPending(false));
  };

  const phase = flash?.phase ?? 'preparing';
  const step = PHASE_STEPS.indexOf(phase as (typeof PHASE_STEPS)[number]);
  const failed = stage === 'done' && flash?.phase === 'failed';

  return (
    <AlertDialog open={open} onOpenChange={(next) => stage !== 'running' && setOpen(next)}>
      <AlertDialogContent className="data-[size=default]:sm:max-w-md">
        <AlertDialogHeader>
          <AlertDialogTitle>{required ? t.requiredTitle : t.title}</AlertDialogTitle>
          <AlertDialogDescription>
            {subject.product ? (
              <span className="block font-medium text-foreground" data-testid="offer-product">
                {subject.product}
              </span>
            ) : null}
            {subject.to != null ? (
              <span className="block" data-testid="offer-versions">
                {subject.kind === 'offerOptional'
                  ? format(t.versions, { from: subject.from ?? '', to: subject.to ?? '' })
                  : format(t.requiredVersion, { to: subject.to })}
              </span>
            ) : null}
            {required && subject.reason ? (
              <span className="block">{t.required[subject.reason]}</span>
            ) : null}
            <span className="block">{t.duration}</span>
          </AlertDialogDescription>
        </AlertDialogHeader>

        {stage === 'running' ? (
          <Progress aria-label={t.progress} value={((Math.max(step, 0) + 1) / 4) * 100}>
            <span className="text-xs text-muted-foreground" role="status">
              {phase === 'flashing' || phase === 'reconnecting'
                ? t.phases[phase]
                : t.phases.preparing}
            </span>
          </Progress>
        ) : null}
        {stage === 'done' && !failed ? (
          <p role="status" className="flex items-center gap-2 text-sm text-success">
            <CircleCheck className="size-4" aria-hidden="true" />
            {t.succeeded}
          </p>
        ) : null}
        {failed || error ? (
          <Alert variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertDescription>
              {failed ? failureMessage(flash?.failure ?? null, flash?.wrongProduct) : t.startFailed}
            </AlertDescription>
          </Alert>
        ) : null}

        <AlertDialogFooter>
          {stage === 'done' ? (
            <Button onClick={() => setOpen(false)}>{t.close}</Button>
          ) : stage === 'running' ? (
            <Button disabled>{t.update}</Button>
          ) : (
            <>
              <Button
                variant="outline"
                disabled={pending}
                onClick={required ? () => setOpen(false) : later}
              >
                {required ? t.notNow : t.later}
              </Button>
              <Button disabled={pending} onClick={update}>
                {t.update}
              </Button>
            </>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
