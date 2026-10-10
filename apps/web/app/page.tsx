import Image from 'next/image';
import type { ReactElement } from 'react';
import mascot from '@brand/mascot.svg';
import { TAGLINE } from '@/lib/site';

export default function HomePage(): ReactElement {
  return (
    <section className="mx-auto flex max-w-3xl flex-col items-center px-5 py-16 text-center sm:py-24">
      <Image
        src={mascot}
        alt="The Kivori keycap buddy"
        width={200}
        height={200}
        priority
        className="animate-float"
      />
      <span className="mt-8 rounded-full border border-border bg-accent px-3 py-1 text-xs font-medium tracking-wide text-accent-foreground uppercase">
        Coming soon
      </span>
      <h1 className="mt-5 font-display text-4xl leading-tight font-semibold sm:text-5xl">
        {TAGLINE}
      </h1>
      <p className="mt-5 max-w-xl text-lg text-muted-foreground">
        Kivori is a desk buddy you control your computer with: the buddy is why you want one, the
        physical knob and button are why you keep using it.
      </p>
      <div className="mt-8 flex flex-wrap justify-center gap-3">
        <a
          href="/download"
          className="rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition-opacity hover:opacity-90"
        >
          Get notified
        </a>
        <a
          href="/products"
          className="rounded-lg border border-border px-5 py-2.5 text-sm font-medium transition-colors hover:bg-secondary"
        >
          Learn more
        </a>
      </div>
    </section>
  );
}
