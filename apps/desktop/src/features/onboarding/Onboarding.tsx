import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { Button } from '@kivori/ui/components/button';
import { getStartupSettings, setLaunchAtLogin } from '@/lib/ipc';
import type {
  ConfigDto,
  ConnectionStatusDto,
  DeskStatusDto,
  StartupSettingsDto,
} from '@/lib/ipc/types';
import { uiConnection } from '@/hooks/use-kivori';
import { brandIconUrl } from '@/lib/brand';
import { format, strings } from '@/lib/i18n/strings';
import { cn } from '@kivori/ui/lib/utils';
import { Customize } from './steps/Customize';
import { Done } from './steps/Done';
import { Permissions } from './steps/Permissions';
import { PlugIn } from './steps/PlugIn';
import { Tour } from './steps/Tour';
import { Welcome } from './steps/Welcome';
import { TOUR_STEPS, useAccessibility, useTour } from './use-onboarding';

const t = strings.onboarding;

type StepId = 'welcome' | 'plug' | 'permissions' | 'tour' | 'customize' | 'done';
const ALL_STEPS: readonly StepId[] = [
  'welcome',
  'plug',
  'permissions',
  'tour',
  'customize',
  'done',
];

/**
 * First-run setup: a full-window stepper shown instead of the normal layout until it is finished
 * or skipped. A step moves on only from real status (connected, permission granted, a control seen
 * on the device) or when the person skips it. Launch at login is the one setting held back until
 * Finish.
 */
export function Onboarding({
  connection,
  desk,
  config,
  updateAvailable,
  onComplete,
}: {
  connection: ConnectionStatusDto | null;
  desk: DeskStatusDto | null;
  config: ConfigDto | null;
  updateAvailable: boolean;
  /** Marks setup finished (Finish and Skip setup both call it). Rejects when it cannot be saved. */
  onComplete: () => Promise<void>;
}): ReactElement {
  const [step, setStep] = useState<StepId>('welcome');
  const [busy, setBusy] = useState(false);
  const [finishError, setFinishError] = useState<string | null>(null);
  const [startup, setStartup] = useState<StartupSettingsDto | null>(null);
  // Explicit consent, on by default; nothing is changed until Finish.
  const [launch, setLaunch] = useState(true);

  const accessibility = useAccessibility(step === 'permissions');
  const tour = useTour(desk, step === 'tour');
  const connected = uiConnection(connection) === 'connected';

  useEffect(() => {
    let active = true;
    getStartupSettings()
      .then((value) => {
        if (active) setStartup(value);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  // The permission step is for macOS only: skipped wherever there is no such permission.
  const steps = ALL_STEPS.filter(
    (id) => id !== 'permissions' || accessibility.state !== 'notApplicable',
  );
  const index = steps.indexOf(step);
  const go = (to: number): void => setStep(steps[Math.min(Math.max(to, 0), steps.length - 1)]!);
  useEffect(() => {
    if (step === 'permissions' && accessibility.state === 'notApplicable') setStep('tour');
  }, [step, accessibility.state]);

  const skipSetup = (): void => {
    setBusy(true);
    onComplete().catch(() => {
      setBusy(false);
      toast.error(t.done.failed);
    });
  };

  const finish = async (): Promise<void> => {
    setBusy(true);
    setFinishError(null);
    try {
      if (startup && launch !== startup.launchAtLogin) {
        try {
          await setLaunchAtLogin(launch);
        } catch {
          // Not a reason to keep the person in setup; the Device page can change it.
          toast.error(t.done.launchFailed);
        }
      }
      await onComplete();
    } catch {
      setBusy(false);
      setFinishError(t.done.failed);
    }
  };

  const gate: Record<StepId, { ready: boolean; skippable: boolean }> = {
    welcome: { ready: true, skippable: false },
    plug: { ready: connected, skippable: true },
    permissions: { ready: accessibility.state === 'granted', skippable: true },
    tour: { ready: tour.done >= TOUR_STEPS.length, skippable: true },
    customize: { ready: true, skippable: false },
    done: { ready: true, skippable: false },
  };
  const { ready, skippable } = gate[step];
  const last = step === 'done';

  return (
    <div className="flex min-h-svh flex-col bg-background text-foreground">
      <header className="flex items-center gap-3 border-b px-4 py-3 sm:px-6">
        <img src={brandIconUrl} alt="" draggable={false} className="-m-1 size-10 select-none" />
        <span className="font-semibold tracking-tight">{strings.appTitle}</span>
        <Button variant="ghost" size="sm" className="ml-auto" disabled={busy} onClick={skipSetup}>
          {t.skipSetup}
        </Button>
      </header>

      <main className="mx-auto flex w-full max-w-2xl flex-1 flex-col justify-center gap-6 px-4 py-8 sm:px-6">
        <div className="flex items-center gap-3" data-testid="onboarding-progress">
          <ol aria-hidden="true" className="flex gap-1.5">
            {steps.map((id, position) => (
              <li
                key={id}
                className={cn(
                  'h-1.5 w-8 rounded-full bg-muted transition-colors',
                  position <= index && 'bg-primary',
                )}
              />
            ))}
          </ol>
          <span className="text-xs text-muted-foreground">
            {format(t.progress, { current: index + 1, total: steps.length })}
          </span>
        </div>

        {step === 'welcome' ? (
          <Welcome />
        ) : step === 'plug' ? (
          <PlugIn connection={connection} updateAvailable={updateAvailable} />
        ) : step === 'permissions' ? (
          <Permissions state={accessibility.state} onChange={accessibility.refresh} />
        ) : step === 'tour' ? (
          <Tour desk={desk} done={tour.done} connected={connected} />
        ) : step === 'customize' ? (
          <Customize
            config={config}
            startup={startup}
            launchAtLogin={launch}
            onLaunchAtLogin={setLaunch}
          />
        ) : (
          <Done error={finishError} />
        )}

        <div className="flex flex-wrap items-center gap-2">
          {index > 0 ? (
            <Button variant="ghost" disabled={busy} onClick={() => go(index - 1)}>
              {t.back}
            </Button>
          ) : null}
          <div className="ml-auto flex flex-wrap items-center gap-2">
            {skippable && !ready ? (
              <Button variant="outline" disabled={busy} onClick={() => go(index + 1)}>
                {step === 'tour' ? t.tour.skipTour : t.skipStep}
              </Button>
            ) : null}
            {last ? (
              <Button disabled={busy} onClick={() => void finish()}>
                {busy ? t.done.finishing : t.done.finish}
              </Button>
            ) : (
              <Button disabled={busy || !ready} onClick={() => go(index + 1)}>
                {step === 'welcome' ? t.welcome.start : t.continue}
              </Button>
            )}
          </div>
        </div>
      </main>
    </div>
  );
}
