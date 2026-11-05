import Link from 'next/link';
import type { ReactElement } from 'react';
import { ProductShot } from '@/components/product-shot';
import { renders } from '@/lib/media';

export function Hero(): ReactElement {
  return (
    <section data-home aria-labelledby="hero-title" className="overflow-x-clip">
      <div className="mx-auto max-w-[calc(1180px+4rem)] px-5 pt-12 text-center sm:px-8 sm:pt-16">
        <h1
          id="hero-title"
          className="home-display mx-auto text-[clamp(2.6rem,6.6vw,5.25rem)] text-home-ink"
        >
          <span className="block">One knob. Three keys.</span>
          <span className="block">One honest buddy.</span>
        </h1>
        <p className="mx-auto mt-6 max-w-[44ch] text-lg text-home-graphite sm:text-xl">
          KV-P1 sits beside your keyboard. Turn, press and glance to control your computer, and the
          little screen shows what really happened.
        </p>
        <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
          <Link
            href="/#waitlist"
            className="inline-flex h-12 items-center rounded-full bg-mint px-7 font-semibold text-[#0f1513] transition-transform active:scale-[0.97]"
          >
            Join the waitlist
          </Link>
          <Link
            href="#try"
            className="inline-flex h-12 items-center rounded-full px-6 font-semibold text-home-ink ring-1 ring-home-hairline transition-colors ring-inset hover:bg-home-raised"
          >
            Try it live
          </Link>
        </div>
        <p className="mt-5 text-sm text-home-graphite">
          In development. No price or ship date yet.
        </p>
      </div>
      <div className="home-rise relative left-1/2 -mt-[6%] w-[min(1040px,118vw)] max-w-none -translate-x-1/2 lg:-mt-36">
        <ProductShot
          image={renders.heroCutout}
          large
          priority
          sizes="(min-width: 1120px) 1120px, 118vw"
        />
      </div>
    </section>
  );
}
