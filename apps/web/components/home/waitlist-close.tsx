import type { ReactElement } from 'react';
import { ProductShot } from '@/components/product-shot';
import { WaitlistSection } from '@/components/waitlist-section';
import { renders } from '@/lib/media';

export function WaitlistClose({
  initialProduct,
}: {
  initialProduct?: string | undefined;
}): ReactElement {
  return (
    <section
      id="waitlist"
      aria-labelledby="waitlist-title"
      className="scroll-mt-20 overflow-x-clip bg-home-paper px-5 py-14 sm:py-20 text-center sm:px-8"
    >
      <div className="mx-auto w-[min(440px,100%)]">
        <ProductShot image={renders.heroCutout} large sizes="440px" />
      </div>
      <h2
        id="waitlist-title"
        className="home-display mt-2 text-[clamp(2.5rem,6vw,4.5rem)] text-home-ink"
      >
        Be first to know.
      </h2>
      <p className="mx-auto mt-5 max-w-[46ch] text-lg text-home-graphite">
        There is no price or ship date yet. Leave your email and we will tell you when there is.
      </p>
      <div className="mx-auto mt-9 flex max-w-[480px] justify-center text-left">
        <WaitlistSection initialProduct={initialProduct} idPrefix="final" variant="inline" />
      </div>
    </section>
  );
}
