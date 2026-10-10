import type { ReactElement } from 'react';
import { KivoriDemo } from '@/components/kivori-demo';
import { Eyebrow } from '@/components/section';
import { WaitlistSection } from '@/components/waitlist-section';

const TICKS = ['Windows first', 'No account', 'Works offline'] as const;

/** Copy and the real waitlist on the left, the interactive Kivori on the right. */
export function Hero({ initialProduct }: { initialProduct?: string | undefined }): ReactElement {
  return (
    <section
      aria-labelledby="hero-title"
      className="overflow-x-clip px-5 pt-6 pb-10 sm:px-8 sm:pt-16"
    >
      <div className="mx-auto grid max-w-[1180px] items-center gap-8 md:grid-cols-[5fr_7fr] md:gap-14">
        <div>
          <Eyebrow>A desk buddy with a knob</Eyebrow>
          <h1 id="hero-title" className="display-1 mt-4 mb-5">
            Say hi to your desk&apos;s new <em className="text-mint not-italic">roommate.</em>
          </h1>
          <p className="lead mb-7 max-w-[34ch]">
            A big knob, three keys and a tiny friend who shows what your computer is up to.{' '}
            <strong className="font-semibold text-foreground">Turn it. Watch it react.</strong>
          </p>
          <div id="waitlist" className="scroll-mt-20">
            <WaitlistSection initialProduct={initialProduct} idPrefix="hero" variant="inline" />
          </div>
          <ul className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
            {TICKS.map((tick) => (
              <li key={tick}>
                <span aria-hidden="true" className="text-mint">
                  &#10003;{' '}
                </span>
                {tick}
              </li>
            ))}
          </ul>
        </div>
        <KivoriDemo />
      </div>
    </section>
  );
}
