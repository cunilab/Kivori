'use client';

import Image from 'next/image';
import type { ReactElement } from 'react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@kivori/ui/components/tabs';
import { screens, type MediaImage } from '@/lib/media';

interface View {
  value: string;
  label: string;
  image: MediaImage;
  title: string;
  body: string;
}

const VIEWS: readonly View[] = [
  {
    value: 'buddy',
    label: 'Buddy',
    image: screens.buddy,
    title: 'Your buddy, always there.',
    body: 'A friendly face, the name of the profile you are in and what each control does right now.',
  },
  {
    value: 'clock',
    label: 'Clock',
    image: screens.clock,
    title: 'A clock you can read across the desk.',
    body: 'Big, calm digits for when you just want to know the time.',
  },
  {
    value: 'volume',
    label: 'Volume',
    image: screens.volume,
    title: 'Volume you can see.',
    body: 'A clear dial that follows your system volume, and says so when you are muted.',
  },
  {
    value: 'media',
    label: 'Media',
    image: screens.media,
    title: 'What is playing, right now.',
    body: 'The song, and whether it is playing or paused, without opening the player.',
  },
  {
    value: 'system',
    label: 'System',
    image: screens.system,
    title: 'How your computer feels.',
    body: 'Live gauges for processor and memory, with the last minute at a glance.',
  },
];

/** The five real display views as Tabs: each frame large, crisp and pixel-sharp on the stage. */
export function ScreensShowcase(): ReactElement {
  return (
    <Tabs defaultValue="buddy" className="items-center gap-10">
      <TabsList className="h-auto! max-w-full flex-wrap justify-center gap-1 rounded-full border border-border bg-card p-1.5">
        {VIEWS.map((view) => (
          <TabsTrigger
            key={view.value}
            value={view.value}
            className="pixel h-10 rounded-full px-5 text-xs data-active:bg-mint data-active:text-primary-foreground dark:data-active:border-transparent dark:data-active:bg-mint dark:data-active:text-primary-foreground"
          >
            {view.label}
          </TabsTrigger>
        ))}
      </TabsList>
      {VIEWS.map((view) => (
        <TabsContent key={view.value} value={view.value} className="w-full">
          <div className="grid items-center gap-8 md:grid-cols-2 md:gap-16">
            <div className="relative isolate mx-auto w-full max-w-[420px] md:justify-self-end">
              <div
                aria-hidden="true"
                className="glow stage-glow absolute -inset-10 -z-10 blur-xl"
              />
              <div className="rounded-[2rem] bg-linear-to-b from-[#222a40] to-[#0a0d17] p-3 shadow-[0_30px_80px_-20px_rgb(0_0_0/0.7)] ring-1 ring-white/10 sm:p-4">
                <Image
                  src={view.image.src}
                  alt={view.image.alt}
                  width={view.image.width}
                  height={view.image.height}
                  unoptimized
                  sizes="(min-width: 768px) 420px, 90vw"
                  className="aspect-square w-full rounded-[1.25rem] bg-stage [image-rendering:pixelated]"
                />
              </div>
            </div>
            <div className="text-center md:text-left">
              <h3 className="display-3">{view.title}</h3>
              <p className="lead mt-4 max-w-md max-md:mx-auto">{view.body}</p>
            </div>
          </div>
        </TabsContent>
      ))}
    </Tabs>
  );
}
