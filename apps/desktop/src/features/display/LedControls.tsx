import { useEffect, useRef, useState } from 'react';
import type { CSSProperties, ReactElement } from 'react';
import { toast } from 'sonner';
import { Pipette, X } from 'lucide-react';
import { RadioGroup, RadioGroupItem } from '@kivori/ui/components/radio-group';
import { Slider } from '@kivori/ui/components/slider';
import { cn } from '@kivori/ui/lib/utils';
import { setLedSettings } from '@/lib/ipc';
import { LED_MODES, LED_SPEEDS } from '@/lib/ipc/types';
import type { ConfigDto, LedMode, LedSettingsDto, LedSpeed } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';

const t = strings.display.led;

/** The preset colors; any other comes from the custom picker. */
export const LED_SWATCHES = [
  '#2EC4B6',
  '#3A86FF',
  '#8338EC',
  '#FF4FA3',
  '#FF3B30',
  '#FF8A00',
  '#FFD60A',
  '#34C759',
  '#FFFFFF',
] as const;

// The device's own periods (its LED engine), so the preview moves like the light.
const BREATHE_MS: Record<LedSpeed, number> = { slow: 6000, medium: 4000, fast: 2000 };
const CYCLE_MS: Record<LedSpeed, number> = { slow: 20000, medium: 10000, fast: 5000 };
const USES_COLOR: readonly LedMode[] = ['solid', 'breathe', 'status'];
const USES_SPEED: readonly LedMode[] = ['breathe', 'cycle'];
// Status signals as the device shows them: muted mic red, no host amber, a press white.
const CUES = [
  ['muted', '#FF0000'],
  ['offline', '#FFA000'],
  ['press', '#FFFFFF'],
] as const;

/** A lit LED: `mode` decides how it moves. Pure CSS, so it costs nothing while idle. */
function LedDot({
  mode,
  color,
  speed,
  brightness = 100,
  className,
}: {
  mode: LedMode;
  color: string;
  speed: LedSpeed;
  brightness?: number;
  className?: string;
}): ReactElement {
  const glow = mode === 'off' ? 'transparent' : color;
  const style: CSSProperties = {
    backgroundColor: mode === 'off' ? undefined : mode === 'cycle' ? '#FF3B30' : color,
    boxShadow: mode === 'off' ? undefined : `0 0 12px 2px ${glow}`,
    opacity: mode === 'off' ? undefined : Math.max(0.25, brightness / 100),
  };
  const motion: CSSProperties | undefined =
    mode === 'breathe'
      ? { animation: `led-breathe ${BREATHE_MS[speed]}ms ease-in-out infinite` }
      : mode === 'cycle'
        ? { animation: `led-cycle ${CYCLE_MS[speed]}ms linear infinite` }
        : mode === 'mood'
          ? { animation: 'led-mood 8s ease-in-out infinite', boxShadow: undefined }
          : undefined;
  return (
    <span
      aria-hidden="true"
      data-led-motion={motion ? '' : undefined}
      className={cn(
        'inline-block rounded-full',
        mode === 'off' && 'bg-muted ring-1 ring-foreground/15 ring-inset',
        className,
      )}
      style={{ ...style, ...motion }}
    />
  );
}

/**
 * The native color input. React's `onChange` fires on every drag step inside the picker; this
 * previews those (`onPreview`) and saves only on the DOM `change` event, when the picker commits.
 */
function NativeColor({
  label,
  value,
  onPreview,
  onCommit,
}: {
  label: string;
  value: string;
  onPreview: (hex: string | null) => void;
  onCommit: (hex: string) => void;
}): ReactElement {
  const ref = useRef<HTMLInputElement>(null);
  const commit = useRef(onCommit);
  commit.current = onCommit;
  useEffect(() => {
    const input = ref.current;
    if (!input) return;
    const changed = (): void => commit.current(input.value.toUpperCase());
    input.addEventListener('change', changed);
    return () => input.removeEventListener('change', changed);
  }, []);
  return (
    <input
      ref={ref}
      type="color"
      aria-label={label}
      value={value.toLowerCase()}
      onChange={(event) => onPreview(event.target.value.toUpperCase())}
      onBlur={() => onPreview(null)}
      className="absolute inset-0 cursor-pointer opacity-0"
    />
  );
}

