import Image from 'next/image';
import Link from 'next/link';
import type { ReactElement } from 'react';
import mascot from '@brand/mascot.svg';
import { SiteNav } from '@/components/site-nav';

/** Slim translucent sticky bar: wordmark left, links centre, Notify me right. */
export function SiteHeader(): ReactElement {
  return (
    <header className="sticky top-0 z-50 border-b border-border/70 bg-background/75 backdrop-blur-xl backdrop-saturate-150">
      <div className="mx-auto grid h-14 max-w-[1200px] grid-cols-[1fr_auto] items-center gap-4 px-5 sm:px-8 md:grid-cols-[1fr_auto_1fr]">
        <Link
          href="/"
          className="flex w-fit items-center gap-2 text-lg font-semibold tracking-tight"
        >
          <Image src={mascot} alt="" width={26} height={26} />
          Kivori
        </Link>
        <SiteNav />
      </div>
    </header>
  );
}
