import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import {
  Cpu,
  LoaderCircle,
  MemoryStick,
  Music2,
  Pause,
  Play,
  Square,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import type { DeskStatusDto, DisplayMode } from '@/lib/ipc/types';

// Custom (no shadcn equivalent): a CSS illustration of the Kivori hardware and its five screen
// views. It is never a pixel mirror of the device (that renderer is Device-Studio-only); live values
// appear only when the desktop knows them, otherwise the art shows neutral placeholders.

type Values = Pick<
  DeskStatusDto,
  'volumePercent' | 'muted' | 'media' | 'mediaTitle' | 'mediaArtist' | 'cpuPercent' | 'ramPercent'
>;

function useClock(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(timer);
  }, []);
  return now;
}

function Bar({ value, tone }: { value: number | null; tone: string }): ReactElement {
  return (
    <div className="h-[4cqw] w-full overflow-hidden rounded-full bg-white/10">
      <div
        className={cn('h-full rounded-full transition-[width] duration-500', tone)}
        style={{ width: `${value ?? 0}%` }}
      />
    </div>
  );
}

function Line({ w }: { w: string }): ReactElement {
  return <div className={cn('h-[4cqw] rounded-full bg-white/15', w)} />;
}

function Buddy(): ReactElement {
  return (
    <div className="flex size-full items-center justify-center">
      <div className="relative size-[56%] motion-safe:animate-float">
        <div className="absolute inset-0 translate-y-[6%] rounded-[24%] bg-[#2c5fb8]" />
        <div className="absolute inset-0 flex items-center justify-center gap-[18%] rounded-[24%] bg-panel-blue shadow-[inset_0_-6px_0_rgb(0_0_0/0.15)]">
          <span className="h-[26%] w-[13%] rounded-full bg-white motion-safe:animate-blink" />
          <span className="h-[26%] w-[13%] rounded-full bg-white motion-safe:animate-blink" />
        </div>
        <span className="absolute bottom-[22%] left-1/2 h-[7%] w-[22%] -translate-x-1/2 rounded-b-full bg-panel/70" />
      </div>
    </div>
  );
}

function Clock(): ReactElement {
  const now = useClock();
  return (
    <div className="flex size-full flex-col items-center justify-center gap-[3cqw]">
      <span className="text-[24cqw] leading-none font-semibold tracking-tight text-white tabular-nums">
        {now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })}
      </span>
      <span className="text-[7cqw] font-medium tracking-wide text-panel-blue uppercase">
        {now.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' })}
      </span>
    </div>
  );
}

function VolumeView({ values }: { values?: Values | undefined }): ReactElement {
  const volume = values?.volumePercent ?? null;
  const Icon = values?.muted ? VolumeX : Volume2;
  return (
    <div className="flex size-full flex-col items-center justify-center gap-[5cqw] px-[14%]">
      <Icon
        className={cn('size-[22cqw]', values?.muted ? 'text-panel-red' : 'text-panel-blue')}
        aria-hidden="true"
      />
      {values ? (
        <span className="text-[16cqw] leading-none font-semibold text-white tabular-nums">
          {volume === null ? '—' : `${volume}%`}
        </span>
      ) : (
        <Line w="w-[40%]" />
      )}
      <Bar value={values ? volume : 60} tone="bg-panel-blue" />
    </div>
  );
}

function MediaView({ values }: { values?: Values | undefined }): ReactElement {
  const StateIcon =
    values?.media === 'playing' ? Play : values?.media === 'paused' ? Pause : Square;
  return (
    <div className="flex size-full flex-col items-center justify-center gap-[4cqw] px-[12%] text-center">
      <div className="flex size-[30cqw] items-center justify-center rounded-[22%] bg-gradient-to-br from-panel-amber to-[#e0702a]">
        <Music2 className="size-[15cqw] text-panel" aria-hidden="true" />
      </div>
      {values?.mediaTitle ? (
        <div className="w-full min-w-0">
          <p className="truncate text-[8cqw] font-semibold text-white">{values.mediaTitle}</p>
          <p className="truncate text-[6.5cqw] text-white/60">{values.mediaArtist || ' '}</p>
        </div>
      ) : (
        <div className="flex w-full flex-col items-center gap-[2.5cqw]">
          <Line w="w-[70%]" />
          <Line w="w-[45%]" />
        </div>
      )}
      {values?.media ? (
        <StateIcon className="size-[8cqw] fill-current text-panel-amber" aria-hidden="true" />
      ) : null}
    </div>
  );
}

