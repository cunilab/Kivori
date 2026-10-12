import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { TriangleAlert } from 'lucide-react';
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
import { useUpdateStatus } from '@/hooks/use-kivori';
import { installUpdate, onUpdateDialogRequested, skipUpdateVersion, snoozeUpdate } from '@/lib/ipc';
import { format, strings } from '@/lib/i18n/strings';
import { onUpdateDialogRequest } from './dialog-request';

const t = strings.updates.dialog;

// Versions whose dialog already opened by itself in this window. It opens once per version; the
// tray item and the Device page's "View update" can always reopen it.
const shown = new Set<string>();

/** Tests only. */
export function resetUpdateDialog(): void {
  shown.clear();
}

/** "1.2 MB": a download size for people, not a byte count. */
export function megabytes(bytes: number): string {
  return `${(bytes / 1_000_000).toFixed(1)} MB`;
}

function releaseDate(iso: string | null): string | null {
  if (!iso) return null;
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? null : date.toLocaleDateString();
}

/**
 * The app update offer (ADR-0015): "Kivori X.Y.Z is ready", the release notes as plain text, and
 * Install & restart / Later / Skip this version. The native side only offers an update when no
 * firmware flash or knob gesture is running, so opening when it becomes available is safe.
 */
export function UpdateDialog(): ReactElement | null {
  const status = useUpdateStatus();
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState(false);
  const statusRef = useRef(status);
  statusRef.current = status;

  const version = status?.update?.version ?? null;
  const state = status?.state;

  useEffect(() => {
    if (state === 'available' && version && !shown.has(version)) {
      shown.add(version);
      setOpen(true);
    }
  }, [state, version]);

  // Later or Skip from anywhere withdraws the offer: nothing left to show.
  useEffect(() => {
    if (!version) setOpen(false);
  }, [version]);

  useEffect(() => {
    const reopen = (): void => {
      if (statusRef.current?.update) setOpen(true);
    };
    const stopLocal = onUpdateDialogRequest(reopen);
    let stopNative: () => void = () => {};
    let active = true;
    void onUpdateDialogRequested(reopen)
      .then((stop) => {
        if (active) stopNative = stop;
        else stop();
      })
      .catch(() => {});
    return () => {
      active = false;
      stopLocal();
      stopNative();
    };
  }, []);

  if (!status?.update) return null;
  const release = status.update;
  const installing = state === 'downloading' || state === 'readyToRestart';
  const failed = state === 'failed';

  const run = (action: () => Promise<unknown>, close: boolean): void => {
    setPending(true);
    setError(false);
    action()
      .then(() => {
        if (close) setOpen(false);
      })
      .catch(() => setError(true))
      .finally(() => setPending(false));
  };

  const progress = status.progress;
  const published = releaseDate(release.date);

  return (
    <AlertDialog open={open} onOpenChange={(next) => !installing && setOpen(next)}>
      <AlertDialogContent className="data-[size=default]:sm:max-w-md">
        <AlertDialogHeader>
          <AlertDialogTitle>{format(t.title, { version: release.version })}</AlertDialogTitle>
          <AlertDialogDescription>
            {format(t.current, { version: status.currentVersion })}
            {published ? ` ${format(t.published, { date: published })}` : null}
          </AlertDialogDescription>
        </AlertDialogHeader>

        <section aria-label={t.notes} className="grid gap-1.5">
          <h3 className="text-sm font-medium">{t.notes}</h3>
          {/* Plain text on purpose: release notes are never rendered as HTML. */}
          <p
            data-testid="release-notes"
            className="max-h-48 overflow-y-auto rounded-md border bg-muted/30 p-3 text-xs break-words whitespace-pre-wrap text-muted-foreground"
          >
            {release.notes || t.noNotes}
          </p>
        </section>

        {state === 'downloading' ? (
          <Progress
            aria-label={t.downloading}
            value={progress?.total ? (progress.downloaded / progress.total) * 100 : null}
          >
            <span className="text-xs text-muted-foreground" role="status">
              {progress?.total
                ? format(t.downloadingOf, {
                    done: megabytes(progress.downloaded),
                    total: megabytes(progress.total),
                  })
                : t.downloading}
            </span>
          </Progress>
        ) : null}
        {state === 'readyToRestart' ? (
          <p role="status" className="text-xs text-muted-foreground">
            {t.restarting}
          </p>
        ) : null}
        {failed || error ? (
          <Alert variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertDescription>{t.failed}</AlertDescription>
          </Alert>
        ) : null}

        <AlertDialogFooter>
          {failed ? (
            <>
              <Button variant="outline" disabled={pending} onClick={() => run(snoozeUpdate, true)}>
                {t.later}
              </Button>
              <Button disabled={pending} onClick={() => run(installUpdate, false)}>
                {t.retry}
              </Button>
            </>
          ) : installing ? (
            <Button disabled>{t.install}</Button>
          ) : (
            <>
              <Button
                variant="ghost"
                disabled={pending}
                onClick={() => run(skipUpdateVersion, true)}
              >
                {t.skip}
              </Button>
              <Button variant="outline" disabled={pending} onClick={() => run(snoozeUpdate, true)}>
                {t.later}
              </Button>
              <Button disabled={pending} onClick={() => run(installUpdate, false)}>
                {t.install}
              </Button>
            </>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
