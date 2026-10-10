import Link from 'next/link';
import type { ReactElement } from 'react';
import { BrandMark } from '@/components/brand-mark';
import { FOOTER_NAV } from '@/lib/nav';

export function SiteFooter(): ReactElement {
  return (
    <footer className="border-t border-border">
      <div className="mx-auto max-w-[1180px] px-5 py-14 sm:px-8">
        <div className="grid gap-10 sm:grid-cols-[1.5fr_1fr_1fr]">
          <div>
            <Link
              href="/"
              className="flex w-fit items-center gap-2.5 font-display text-xl font-bold tracking-tight"
            >
              <BrandMark />
              Kivori
            </Link>
            <p className="mt-3 max-w-xs text-sm text-muted-foreground">
              A desk buddy with a knob. No account, no cloud.
            </p>
          </div>
          {FOOTER_NAV.map((column) => (
            <nav key={column.title} aria-label={column.title}>
              <p className="pixel text-xs text-mint">{column.title}</p>
              <ul className="mt-4 space-y-2.5 text-sm text-muted-foreground">
                {column.links.map((link) => (
                  <li key={link.href}>
                    <Link href={link.href} className="transition-colors hover:text-foreground">
                      {link.label}
                    </Link>
                  </li>
                ))}
              </ul>
            </nav>
          ))}
        </div>
        <div className="mt-12 border-t border-border pt-6 text-sm text-muted-foreground">
          <p>&copy; {new Date().getFullYear()} Cunilab</p>
        </div>
      </div>
    </footer>
  );
}