/** A round color choice: the given swatches plus one native picker for anything else. */
function ColorPicker({
  labelledBy,
  value,
  onChange,
}: {
  labelledBy: string;
  value: string;
  onChange: (hex: string) => void;
}): ReactElement {
  const [draft, setDraft] = useState<string | null>(null);
  const shown = draft ?? value;
  const custom = !LED_SWATCHES.some((swatch) => swatch === shown);
  return (
    <div className="flex flex-wrap items-center gap-2">
      <RadioGroup
        aria-labelledby={labelledBy}
        value={custom ? null : shown}
        onValueChange={(next) => onChange(String(next))}
        className="flex flex-wrap gap-2"
      >
        {LED_SWATCHES.map((swatch) => (
          <label
            key={swatch}
            data-swatch={swatch}
            className={cn(
              'size-8 cursor-pointer rounded-full ring-1 ring-foreground/15 transition-transform ring-inset hover:scale-110 has-focus-visible:ring-2 has-focus-visible:ring-ring',
              shown === swatch && 'ring-2 ring-primary ring-offset-2 ring-offset-background',
            )}
            style={{ backgroundColor: swatch }}
          >
            <span className="sr-only">{t.swatch(swatch)}</span>
            <RadioGroupItem value={swatch} className="sr-only" />
          </label>
        ))}
      </RadioGroup>
      <label
        className={cn(
          'relative flex size-8 cursor-pointer items-center justify-center rounded-full bg-muted ring-1 ring-foreground/15 transition-transform ring-inset hover:scale-110 has-focus-visible:ring-2 has-focus-visible:ring-ring',
          custom && 'ring-2 ring-primary ring-offset-2 ring-offset-background',
        )}
        style={custom ? { backgroundColor: shown } : undefined}
      >
        <Pipette
          aria-hidden="true"
          className={cn(
            'size-4',
            custom ? 'text-white mix-blend-difference' : 'text-muted-foreground',
          )}
        />
        <NativeColor
          label={t.custom}
          value={shown}
          onPreview={setDraft}
          onCommit={(hex) => {
            setDraft(null);
            onChange(hex);
          }}
        />
      </label>
    </div>
  );
}

/**
 * Display > LED: what the device's RGB light does. The app keeps the settings and sends them on
 * every connect; the device runs the light itself. A change shows at once (UI only) and is dropped
 * when the saved config arrives, or reverted with a toast when the save fails.
 */
