import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { LoaderCircle, MonitorSmartphone } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
import { Skeleton } from '@/components/ui/skeleton';
import { DeviceScreen } from '@/components/kivori/device-art';
import { Page } from '@/components/kivori/page';
import { setDisplayMode } from '@/lib/ipc';
import { DISPLAY_MODES } from '@/lib/ipc/types';
import type { ConnectionStatusDto, DeskStatusDto, DisplayMode } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { cn, errorText } from '@/lib/utils';
import { CompanionControls } from './CompanionControls';

const t = strings.display;

/// How long a pending selection may wait for desk://status before the UI falls back to the truth.
export const PENDING_TIMEOUT_MS = 4000;

/**
 * Display: choose the device view. The highlight moves at once (optimistic, UI only); what the
 * device shows still comes from desk://status, and a rejection reverts the highlight with a toast.
 */
export function DisplayPage({
  desk,
  connection,
}: {
  desk: DeskStatusDto | null;
  connection: ConnectionStatusDto | null;
}): ReactElement {
  const [pending, setPending] = useState<DisplayMode | null>(null);

  // The real mode arrived (or the wait timed out): drop the optimistic highlight.
  useEffect(() => {
    if (pending === null) return;
    if (desk?.mode === pending) {
      setPending(null);
      return;
    }
    const timer = setTimeout(() => setPending(null), PENDING_TIMEOUT_MS);
    return () => clearTimeout(timer);
  }, [pending, desk?.mode]);

  const choose = (mode: DisplayMode): void => {
    if (mode === (pending ?? desk?.mode)) return;
    setPending(mode);
    void setDisplayMode(mode).catch((error: unknown) => {
      setPending(null);
      toast.error(t.failed, { description: errorText(error) });
    });
  };

  const selected = pending ?? desk?.mode ?? null;

  return (
    <Page title={t.title} description={t.description}>
      <section aria-labelledby="view-heading" className="space-y-3">
        <h2 id="view-heading" className="flex items-center gap-2 text-base font-semibold">
          <MonitorSmartphone className="size-4 text-muted-foreground" aria-hidden="true" />
          {t.view}
        </h2>
        {desk ? (
          <RadioGroup
            aria-labelledby="view-heading"
            value={selected}
            onValueChange={(value) => choose(value as DisplayMode)}
            className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5"
          >
            {DISPLAY_MODES.map((mode) => {
              const isSelected = selected === mode;
              const isPending = pending === mode;
              const onDevice = desk.mode === mode;
              return (
                <label
                  key={mode}
                  data-mode={mode}
                  data-selected={isSelected}
                  className={cn(
                    'group relative flex cursor-pointer flex-col gap-3 rounded-xl bg-card p-3 ring-1 ring-foreground/10 transition-all hover:-translate-y-0.5 hover:shadow-md has-focus-visible:ring-2 has-focus-visible:ring-ring',
                    isSelected && 'ring-2 ring-primary shadow-md shadow-primary/10',
                  )}
                >
                  <div aria-hidden="true">
                    <DeviceScreen
                      mode={mode}
                      className={cn(
                        'w-full transition-opacity',
                        !isSelected && 'opacity-80 group-hover:opacity-100',
                      )}
                    />
                  </div>
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0 space-y-0.5">
                      <span className="block text-sm font-medium">{t.modes[mode].name}</span>
                      <span className="block text-xs text-muted-foreground">
                        {t.modes[mode].body}
                      </span>
                    </div>
                    <RadioGroupItem value={mode} className="mt-0.5" />
                  </div>
                  {isPending ? (
                    <Badge
                      variant="secondary"
                      className="absolute top-5 left-5 h-6 gap-1.5 px-2.5"
                      role="status"
                    >
                      <LoaderCircle
                        data-icon="inline-start"
                        className="animate-spin"
                        aria-hidden="true"
                      />
                      {t.applying}
                    </Badge>
                  ) : onDevice ? (
                    <Badge className="absolute top-5 left-5 h-6 px-2.5" data-testid="on-device">
                      {t.current}
                    </Badge>
                  ) : null}
                </label>
              );
            })}
          </RadioGroup>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5">
            {DISPLAY_MODES.map((mode) => (
              <Skeleton key={mode} className="aspect-[0.8] rounded-xl" />
            ))}
          </div>
        )}
      </section>

      <CompanionControls
        connected={connection?.connection === 'connected'}
        supported={connection?.mascotInteraction ?? false}
      />
    </Page>
  );
}
