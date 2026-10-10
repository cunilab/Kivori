import Image from 'next/image';
import {
  ArrowRightIcon,
  CheckIcon,
  EyeOffIcon,
  KeyboardIcon,
  LayersIcon,
  ListChecksIcon,
  LockKeyholeIcon,
  SmileIcon,
  UserXIcon,
  WifiOffIcon,
  type LucideIcon,
} from 'lucide-react';
import type { ReactElement, ReactNode } from 'react';
import { Badge } from '@kivori/ui/components/badge';
import { Card } from '@kivori/ui/components/card';
import { cn } from '@kivori/ui/lib/utils';
import { ProductShot } from '@/components/product-shot';
import { moods, renders } from '@/lib/media';

interface BentoCardProps {
  icon: LucideIcon;
  title: string;
  body: string;
  className?: string;
  dark?: boolean;
  children?: ReactNode;
}

function BentoCard({
  icon: Icon,
  title,
  body,
  className,
  dark = false,
  children,
}: BentoCardProps): ReactElement {
  return (
    <Card
      className={cn(
        'reveal gap-0 rounded-3xl p-7 shadow-sm ring-1 ring-foreground/8 sm:p-9',
        dark && 'dark bg-[#0c101c] text-foreground ring-white/10',
        className,
      )}
    >
      <span className="flex size-11 items-center justify-center rounded-2xl bg-accent text-accent-foreground">
        <Icon className="size-5" aria-hidden="true" />
      </span>
      <h3 className="display-3 mt-5 text-[1.625rem]! sm:text-[2rem]!">{title}</h3>
      <p className="mt-3 max-w-md text-base text-muted-foreground sm:text-lg">{body}</p>
      {children ? <div className="mt-8 flex-1">{children}</div> : null}
    </Card>
  );
}

const PROFILES = ['General', 'Browser', 'Code', 'Media', 'Zoom', 'Teams'] as const;

const STATUS: { name: string; color: string; hint: string }[] = [
  { name: 'Confirmed', color: 'var(--panel-green)', hint: 'Read back from your computer' },
  { name: 'Started', color: 'var(--panel-blue)', hint: 'Accepted and under way' },
  { name: 'Unverified', color: 'var(--panel-amber)', hint: 'Sent, but cannot be checked' },
  { name: 'Error', color: 'var(--panel-red)', hint: 'Did not work, nothing changed' },
];

const MACRO = ['Open', 'Wait', 'Type', 'Send'] as const;

/** Six selling points as a bento grid of rounded cards, each with a small visual from the real media. */
export function Bento(): ReactElement {
  return (
    <div className="grid gap-5 lg:grid-cols-6">
      <BentoCard
        icon={LayersIcon}
        title="Knows what you're working on."
        body="Profiles change with the app in front of you: media keys in a player, tab switching in a browser."
        className="lg:col-span-4"
      >
        <div className="flex flex-wrap gap-2.5">
          {PROFILES.map((profile) => (
            <Badge
              key={profile}
              variant={profile === 'Browser' ? 'default' : 'secondary'}
              className="h-9 rounded-full px-4 text-sm"
            >
              {profile}
            </Badge>
          ))}
        </div>
      </BentoCard>

      <BentoCard
        icon={KeyboardIcon}
        title="Three buttons, your rules."
        body="Press or hold each one. Make them do anything."
        className="lg:col-span-2"
      >
        <div className="relative -mx-4 -mb-6 h-44 overflow-hidden sm:-mb-8">
          <ProductShot
            image={renders.top}
            sizes="(min-width: 1024px) 400px, 100vw"
            className="absolute top-1/2 left-1/2 w-[150%] max-w-none -translate-x-[45%] -translate-y-1/2"
          />
        </div>
      </BentoCard>

      <BentoCard
        icon={CheckIcon}
        title="Honest status lights."
        body="After every action, the screen says what Kivori actually knows."
        className="lg:col-span-2"
      >
        <ul className="space-y-2 rounded-2xl bg-panel p-4">
          {STATUS.map((status) => (
            <li key={status.name} className="flex items-center gap-3">
              <span
                aria-hidden="true"
                className="size-3 shrink-0 rounded-full"
                style={{ background: status.color, boxShadow: `0 0 12px ${status.color}` }}
              />
              <span className="text-sm font-semibold text-white">{status.name}</span>
              <span className="ml-auto hidden truncate text-xs text-[#a9b3c9] xl:block">
                {status.hint}
              </span>
            </li>
          ))}
        </ul>
      </BentoCard>

      <BentoCard
        icon={ListChecksIcon}
        title="Macros in one press."
        body="Chain steps with optional waits and run them all with a single press."
        className="lg:col-span-2"
      >
        <ol className="flex flex-wrap items-center gap-2">
          {MACRO.map((step, index) => (
            <li key={step} className="flex items-center gap-2">
              <span className="rounded-xl bg-secondary px-3.5 py-2 text-sm font-medium">
                {step}
              </span>
              {index < MACRO.length - 1 ? (
                <ArrowRightIcon className="size-4 text-primary" aria-hidden="true" />
              ) : null}
            </li>
          ))}
        </ol>
      </BentoCard>

      <BentoCard
        icon={SmileIcon}
        title="A buddy with moods."
        body="Happy when it worked, busy under load, asleep when you are away."
        className="lg:col-span-2"
      >
        <div className="grid grid-cols-3 gap-2">
          {[moods.happy, moods.busy, moods.sleeping].map((mood) => (
            <Image
              key={mood.src}
              src={mood.src}
              alt={mood.alt}
              width={mood.width}
              height={mood.height}
              unoptimized
              loading="lazy"
              sizes="120px"
              className="aspect-square w-full rounded-xl"
            />
          ))}
        </div>
      </BentoCard>

      <BentoCard
        dark
        icon={LockKeyholeIcon}
        title="Private by design."
        body="Everything stays on your computer."
        className="lg:col-span-6"
      >
        <ul className="grid gap-4 sm:grid-cols-3">
          {[
            { icon: WifiOffIcon, label: 'Works offline' },
            { icon: UserXIcon, label: 'No account' },
            { icon: EyeOffIcon, label: 'No tracking' },
          ].map(({ icon: Icon, label }) => (
            <li
              key={label}
              className="flex items-center gap-3 rounded-2xl bg-white/6 px-5 py-4 text-lg font-medium"
            >
              <Icon className="size-5 text-primary" aria-hidden="true" />
              {label}
            </li>
          ))}
        </ul>
      </BentoCard>
    </div>
  );
}
