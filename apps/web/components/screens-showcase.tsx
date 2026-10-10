'use client';

import type { ReactElement } from 'react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@kivori/ui/components/tabs';
import { DeviceScreen } from '@/components/device-screen';
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

/** The five real display views as segmented Tabs, each in a glowing device screen. */
export function ScreensShowcase(): ReactElement {
  return (
    <Tabs defaultValue="buddy" className="items-center gap-12">
      <TabsList className="h-auto max-w-full flex-wrap justify-center gap-1 rounded-full bg-white/8 p-1.5">
        {VIEWS.map((view) => (
          <TabsTrigger
            key={view.value}
            value={view.value}
            className="h-10 rounded-full px-5 text-base text-white/70 hover:text-white data-active:bg-white data-active:text-[#0c101c] data-active:shadow-none dark:data-active:bg-white dark:data-active:text-[#0c101c]"
          >
            {view.label}
          </TabsTrigger>
        ))}
      </TabsList>
      {VIEWS.map((view) => (
        <TabsContent key={view.value} value={view.value} className="w-full">
          <div className="grid items-center gap-10 md:grid-cols-2 md:gap-16">
            <DeviceScreen image={view.image} glow className="max-w-sm md:max-w-md md:justify-self-end" />
            <div className="text-center md:text-left">
              <h3 className="display-3">{view.title}</h3>
              <p className="lead mt-4 max-w-md md:mx-0">{view.body}</p>
            </div>
          </div>
        </TabsContent>
      ))}
    </Tabs>
  );
}
