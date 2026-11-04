import Link from 'next/link';
import type { ReactElement } from 'react';
import { BrandMark } from '@/components/brand-mark';
import { SiteNav } from '@/components/site-nav';

export function SiteHeader(): ReactElement {
  return (
    <header className="sticky top-0 z-50 border-b border-border/70 bg-background/80 backdrop-blur-xl">
      <div className="mx-auto flex h-16 max-w-[calc(1180px+4rem)] items-center justify-between gap-4 px-5 sm:px-8">
        <Link
          href="/"
          className="flex w-fit items-center gap-2.5 font-display text-xl font-bold tracking-tight"
        >
          <BrandMark />
          Kivori
        </Link>
        <SiteNav />
      </div>
    </header>
  );
}
