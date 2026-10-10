'use client';

import { MoonIcon, SunIcon } from 'lucide-react';
import type { ReactElement } from 'react';
import { Button } from '@kivori/ui/components/button';
import { THEME_STORAGE_KEY } from '@/app/theme-script';

/** Flips the `dark` class set by the inline init script and remembers the choice. */
export function ThemeToggle({ className }: { className?: string }): ReactElement {
  function toggle(): void {
    const root = document.documentElement;
    const next = root.classList.contains('dark') ? 'light' : 'dark';
    root.classList.toggle('dark', next === 'dark');
    root.style.colorScheme = next;
    try {
      localStorage.setItem(THEME_STORAGE_KEY, next);
    } catch {
      // storage blocked: the choice just does not persist
    }
  }

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={toggle}
      aria-label="Toggle light and dark theme"
      className={`rounded-full text-muted-foreground ${className ?? ''}`}
    >
      <SunIcon className="hidden dark:block" />
      <MoonIcon className="dark:hidden" />
    </Button>
  );
}