export function LedControls({ config }: { config: ConfigDto | null }): ReactElement {
  const [pending, setPending] = useState<LedSettingsDto | null>(null);
  // The slider moves locally while dragged and saves once on release.
  const [dragging, setDragging] = useState<number | null>(null);

  const revision = config?.revision;
  useEffect(() => setPending(null), [revision]);

  const led = pending ?? config?.led ?? null;
  const save = (next: LedSettingsDto): void => {
    setPending(next);
    void setLedSettings(next).catch((error: unknown) => {
      setPending(null);
      toast.error(t.failed, { description: errorText(error) });
    });
  };
  const update = (change: Partial<LedSettingsDto>): void => {
    if (led) save({ ...led, ...change });
  };
  const setProfileColor = (profile: string, hex: string | null): void => {
    if (!led) return;
    const profileColors = { ...led.profileColors };
    if (hex === null) delete profileColors[profile];
    else profileColors[profile] = hex;
    save({ ...led, profileColors });
  };

  if (!led) {
    return <div className="h-64 animate-pulse rounded-xl bg-muted" aria-busy="true" />;
  }
  const brightness = dragging ?? led.brightness;

  return (
    <div className="space-y-8">
      <section aria-labelledby="led-heading" className="space-y-4">
        <div className="flex items-center gap-5 rounded-xl bg-card p-5 ring-1 ring-foreground/10">
          <div className="flex size-20 shrink-0 items-center justify-center rounded-2xl bg-neutral-950">
            <LedDot
              mode={led.mode}
              color={led.color}
              speed={led.speed}
              brightness={brightness}
              className="size-8"
            />
          </div>
          <div className="min-w-0 space-y-0.5">
            <h2 id="led-heading" className="text-base font-semibold">
              {t.heading}
            </h2>
            <p className="text-sm text-muted-foreground">{t.body}</p>
            <p className="text-xs font-medium text-muted-foreground" data-testid="led-summary">
              {t.preview}: {t.modes[led.mode].name}
              {led.mode === 'off' ? null : ` · ${t.brightnessValue(brightness)}`}
            </p>
          </div>
        </div>
      </section>

      <section aria-labelledby="led-mode-heading" className="space-y-3">
        <h2 id="led-mode-heading" className="text-base font-semibold">
          {t.modeHeading}
        </h2>
        <RadioGroup
          aria-labelledby="led-mode-heading"
          value={led.mode}
          onValueChange={(mode) => mode !== led.mode && update({ mode: mode as LedMode })}
          className="grid grid-cols-2 gap-3 sm:grid-cols-3"
        >
          {LED_MODES.map((mode) => (
            <label
              key={mode}
              data-mode={mode}
              data-selected={led.mode === mode}
              className={cn(
                'group flex cursor-pointer items-start gap-3 rounded-xl bg-card p-3 ring-1 ring-foreground/10 transition-all hover:-translate-y-0.5 hover:shadow-md has-focus-visible:ring-2 has-focus-visible:ring-ring',
                led.mode === mode && 'shadow-md ring-2 shadow-primary/10 ring-primary',
              )}
            >
              <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-neutral-950">
                <LedDot mode={mode} color={led.color} speed={led.speed} className="size-3.5" />
              </span>
              <span className="min-w-0 flex-1 space-y-0.5">
                <span className="block text-sm font-medium">{t.modes[mode].name}</span>
                <span className="block text-xs text-muted-foreground">{t.modes[mode].body}</span>
              </span>
              <RadioGroupItem value={mode} className="mt-0.5" />
            </label>
          ))}
        </RadioGroup>
      </section>

      {led.mode === 'off' ? null : (
        <>
          <section aria-labelledby="led-color-heading" className="space-y-3">
            <div className="space-y-0.5">
              <h2 id="led-color-heading" className="text-base font-semibold">
                {t.colorHeading}
              </h2>
              <p className="text-sm text-muted-foreground">
                {USES_COLOR.includes(led.mode) ? t.colorBody : t.autoColor}
              </p>
            </div>
            {USES_COLOR.includes(led.mode) ? (
              <ColorPicker
                labelledBy="led-color-heading"
                value={led.color}
                onChange={(color) => color !== led.color && update({ color })}
              />
            ) : null}
          </section>

          <div className="grid gap-8 sm:grid-cols-2">
            <section aria-labelledby="led-brightness-heading" className="space-y-3">
              <div className="flex items-baseline justify-between">
                <h2 id="led-brightness-heading" className="text-base font-semibold">
                  {t.brightnessHeading}
                </h2>
                <span className="text-sm text-muted-foreground tabular-nums">
                  {t.brightnessValue(brightness)}
                </span>
              </div>
              <Slider
                aria-label={t.brightnessHeading}
                min={0}
                max={100}
                step={5}
                value={brightness}
                onValueChange={(value) =>
                  setDragging(Array.isArray(value) ? (value[0] ?? 0) : value)
                }
                onValueCommitted={(value) => {
                  const next = Array.isArray(value) ? (value[0] ?? 0) : value;
                  setDragging(null);
                  if (next !== led.brightness) update({ brightness: next });
                }}
              />
            </section>

            {USES_SPEED.includes(led.mode) ? (
              <section aria-labelledby="led-speed-heading" className="space-y-3">
                <h2 id="led-speed-heading" className="text-base font-semibold">
                  {t.speedHeading}
                </h2>
                <RadioGroup
                  aria-labelledby="led-speed-heading"
                  value={led.speed}
                  onValueChange={(speed) =>
                    speed !== led.speed && update({ speed: speed as LedSpeed })
                  }
                  className="grid-cols-3 gap-1 rounded-xl bg-muted p-1"
                >
                  {LED_SPEEDS.map((speed) => (
                    <label
                      key={speed}
                      className={cn(
                        'flex h-9 cursor-pointer items-center justify-center rounded-lg text-sm transition-colors hover:bg-background/60 has-focus-visible:ring-2 has-focus-visible:ring-ring',
                        led.speed === speed
                          ? 'bg-background font-medium shadow-sm'
                          : 'text-muted-foreground',
                      )}
                    >
                      {t.speeds[speed]}
                      <RadioGroupItem value={speed} className="sr-only" />
                    </label>
                  ))}
                </RadioGroup>
              </section>
            ) : null}
          </div>
        </>
      )}

      {led.mode === 'status' ? (
        <>
          <section aria-labelledby="led-cues-heading" className="space-y-3">
            <h2 id="led-cues-heading" className="text-base font-semibold">
              {t.cuesHeading}
            </h2>
            <ul className="flex flex-wrap gap-2">
              {CUES.map(([cue, color]) => (
                <li
                  key={cue}
                  className="flex items-center gap-2 rounded-full bg-muted py-1 pr-3 pl-1.5 text-sm"
                >
                  <span className="flex size-6 items-center justify-center rounded-full bg-neutral-950">
                    <LedDot mode="solid" color={color} speed="medium" className="size-2.5" />
                  </span>
                  {t.cues[cue]}
                </li>
              ))}
            </ul>
          </section>

          <section aria-labelledby="led-per-app-heading" className="space-y-3">
            <div className="space-y-0.5">
              <h2 id="led-per-app-heading" className="text-base font-semibold">
                {t.perAppHeading}
              </h2>
              <p className="text-sm text-muted-foreground">{t.perAppBody}</p>
            </div>
            <ul className="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {(config?.profiles ?? []).map((profile) => {
                const own = led.profileColors[profile.id];
                const shown = own ?? led.color;
                return (
                  <li
                    key={profile.id}
                    data-profile={profile.id}
                    className="flex items-center gap-3 rounded-lg bg-card px-3 py-2 ring-1 ring-foreground/10"
                  >
                    <label
                      className="relative size-6 shrink-0 cursor-pointer rounded-full ring-1 ring-foreground/15 ring-inset has-focus-visible:ring-2 has-focus-visible:ring-ring"
                      style={{ backgroundColor: shown, opacity: own ? 1 : 0.45 }}
                    >
                      <NativeColor
                        label={t.profileColor(profile.name)}
                        value={shown}
                        onPreview={() => undefined}
                        onCommit={(hex) => setProfileColor(profile.id, hex)}
                      />
                    </label>
                    <span
                      className={cn(
                        'min-w-0 flex-1 truncate text-sm',
                        !own && 'text-muted-foreground',
                      )}
                    >
                      {profile.name}
                    </span>
                    {own ? (
                      <button
                        type="button"
                        aria-label={t.useDefault(profile.name)}
                        onClick={() => setProfileColor(profile.id, null)}
                        className="rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                      >
                        <X className="size-3.5" aria-hidden="true" />
                      </button>
                    ) : null}
                  </li>
                );
              })}
            </ul>
          </section>
        </>
      ) : null}
    </div>
  );
}
