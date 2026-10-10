'use client';

import { useEffect, useRef, useState, type ReactElement } from 'react';
import { cn } from '@kivori/ui/lib/utils';
import { ProductShot } from '@/components/product-shot';
import { renders, type MediaImage } from '@/lib/media';

interface Panel {
  headline: string;
  body: string;
  image: MediaImage;
}

const PANELS: readonly Panel[] = [
  {
    headline: 'Turn.',
    body: 'Volume right under your hand. Faster turns take bigger steps.',
    image: renders.frontVolume,
  },
  {
    headline: 'Press.',
    body: 'Play, pause and skip on three keys that are always where you left them.',
    image: renders.frontMedia,
  },
  {
    headline: 'Glance.',
    body: 'The time, what is playing, how your computer feels. One look, no window switching.',
    image: renders.frontClock,
  },
];

/**
 * Sticky scroll story: the front render stays pinned while its screen changes as each text panel
 * crosses the middle of the viewport. Layers crossfade over the base render; the panels themselves are
 * ordinary stacked content, so it still reads fine without scripting or with reduced motion.
 */
export function ScrollStory(): ReactElement {
  const [active, setActive] = useState(0);
  const panelRefs = useRef<(HTMLDivElement | null)[]>([]);

  useEffect(() => {
    const nodes = panelRefs.current.filter((node): node is HTMLDivElement => node !== null);
    if (typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) setActive(nodes.indexOf(entry.target as HTMLDivElement));
        }
      },
      { rootMargin: '-45% 0px -45% 0px' },
    );
    nodes.forEach((node) => observer.observe(node));
    return () => observer.disconnect();
  }, []);

  return (
    <div className="grid md:grid-cols-2 md:gap-16">
      <div className="sticky top-14 z-10 self-start bg-surface py-4 md:top-24 md:bg-transparent md:py-0 md:pt-[calc(50vh-9rem)]">
        <div className="shot-stage relative aspect-[3/2] overflow-hidden rounded-[2rem] bg-white shadow-sm ring-1 ring-black/5">
          <ProductShot
            image={renders.front}
            sizes="(min-width: 768px) 560px, 100vw"
            className="mix-blend-normal"
          />
          {PANELS.map((panel, index) => (
            <div
              key={panel.headline}
              aria-hidden={index !== active}
              className={cn(
                'absolute inset-0 transition-opacity duration-700 motion-reduce:transition-none',
                index === active ? 'opacity-100' : 'opacity-0',
              )}
            >
              <ProductShot
                image={panel.image}
                sizes="(min-width: 768px) 560px, 100vw"
                className="mix-blend-normal"
              />
            </div>
          ))}
        </div>
      </div>
      <div>
        {PANELS.map((panel, index) => (
          <div
            key={panel.headline}
            ref={(node) => {
              panelRefs.current[index] = node;
            }}
            className="flex min-h-[55vh] flex-col justify-center md:min-h-screen"
          >
            <h3 className={cn('display-1 transition-colors duration-500', index === active ? 'text-foreground' : 'text-foreground/25')}>
              {panel.headline}
            </h3>
            <p className="lead mt-5 max-w-md">{panel.body}</p>
          </div>
        ))}
      </div>
    </div>
  );
}
