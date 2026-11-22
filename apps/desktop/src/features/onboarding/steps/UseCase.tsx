import { useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { Check, Clapperboard, Coffee, Palette, Terminal } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { toast } from 'sonner';
import { cn } from '@kivori/ui/lib/utils';
import { setProfilePinCycle } from '@/lib/ipc';
import { PROFILE_LIMITS } from '@/lib/ipc/types';
import type { BuiltinProfileId, ConfigDto } from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { StepFrame } from './frame';

const t = strings.onboarding.usecase;

/** Work groups and the built-in profiles each one puts on the device. */
export const USE_CASES: readonly {
  key: keyof typeof t.cases;
  name: string;
  icon: LucideIcon;
  profiles: readonly BuiltinProfileId[];
}[] = [
  {
    key: 'video',
    name: t.cases.video,
    icon: Clapperboard,
    profiles: ['capcut', 'premiere', 'aftereffects'],
  },
  {
    key: 'design',
    name: t.cases.design,
    icon: Palette,
    profiles: ['photoshop', 'blender'],
  },
  {
    key: 'coding',
    name: t.cases.coding,
    icon: Terminal,
    profiles: ['code', 'claudecode', 'codex', 'opencode'],
  },
  {
    key: 'everyday',
    name: t.cases.everyday,
    icon: Coffee,
    profiles: ['browser', 'media', 'zoom', 'teams'],
  },
];

function isOn(config: ConfigDto, id: string): boolean {
  return config.profiles.some((profile) => profile.id === id && profile.inPinCycle);
}

/**
 * Chooses which built-in profiles start on the device. A card is on only when every one of its
 * profiles is; the saved config is the only selection state.
 */
export function UseCase({ config }: { config: ConfigDto | null }): ReactElement {
  const running = useRef(false);
  const [busy, setBusy] = useState(false);

  const toggle = async (useCase: (typeof USE_CASES)[number]): Promise<void> => {
    if (!config || running.current) return;
    const selected = useCase.profiles.every((id) => isOn(config, id));
    const ids = selected
      ? [...useCase.profiles]
      : useCase.profiles.filter((id) => !isOn(config, id));
    const pinned = config.profiles.filter((profile) => profile.inPinCycle).length;
    if (!selected && pinned + ids.length > PROFILE_LIMITS.pinned) {
      toast.error(format(t.limit, { max: PROFILE_LIMITS.pinned }));
      return;
    }
    running.current = true;
    setBusy(true);
    try {
      for (const id of ids) await setProfilePinCycle(id, !selected);
    } catch {
      toast.error(t.failed);
    } finally {
      running.current = false;
      setBusy(false);
    }
  };

  const onDevice = config?.profiles.filter((profile) => profile.inPinCycle) ?? [];

  return (
    <StepFrame title={t.title} body={t.body}>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {USE_CASES.map((useCase) => {
          const selected = config !== null && useCase.profiles.every((id) => isOn(config, id));
          const Icon = useCase.icon;
          const names =
            config === null
              ? ''
              : useCase.profiles
                  .flatMap((id) => {
                    const name = config.profiles.find((profile) => profile.id === id)?.name;
                    return name ? [name] : [];
                  })
                  .join(', ');
          return (
            <button
              key={useCase.key}
              type="button"
              data-testid={`usecase-${useCase.key}`}
              aria-pressed={selected}
              disabled={config === null || busy}
              onClick={() => void toggle(useCase)}
              className={cn(
                'flex flex-col items-start gap-2 rounded-lg border p-4 text-left transition-colors motion-safe:animate-in motion-safe:fade-in disabled:opacity-50',
                selected && 'border-primary bg-primary/10',
              )}
            >
              <span className="flex w-full items-center gap-2">
                <Icon className="size-5 shrink-0" aria-hidden="true" />
                <span className="font-medium">{useCase.name}</span>
                {selected ? <Check className="ml-auto size-4 shrink-0" aria-hidden="true" /> : null}
              </span>
              <span className="text-sm text-muted-foreground">{names}</span>
            </button>
          );
        })}
      </div>
      <p className="text-sm text-muted-foreground">
        {onDevice.length === 0
          ? t.none
          : format(t.onDevice, { names: onDevice.map((profile) => profile.name).join(', ') })}
      </p>
    </StepFrame>
  );
}
