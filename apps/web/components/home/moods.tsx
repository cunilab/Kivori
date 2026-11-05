import Image from 'next/image';
import type { ReactElement } from 'react';
import { moods } from '@/lib/media';

const ROW = [
  ['Idle', moods.idle],
  ['Attentive', moods.attentive],
  ['Listening', moods.listening],
  ['Busy', moods.busy],
  ['Happy', moods.happy],
  ['Sleeping', moods.sleeping],
] as const;

export function Moods(): ReactElement {
  return (
    <section
      id="moods"
      aria-labelledby="moods-title"
      className="scroll-mt-20 overflow-x-clip bg-home-paper px-5 py-14 sm:py-20 sm:px-8"
    >
      <div className="mx-auto max-w-[1180px]">
        <h2 id="moods-title" className="home-display text-[clamp(2.5rem,6vw,4.5rem)] text-home-ink">
          It has moods.
        </h2>
        <p className="mt-5 max-w-[52ch] text-lg text-home-graphite">
          Strained under heavy load, muted when you are muted, happy after a confirmed action. Turn
          reactions off any time.
        </p>
        <ul className="mt-12 grid grid-cols-3 gap-3 sm:gap-5 lg:grid-cols-6">
          {ROW.map(([label, image]) => (
            <li key={label}>
              <div className="flex aspect-square items-center justify-center rounded-[20px] bg-[#080c18]">
                <Image
                  src={image.src}
                  alt={image.alt}
                  width={image.width}
                  height={image.height}
                  unoptimized
                  loading="eager"
                  className="h-auto w-[82%] max-w-[150px] [image-rendering:pixelated]"
                />
              </div>
              <p className="mt-3 text-sm text-home-graphite">{label}</p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
