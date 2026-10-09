import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { setBuddySettings, setDisplaySettings } from '@/lib/ipc';
import { DISPLAY_MODES, INTENSITIES } from '@/lib/ipc/types';
import type { ConfigDto, DisplayMode, Intensity, StartupSettingsDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { StepFrame } from './frame';

const t = strings.onboarding.customize;

/**
 * Explains Auto profiles and the pin, and sets the screen at start and the buddy's liveliness.
 * Those two are saved as soon as they are chosen (the same commands as the Display page). Launch
 * at login is only a choice here: it is applied when the person finishes setup.
 */
export function Customize({
  config,
  startup,
  launchAtLogin,
  onLaunchAtLogin,
}: {
  config: ConfigDto | null;
  startup: StartupSettingsDto | null;
  launchAtLogin: boolean;
  onLaunchAtLogin: (enabled: boolean) => void;
}): ReactElement {
  const [view, setView] = useState<ConfigDto['display'] | null>(null);
  const [intensity, setIntensity] = useState<Intensity | null>(null);

  // The saved config arrived: it is the truth again.
  const revision = config?.revision;
  useEffect(() => {
    setView(null);
    setIntensity(null);
  }, [revision]);

  const display = view ?? config?.display ?? null;
  const buddy = config?.buddy ?? null;
  const shownIntensity = intensity ?? buddy?.intensity ?? null;

  const chooseView = (next: DisplayMode): void => {
    if (!display || next === display.defaultView) return;
    // The double-press view must differ from the default, so picking it swaps the two.
    const settings = {
      defaultView: next,
      secondaryView: display.secondaryView === next ? display.defaultView : display.secondaryView,
    };
    setView(settings);
    void setDisplaySettings(settings.defaultView, settings.secondaryView).catch(() => {
      setView(null);
      toast.error(t.saveFailed);
    });
  };
  const chooseIntensity = (next: Intensity): void => {
    if (!buddy || next === shownIntensity) return;
    setIntensity(next);
    void setBuddySettings(buddy.reactions, next).catch(() => {
      setIntensity(null);
      toast.error(t.saveFailed);
    });
  };

  return (
    <StepFrame title={t.title} body={t.body}>
      <div className="space-y-1 rounded-lg bg-muted/60 px-4 py-3">
        <h2 className="text-sm font-semibold">{t.autoHeading}</h2>
        <p className="text-sm text-muted-foreground">{t.autoBody}</p>
      </div>

      <div className="space-y-2">
        <div className="space-y-0.5">
          <h2 id="onboarding-view" className="text-sm font-semibold">
            {t.viewHeading}
          </h2>
          <p className="text-xs text-muted-foreground">{t.viewBody}</p>
        </div>
        <ToggleGroup
          aria-labelledby="onboarding-view"
          value={display ? [display.defaultView] : []}
          disabled={display === null}
          onValueChange={(value) => {
            const picked = value[0] as DisplayMode | undefined;
            if (picked) chooseView(picked);
          }}
          variant="outline"
          className="flex-wrap"
        >
          {DISPLAY_MODES.map((mode) => (
            <ToggleGroupItem key={mode} value={mode} className="px-4">
              {strings.display.modes[mode].name}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>

      <div className="space-y-2">
        <div className="space-y-0.5">
          <h2 id="onboarding-intensity" className="text-sm font-semibold">
            {t.intensityHeading}
          </h2>
          <p className="text-xs text-muted-foreground">{t.intensityBody}</p>
        </div>
        <ToggleGroup
          aria-labelledby="onboarding-intensity"
          value={shownIntensity ? [shownIntensity] : []}
          disabled={buddy === null}
          onValueChange={(value) => {
            const picked = value[0] as Intensity | undefined;
            if (picked) chooseIntensity(picked);
          }}
          variant="outline"
        >
          {INTENSITIES.map((level) => (
            <ToggleGroupItem key={level} value={level} className="px-4">
              {strings.companion.intensities[level]}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>

      <div className="flex items-center justify-between gap-4 rounded-lg border px-4 py-3">
        <div className="space-y-0.5">
          <Label htmlFor="onboarding-launch">{t.launchLabel[startup?.platform ?? 'other']}</Label>
          <p id="onboarding-launch-hint" className="text-xs text-muted-foreground">
            {t.launchBody} {t.launchNote}
          </p>
        </div>
        <Switch
          id="onboarding-launch"
          aria-describedby="onboarding-launch-hint"
          checked={launchAtLogin}
          onCheckedChange={onLaunchAtLogin}
        />
      </div>
    </StepFrame>
  );
}
