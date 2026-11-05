import type { ReactElement } from 'react';
import { KivoriDemo } from '@/components/kivori-demo';

export function TryBand(): ReactElement {
  return (
    <section
      id="try"
      aria-labelledby="try-title"
      className="scroll-mt-20 overflow-x-clip bg-home-deep px-5 py-14 sm:py-20 sm:px-8"
    >
      <div className="mx-auto max-w-[1180px] text-center">
        <h2 id="try-title" className="home-display text-[clamp(2.75rem,7vw,5.5rem)] text-white">
          Try it here.
        </h2>
        <p className="mx-auto mt-5 max-w-[46ch] text-lg text-[#98a69f]">
          Turn the knob or press a key. The screen shows what happened.
        </p>
        <div className="mx-auto mt-12 max-w-[640px] text-left sm:mt-16">
          <KivoriDemo />
        </div>
      </div>
    </section>
  );
}
