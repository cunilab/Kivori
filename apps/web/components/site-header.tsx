import Image from 'next/image';
import Link from 'next/link';
import type { ReactElement } from 'react';
import icon from '@brand/icon/kivori-icon.svg';
import { ThemeToggle } from '@/components/theme-toggle';

const NAV = [
  { href: '/products', label: 'Products' },
  { href: '/download', label: 'Download' },
  { href: '/changelog', label: 'Changelog' },
  { href: '/support', label: 'Support' },
  { href: '/blog', label: 'Blog' },
] as const;

export function SiteHeader(): ReactElement {
  return (
    <header className="border-b border-border">
      <div className="mx-auto flex h-16 max-w-5xl items-center justify-between px-5">
        <Link href="/" className="flex items-center gap-2.5 font-display text-lg font-semibold">
          <Image src={icon} alt="" width={32} height={32} className="rounded-lg" />
          Kivori
        </Link>
        <div className="flex items-center gap-1 sm:gap-4">
          <nav
            aria-label="Main"
            className="flex items-center gap-3 text-sm text-muted-foreground sm:gap-5"
          >
            {NAV.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className="transition-colors hover:text-foreground"
              >
                {item.label}
              </Link>
            ))}
          </nav>
          <ThemeToggle />
        </div>
      </div>
    </header>
  );
}
