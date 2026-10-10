import Link from 'next/link';
import type { ReactElement } from 'react';

export function SiteFooter(): ReactElement {
  return (
    <footer className="border-t border-border">
      <div className="mx-auto flex max-w-5xl flex-col items-center justify-between gap-3 px-5 py-8 text-sm text-muted-foreground sm:flex-row">
        <p>&copy; {new Date().getFullYear()} Cunilab</p>
        <nav aria-label="Footer" className="flex items-center gap-5">
          <Link href="/privacy" className="transition-colors hover:text-foreground">
            Privacy
          </Link>
          <Link href="/press" className="transition-colors hover:text-foreground">
            Press
          </Link>
        </nav>
      </div>
    </footer>
  );
}
