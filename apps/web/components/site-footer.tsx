import Image from 'next/image';
import Link from 'next/link';
import type { ReactElement } from 'react';
import mascot from '@brand/mascot.svg';
import { ThemeToggle } from '@/components/theme-toggle';
import { FOOTER_NAV } from '@/lib/nav';

export function SiteFooter(): ReactElement {
  return (
    <footer className="border-t border-border bg-surface">
      <div className="mx-auto max-w-[1200px] px-5 py-14 sm:px-8">
        <div className="grid gap-10 sm:grid-cols-[1.5fr_1fr_1fr]">
          <div>
            <Link href="/" className="flex w-fit items-center gap-2 text-lg font-semibold">
              <Image src={mascot} alt="" width={28} height={28} />
              Kivori
            </Link>
            <p className="mt-3 max-w-xs text-sm text-muted-foreground">
              The desk buddy that works.
            </p>
          </div>
          {FOOTER_NAV.map((column) => (
            <nav key={column.title} aria-label={column.title}>
              <p className="text-sm font-semibold">{column.title}</p>
              <ul className="mt-3 space-y-2 text-sm text-muted-foreground">
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
        <div className="mt-12 flex items-center justify-between border-t border-border pt-6 text-sm text-muted-foreground">
          <p>&copy; {new Date().getFullYear()} Cunilab</p>
          <ThemeToggle className="-mr-2" />
        </div>
      </div>
    </footer>
  );
}