function SystemView({ values }: { values?: Values | undefined }): ReactElement {
  const rows = [
    { Icon: Cpu, value: values ? values.cpuPercent : 35, tone: 'bg-panel-green' },
    { Icon: MemoryStick, value: values ? values.ramPercent : 55, tone: 'bg-panel-blue' },
  ];
  return (
    <div className="flex size-full flex-col justify-center gap-[9cqw] px-[13%]">
      {rows.map(({ Icon, value, tone }, index) => (
        <div key={index} className="flex flex-col gap-[3cqw]">
          <div className="flex items-center justify-between">
            <Icon className="size-[10cqw] text-white/70" aria-hidden="true" />
            {values ? (
              <span className="text-[10cqw] leading-none font-semibold text-white tabular-nums">
                {value === null ? '—' : `${value}%`}
              </span>
            ) : null}
          </div>
          <Bar value={value} tone={value !== null && value >= 85 ? 'bg-panel-red' : tone} />
        </div>
      ))}
    </div>
  );
}

/** The 240×240 screen alone. Without `values` it is a neutral sample of the view. */
export function DeviceScreen({
  mode,
  values,
  busy = false,
  className,
}: {
  mode: DisplayMode;
  values?: Values | undefined;
  busy?: boolean | undefined;
  className?: string;
}): ReactElement {
  return (
    <div
      className={cn(
        '@container relative aspect-square overflow-hidden rounded-[8%] bg-panel ring-1 ring-white/10',
        className,
      )}
    >
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(120%_80%_at_50%_0%,rgb(76_141_246/0.18),transparent_60%)]" />
      {busy ? (
        <div className="flex size-full items-center justify-center">
          <LoaderCircle className="size-[22cqw] animate-spin text-panel-blue" aria-hidden="true" />
        </div>
      ) : mode === 'buddy' ? (
        <Buddy />
      ) : mode === 'clock' ? (
        <Clock />
      ) : mode === 'volume' ? (
        <VolumeView values={values} />
      ) : mode === 'media' ? (
        <MediaView values={values} />
      ) : (
        <SystemView values={values} />
      )}
    </div>
  );
}

/** The whole device: a navy body with the screen and the knob. */
export function DeviceArt({
  mode,
  values,
  busy,
  label,
  className,
}: {
  mode: DisplayMode;
  values?: Values | undefined;
  busy?: boolean | undefined;
  label: string;
  className?: string;
}): ReactElement {
  return (
    <figure
      role="img"
      aria-label={label}
      className={cn(
        'relative flex aspect-[1.45] items-center gap-[7%] rounded-[16%] bg-gradient-to-b from-[#1b2236] to-[#0c101c] p-[7%] shadow-[0_24px_48px_-20px_rgb(12_16_28/0.55),inset_0_1px_0_rgb(255_255_255/0.08)] ring-1 ring-black/20',
        className,
      )}
    >
      <DeviceScreen mode={mode} values={values} busy={busy} className="h-full" />
      <div className="relative aspect-square flex-1 rounded-full bg-gradient-to-b from-[#3a4257] to-[#151a28] shadow-[0_6px_14px_rgb(0_0_0/0.4),inset_0_1px_0_rgb(255_255_255/0.15)]">
        <div className="absolute inset-[16%] rounded-full bg-[repeating-conic-gradient(rgb(255_255_255/0.07)_0deg_4deg,transparent_4deg_12deg)]" />
        <span className="absolute top-[12%] left-1/2 h-[16%] w-[5%] -translate-x-1/2 rounded-full bg-panel-amber" />
      </div>
    </figure>
  );
}
