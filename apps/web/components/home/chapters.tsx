'use client';

import { useEffect, useRef, useState, type CSSProperties, type ReactElement } from 'react';
import { cn } from '@kivori/ui/lib/utils';
import { ProductShot } from '@/components/product-shot';
import { renders, type MediaImage } from '@/lib/media';

// Highlight boxes in % of the 3000x1800 straight-on cut-outs, measured from the files (knob, the
// three-key group and the screen, each padded by about one percent).
interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
  radius: string;
}
const KNOB: Box = { left: 18.4, top: 14.7, width: 30.6, height: 31.7, radius: '50%' };
const KEYS: Box = { left: 47.3, top: 34.5, width: 33.6, height: 12.5, radius: '12px' };
const SCREEN: Box = { left: 55, top: 26.2, width: 16.8, height: 11, radius: '10px' };

const APPS = ['Browser', 'Music', 'Meetings', 'Code'] as const;
const BADGES = [
  { label: 'Confirmed', dot: '#3cc47c' },
  { label: 'Started', dot: '#4fb7ff' },
  { label: 'Unverified', dot: '#f5a524' },
  { label: 'Error', dot: '#ef4444' },
] as const;

interface Chapter {
  id: string;
  title: string;
  body: string;
  image: MediaImage;
  box: Box;
  extra?: ReactElement;
}

const CHAPTERS: readonly Chapter[] = [
  {
    id: 'turn',
    title: 'Turn.',
    body: 'Change the volume, or move through your browser tabs. Every click of the knob is one step.',
    image: renders.cutoutVolume,
    box: KNOB,
  },
  {
    id: 'press',
    title: 'Press.',
    body: 'Skip tracks, pause, or fire your meeting and editor shortcuts. Each key has a press and a hold.',
    image: renders.cutoutMedia,
    box: KEYS,
    extra: (
      <>
        <ul className="mt-6 flex flex-wrap gap-2">
          {APPS.map((app) => (
            <li
              key={app}
              className="rounded-full border border-home-hairline px-4 py-1.5 text-sm text-home-ink"
            >
              {app}
            </li>
          ))}
        </ul>
        <p className="mt-3 text-sm text-home-graphite">
          Controls change with the app in front of you.
        </p>
      </>
    ),
  },
  {
    id: 'glance',
    title: 'Glance.',
    body: 'Buddy, clock, volume, media and system. Double-press the knob to flip between views.',
    image: renders.cutoutSystem,
    box: SCREEN,
  },
  {
    id: 'know',
    title: 'Know.',
    body: 'Every action ends in a badge that says what Kivori actually knows. A shortcut is never shown as a success it cannot prove.',
    image: renders.cutoutBuddy,
    box: SCREEN,
    extra: (
      <ul className="mt-6 flex flex-wrap gap-2">
        {BADGES.map(({ label, dot }) => (
          <li
            key={label}
            className="flex items-center gap-2 rounded-full border border-home-hairline px-3 py-1.5 font-pixel text-[11px] text-home-ink"
          >
            <span
              className="size-2 rounded-full"
              style={{ backgroundColor: dot }}
              aria-hidden="true"
            />
            {label}
          </li>
        ))}
      </ul>
    ),
  },
];

function ringStyle(box: Box): CSSProperties {
  return {
    left: `${box.left}%`,
    top: `${box.top}%`,
    width: `${box.width}%`,
    height: `${box.height}%`,
    borderRadius: box.radius,
  };
}

const RING =
  'pointer-events-none absolute border-2 border-mint outline-[6px] outline-mint/40 transition-[left,top,width,height,border-radius] duration-[400ms] ease-out motion-reduce:transition-none';

export function Chapters(): ReactElement {
  const [active, setActive] = useState(0);
  const refs = useRef<(HTMLElement | null)[]>([]);

  // The chapter crossing the middle band of the viewport is the active one.
  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) setActive(refs.current.indexOf(entry.target as HTMLElement));
        }
      },
      { rootMargin: '-45% 0px -45% 0px' },
    );
    for (const el of refs.current) if (el) observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <section
      id="how"
      aria-labelledby="how-title"
      className="scroll-mt-20 overflow-clip pt-4 pb-4 lg:pb-24 [--stage-w:calc((min(100vw-4rem,1180px)-5rem)*13/23)]"
    >
      <h2 id="how-title" className="sr-only">
        How Kivori works
      </h2>
      <div className="mx-auto max-w-[calc(1180px+4rem)] px-5 sm:px-8 lg:grid lg:grid-cols-[1rem_minmax(0,10fr)_minmax(0,13fr)] lg:gap-x-8">
        <div className="hidden lg:block">
          <div
            className="sticky top-[max(5.5rem,calc(50svh-var(--stage-w)*3/10))] flex h-[calc(var(--stage-w)*3/5)] justify-center"
            aria-hidden="true"
          >
            <div className="relative flex h-full w-px flex-col justify-between bg-home-hairline">
              {CHAPTERS.map((chapter, i) => (
                <span
                  key={chapter.id}
                  className={cn(
                    'absolute left-1/2 size-3 -translate-x-1/2 rounded-full border transition-colors motion-reduce:transition-none',
                    i === active
                      ? 'border-transparent bg-mint'
                      : 'border-home-hairline bg-home-paper',
                  )}
                  style={{ top: `calc(${(i / (CHAPTERS.length - 1)) * 100}% - 0.375rem)` }}
                />
              ))}
            </div>
          </div>
        </div>

        <div>
          {CHAPTERS.map((chapter, i) => (
            <article
              key={chapter.id}
              id={chapter.id}
              ref={(el) => {
                refs.current[i] = el;
              }}
              className="py-14 lg:flex lg:min-h-[85vh] lg:flex-col lg:justify-center lg:py-10 lg:last:min-h-[65vh]"
            >
              <div className="relative -mx-5 aspect-[5/3] sm:mx-0 lg:hidden">
                <ProductShot
                  image={chapter.image}
                  large
                  sizes="100vw"
                  className="home-fade-sides absolute inset-0 h-full w-full"
                />
                <div
                  className={cn(RING, 'transition-none')}
                  style={ringStyle(chapter.box)}
                  aria-hidden="true"
                />
              </div>
              <h3 className="home-display mt-8 w-fit text-[clamp(3.5rem,14vw,4.5rem)] text-home-ink lg:mt-0 lg:text-[clamp(4rem,7vw,6.5rem)]">
                {chapter.title}
              </h3>
              <p className="mt-5 max-w-[40ch] text-lg text-home-graphite lg:text-xl">
                {chapter.body}
              </p>
              {chapter.extra}
            </article>
          ))}
        </div>

        <div className="hidden lg:block">
          <div className="sticky top-[max(5.5rem,calc(50svh-var(--stage-w)*3/10))]">
            <div className="relative aspect-[5/3]">
              {CHAPTERS.map((chapter, i) => (
                <div
                  key={chapter.id}
                  aria-hidden={i !== active}
                  className={cn(
                    'absolute inset-0 transition-opacity duration-300 motion-reduce:transition-none',
                    i === active ? 'opacity-100' : 'opacity-0',
                  )}
                >
                  <ProductShot
                    image={chapter.image}
                    large
                    sizes="(min-width: 1244px) 620px, 56vw"
                    className="home-fade-sides absolute inset-0 h-full w-full"
                  />
                </div>
              ))}
              <div
                className={RING}
                style={ringStyle(CHAPTERS[active]?.box ?? KNOB)}
                aria-hidden="true"
              />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
