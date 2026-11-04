import Link from 'next/link';
import type { ReactElement } from 'react';
import { Button } from '@kivori/ui/components/button';
import { KivoriDemo } from '@/components/kivori-demo';
import { Eyebrow } from '@/components/section';

const BENEFITS = [
  {
    title: 'Your shortcuts, within reach',
    body: 'Give the knob and keys jobs that fit the way you work.',
  },
  {
    title: 'A screen with personality',
    body: 'A tiny buddy and useful information, right on your desk.',
  },
  {
    title: 'Local by design',
    body: 'Your settings stay on your computer. No account or cloud required.',
  },
] as const;

export function Hero(): ReactElement {
  return (
    <>
      <section
        aria-labelledby="hero-title"
        className="overflow-x-clip px-5 pt-10 pb-12 sm:px-8 sm:pt-20 sm:pb-16"
      >
        <div className="mx-auto grid max-w-[1180px] items-center gap-10 md:grid-cols-[1fr_1.15fr] md:gap-14">
          <div>
            <Eyebrow>Meet KV-P1</Eyebrow>
            <h1
              id="hero-title"
              className="display-1 mt-4 max-w-[13ch] md:max-w-none md:text-[clamp(2.9rem,4vw,3.8rem)]"
            >
              <span className="md:block">A little company.</span>{' '}
              <span className="md:block">A lot of control.</span>
            </h1>
            <p className="lead mt-6 max-w-[40ch]">
              A knob, three keys and a tiny desk buddy. Control music, switch browser tabs and put
              everyday shortcuts at your fingertips.
            </p>
            <div className="mt-8 flex flex-wrap items-center gap-x-7 gap-y-4">
              <Button
                size="lg"
                className="keycap h-12 px-6 text-base"
                nativeButton={false}
                render={<Link href="/products/kv-p1" />}
              >
                Explore KV-P1
              </Button>
              <Link
                href="/download"
                className="font-semibold underline underline-offset-4 hover:text-mint"
              >
                Get the desktop app
              </Link>
            </div>
            <p className="mt-6 text-sm text-muted-foreground">
              In development. Price and ship date to come.
            </p>
          </div>
          <KivoriDemo />
        </div>
      </section>
      <section aria-label="At a glance" className="px-5 sm:px-8">
        <ul className="mx-auto grid max-w-[1180px] divide-y divide-border border-y border-border sm:grid-cols-3 sm:divide-x sm:divide-y-0">
          {BENEFITS.map((benefit) => (
            <li key={benefit.title} className="py-6 sm:px-6 sm:first:pl-0 sm:last:pr-0">
              <h2 className="font-display text-xl font-bold tracking-tight">{benefit.title}</h2>
              <p className="mt-2 max-w-sm text-muted-foreground">{benefit.body}</p>
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}
