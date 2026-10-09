import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { LoaderCircle, MonitorSmartphone } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
import { Skeleton } from '@/components/ui/skeleton';
import { DeviceScreen } from '@/components/kivori/device-art';
import { Page } from '@/components/kivori/page';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { setDisplayMode, setDisplaySettings } from '@/lib/ipc';
import { DISPLAY_MODES } from '@/lib/ipc/types';
import type { ConfigDto, ConnectionStatusDto, DeskStatusDto, DisplayMode } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { cn, errorText } from '@/lib/utils';
import { CompanionControls } from './CompanionControls';

const t = strings.display;

/// How long a pending selection may wait for desk://status before the UI falls back to the truth.
export const PENDING_TIMEOUT_MS = 4000;

type DisplaySettings = ConfigDto['display'];

/**
 * Display: the saved default and double-press views, and "show now" for the device view. The
 * highlight moves at once (optimistic, UI only); what the device shows still comes from
 * desk://status, saved settings from the config, and a rejection reverts with a toast.
 */
export function DisplayPage({
  desk,
  config,
  connection,
}: {
  desk: DeskStatusDto | null;
  config: ConfigDto | null;
  connection: ConnectionStatusDto | null;
}): ReactElement {
  const [pending, setPending] = useState<DisplayMode | null>(null);
  const [pendingSettings, setPendingSettings] = useState<DisplaySettings | null>(null);

  // The saved config arrived: it is the truth again.
  const revision = config?.revision;
  useEffect(() => setPendingSettings(null), [revision]);

  const settings = pendingSettings ?? config?.display ?? null;
  const saveSettings = (next: DisplaySettings): void => {
    setPendingSettings(next);
    void setDisplaySettings(next.defaultView, next.secondaryView).catch((error: unknown) => {
      setPendingSettings(null);
      toast.error(t.settingsFailed, { description: errorText(error) });
    });
  };
  const chooseDefault = (view: DisplayMode): void => {
    if (!settings || view === settings.defaultView) return;
    // The double-press view must differ from the default, so picking it swaps the two.
    saveSettings({
      defaultView: view,
      secondaryView:
        settings.secondaryView === view ? settings.defaultView : settings.secondaryView,
    });
  };
  const chooseSecondary = (view: DisplayMode | 'cycle'): void => {
    if (settings && view !== settings.secondaryView)
      saveSettings({ ...settings, secondaryView: view });
  };

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
      <section aria-labelledby="default-view-heading" className="space-y-3">
        <div className="space-y-0.5">
          <h2 id="default-view-heading" className="text-base font-semibold">
            {t.defaultView.heading}
          </h2>
          <p className="text-sm text-muted-foreground">{t.defaultView.body}</p>
        </div>
        <ToggleGroup
          aria-labelledby="default-view-heading"
          value={settings ? [settings.defaultView] : []}
          disabled={settings === null}
          onValueChange={(value) => {
            const selected = value[0] as DisplayMode | undefined;
            if (selected) chooseDefault(selected);
          }}
          variant="outline"
          className="flex-wrap"
        >
          {DISPLAY_MODES.map((mode) => (
            <ToggleGroupItem key={mode} value={mode} className="px-4">
              {t.modes[mode].name}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </section>

      <section aria-labelledby="double-press-heading" className="space-y-3">
        <div className="space-y-0.5">
          <h2 id="double-press-heading" className="text-base font-semibold">
            {t.doublePress.heading}
          </h2>
          <p className="text-sm text-muted-foreground">{t.doublePress.body}</p>
        </div>
        <ToggleGroup
          aria-labelledby="double-press-heading"
          value={settings ? [settings.secondaryView] : []}
          disabled={settings === null}
          onValueChange={(value) => {
            const selected = value[0] as DisplayMode | 'cycle' | undefined;
            if (selected) chooseSecondary(selected);
          }}
          variant="outline"
          className="flex-wrap"
        >
          {DISPLAY_MODES.map((mode) => (
            <ToggleGroupItem
              key={mode}
              value={mode}
              disabled={mode === settings?.defaultView}
              className="px-4"
            >
              {t.modes[mode].name}
            </ToggleGroupItem>
          ))}
          <ToggleGroupItem value="cycle" className="px-4">
            {t.doublePress.cycle}
          </ToggleGroupItem>
        </ToggleGroup>
      </section>

      <section aria-labelledby="view-heading" className="space-y-3">
        <div className="space-y-0.5">
          <h2 id="view-heading" className="flex items-center gap-2 text-base font-semibold">
            <MonitorSmartphone className="size-4 text-muted-foreground" aria-hidden="true" />
            {t.view}
          </h2>
          <p className="text-sm text-muted-foreground">{t.viewBody}</p>
        </div>
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
        config={config}
        connected={connection?.connection === 'connected'}
        supported={connection?.mascotInteraction ?? false}
      />
    </Page>
  );
}
