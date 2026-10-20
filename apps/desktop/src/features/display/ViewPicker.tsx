import type { ReactElement, ReactNode } from 'react';
import { Sun } from 'lucide-react';
import { RadioGroup, RadioGroupItem } from '@kivori/ui/components/radio-group';
import { cn } from '@kivori/ui/lib/utils';
import { CycleScreens, DeviceScreen } from '@/components/kivori/device-art';
import { BRIGHTNESS_STEPS, DISPLAY_MODES } from '@/lib/ipc/types';
import type { DisplayMode } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';

const t = strings.display;

export type ViewChoice = DisplayMode | 'cycle';

/**
 * Option cards for a saved view: a small illustration of each screen plus its name. A radio
 * group, so one choice is selected and the arrow keys move between them.
 */
export function ViewPicker({
  labelledBy,
  value,
  onChange,
  disabled,
  disabledViews = [],
  withCycle = false,
}: {
  labelledBy: string;
  value: ViewChoice | null;
  onChange: (view: ViewChoice) => void;
  disabled: boolean;
  disabledViews?: readonly DisplayMode[];
  withCycle?: boolean;
}): ReactElement {
  const options: ViewChoice[] = withCycle ? [...DISPLAY_MODES, 'cycle'] : [...DISPLAY_MODES];
  return (
    <RadioGroup
      aria-labelledby={labelledBy}
      value={value}
      disabled={disabled}
      onValueChange={(next) => onChange(next as ViewChoice)}
      className="grid-cols-[repeat(auto-fill,minmax(6.25rem,1fr))] gap-2.5"
    >
      {options.map((option) => {
        const off = option !== 'cycle' && disabledViews.includes(option);
        return (
          <OptionCard
            key={option}
            value={option}
            selected={value === option}
            disabled={off || disabled}
            preview={
              option === 'cycle' ? (
                <CycleScreens className="w-full" />
              ) : (
                <DeviceScreen mode={option} className="w-full" />
              )
            }
            name={option === 'cycle' ? strings.display.doublePress.cycle : t.modes[option].name}
          />
        );
      })}
    </RadioGroup>
  );
}

function OptionCard({
  value,
  selected,
  disabled,
  preview,
  name,
}: {
  value: string;
  selected: boolean;
  disabled: boolean;
  preview: ReactNode;
  name: string;
}): ReactElement {
  return (
    <label
      data-selected={selected}
      className={cn(
        'relative flex flex-col gap-2 rounded-xl bg-card p-2 ring-1 ring-foreground/10 transition-all has-focus-visible:ring-2 has-focus-visible:ring-ring',
        disabled
          ? 'cursor-not-allowed opacity-50 grayscale'
          : 'cursor-pointer hover:-translate-y-0.5 hover:shadow-md',
        selected && 'bg-primary/5 ring-2 ring-primary shadow-md shadow-primary/10',
      )}
    >
      <div aria-hidden="true" className="px-1">
        {preview}
      </div>
      <div className="flex items-center justify-between gap-1.5 px-1 pb-0.5">
        <span className="min-w-0 text-sm leading-tight font-medium">{name}</span>
        <RadioGroupItem value={value} disabled={disabled} />
      </div>
    </label>
  );
}

/** Brightness as five segments of growing sun: everything up to the chosen step is lit. */
export function BrightnessPicker({
  labelledBy,
  value,
  onChange,
  disabled,
}: {
  labelledBy: string;
  value: number | null;
  onChange: (step: number) => void;
  disabled: boolean;
}): ReactElement {
  return (
    <RadioGroup
      aria-labelledby={labelledBy}
      value={value === null ? null : String(value)}
      disabled={disabled}
      onValueChange={(next) => onChange(Number(next))}
      className="grid-cols-5 gap-1 rounded-xl bg-muted p-1 sm:max-w-md"
    >
      {Array.from({ length: BRIGHTNESS_STEPS }, (_, i) => i + 1).map((step) => {
        const lit = value !== null && step <= value;
        return (
          <label
            key={step}
            data-lit={lit}
            className={cn(
              'flex h-11 items-center justify-center rounded-lg transition-colors has-focus-visible:ring-2 has-focus-visible:ring-ring',
              disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer hover:bg-background/60',
              lit ? 'bg-primary/15 text-primary' : 'text-muted-foreground',
              value === step && 'bg-primary/25 ring-2 ring-primary',
            )}
          >
            <Sun
              aria-hidden="true"
              style={{ width: 12 + step * 3, height: 12 + step * 3 }}
              className={cn(!lit && 'opacity-60')}
            />
            <span className="sr-only">{t.brightness.step(step)}</span>
            <RadioGroupItem value={String(step)} className="sr-only" />
          </label>
        );
      })}
    </RadioGroup>
  );
}
