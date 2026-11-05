import Link from 'next/link';
import type { ReactElement } from 'react';
import { Button } from '@kivori/ui/components/button';
import { KivoriDemo } from '@/components/kivori-demo';

export function Hero(): ReactElement {
  return (
    <section aria-labelledby="hero-title" className="px-5 pt-12 sm:px-8 sm:pt-16">
      <div className="mx-auto max-w-[1180px]">
        <div className="grid gap-6 pb-10 md:grid-cols-[1.1fr_0.9fr] md:items-start md:gap-16 md:pb-12">
          <h1 id="hero-title" className="display-1">
            <span className="block">Desktop control.</span>
            <span className="block">Made physical.</span>
          </h1>
          <div className="md:pt-1">
            <p className="max-w-[42ch] text-lg leading-relaxed sm:text-xl">
              Meet KV-P1. A knob, three keys and a small companion for the things you do every day.
            </p>
            <div className="mt-6 flex flex-wrap items-center gap-x-7 gap-y-4">
              <Button
                size="lg"
                className="site-button h-12 px-6 text-base"
                nativeButton={false}
                render={<Link href="/products/kv-p1" />}
              >
                Explore KV-P1
              </Button>
              <Link
                href="/download"
                className="font-semibold text-teal underline underline-offset-4 hover:text-foreground"
              >
                Download the app
              </Link>
            </div>
            <p className="mt-5 text-sm text-muted-foreground">
              In development. Price and ship date to come.
            </p>
          </div>
        </div>
        <div className="rounded-lg bg-stage px-4 pt-5 pb-6 sm:px-8 sm:pt-7 sm:pb-9">
          <p className="mb-5 text-base font-semibold text-white">Try the controls</p>
          <div className="mx-auto max-w-[640px]">
            <KivoriDemo />
          </div>
        </div>
      </div>
    </section>
  );
}
