import { useEffect, useState } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import { toast } from 'sonner';
import { Label } from '@kivori/ui/components/label';
import { Switch } from '@kivori/ui/components/switch';
import { ToggleGroup, ToggleGroupItem } from '@kivori/ui/components/toggle-group';
import { cn } from '@kivori/ui/lib/utils';
import { setBuddySettings, setDisplaySettings } from '@/lib/ipc';
import { mascotUrl } from '@/lib/brand';
import { DISPLAY_MODES, INTENSITIES, INTENSITY_PERSONALITY } from '@/lib/ipc/types';
import type {
  ConfigDto,
  ConnectionStatusDto,
  DisplayMode,
  Intensity,
  StartupSettingsDto,
} from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { controlSupported, hasCapability } from '@/lib/device-support';
import { GnomeFocusHelp } from '../../permissions/GnomeFocusHelp';
import { StepFrame } from './frame';
import './onboarding-motion.css';

const MASCOT_MOTION = {
  calm: 'motion-safe:animate-[onboarding-breathe_4s_ease-in-out_infinite]',
  cozy: 'motion-safe:animate-[onboarding-bob_2.4s_ease-in-out_infinite]',
  playful: 'motion-safe:animate-[onboarding-playful_0.9s_ease-in-out_infinite]',
} as const;

const t = strings.onboarding.customize;

/**
 * Explains Auto profiles and the pin, and sets the screen at start and the buddy's personality.
 * Those two are saved as soon as they are chosen (the same commands as the Display page). Launch
 * at login is only a choice here: it is applied when the person finishes setup.
 */
export function Customize({
  connection,
  config,
  startup,
  launchAtLogin,
  onLaunchAtLogin,
}: {
  connection: ConnectionStatusDto | null;
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
      ...display,
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
  const moveIntensity = (event: KeyboardEvent<HTMLButtonElement>, index: number): void => {
    const direction =
      event.key === 'ArrowRight' || event.key === 'ArrowDown'
        ? 1
        : event.key === 'ArrowLeft' || event.key === 'ArrowUp'
          ? -1
          : 0;
    if (direction === 0) return;
    event.preventDefault();
    const next = INTENSITIES[(index + direction + INTENSITIES.length) % INTENSITIES.length]!;
    chooseIntensity(next);
    event.currentTarget.parentElement
      ?.querySelector<HTMLButtonElement>(`[data-testid="personality-${next}"]`)
      ?.focus();
  };

  return (
    <StepFrame title={t.title} body={t.body}>
      <p className="text-sm text-muted-foreground">{strings.savedConfiguration}</p>
      <div className="space-y-1 rounded-lg bg-muted/60 px-4 py-3">
        <h2 className="text-sm font-semibold">{t.autoHeading}</h2>
        <p className="text-sm text-muted-foreground">{t.autoBody}</p>
        {controlSupported(connection, 'button2Hold') ? (
          <p className="text-sm text-muted-foreground">{t.pinBody}</p>
        ) : null}
      </div>

      <GnomeFocusHelp />

      <div className="space-y-2">
        <div className="space-y-0.5">
          <h2 id="onboarding-view" className="text-sm font-semibold">
            {t.viewHeading}
          </h2>
          <p className="text-xs text-muted-foreground">{t.viewBody}</p>
          {!hasCapability(connection, 'deskStatusV1') ? (
            <p className="text-xs text-muted-foreground" role="note">
              {strings.savedOnly}
            </p>
          ) : null}
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
          {!(
            hasCapability(connection, 'idleLifeV1') ||
            hasCapability(connection, 'mascotInteraction')
          ) ? (
            <p className="text-xs text-muted-foreground" role="note">
              {strings.savedOnly}
            </p>
          ) : null}
        </div>
        <div
          role="radiogroup"
          aria-labelledby="onboarding-intensity"
          className="grid grid-cols-1 gap-3 sm:grid-cols-3"
        >
          {INTENSITIES.map((level, index) => {
            const personality = INTENSITY_PERSONALITY[level];
            const checked = shownIntensity === level;
            return (
              <button
                key={level}
                type="button"
                role="radio"
                aria-checked={checked}
                data-testid={`personality-${level}`}
                disabled={buddy === null}
                tabIndex={checked ? 0 : -1}
                onClick={() => chooseIntensity(level)}
                onKeyDown={(event) => moveIntensity(event, index)}
                className={cn(
                  'flex flex-col items-start gap-2 rounded-lg border p-4 text-left transition-colors',
                  checked && 'border-primary bg-primary/10',
                )}
              >
                <img
                  src={mascotUrl}
                  alt=""
                  draggable={false}
                  className={cn('size-14 select-none', MASCOT_MOTION[personality])}
                />
                <span className="text-sm font-medium">{t.personalities[personality]}</span>
                <span className="text-xs text-muted-foreground">{t.personalityBody[level]}</span>
              </button>
            );
          })}
        </div>
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
