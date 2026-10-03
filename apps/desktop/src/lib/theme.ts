import { useSyncExternalStore } from 'react';

// Light / dark / system theme, stored per OS user in localStorage and applied as the `.dark` class
// shadcn's tokens key off. A dozen lines instead of a theming dependency.

export type ThemeChoice = 'light' | 'dark' | 'system';

const KEY = 'kivori.theme';
const listeners = new Set<() => void>();
const media = (): MediaQueryList | null =>
  typeof window !== 'undefined' && window.matchMedia
    ? window.matchMedia('(prefers-color-scheme: dark)')
    : null;

function read(): ThemeChoice {
  try {
    const saved = localStorage.getItem(KEY);
    return saved === 'light' || saved === 'dark' ? saved : 'system';
  } catch {
    return 'system';
  }
}

export function resolvedTheme(choice: ThemeChoice = read()): 'light' | 'dark' {
  if (choice !== 'system') return choice;
  return media()?.matches ? 'dark' : 'light';
}

export function applyTheme(): void {
  document.documentElement.classList.toggle('dark', resolvedTheme() === 'dark');
}

export function setTheme(choice: ThemeChoice): void {
  try {
    if (choice === 'system') localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, choice);
  } catch {
    // Storage can be unavailable; the choice still applies for this session.
  }
  applyTheme();
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  const query = media();
  const onSystemChange = (): void => {
    applyTheme();
    listener();
  };
  query?.addEventListener?.('change', onSystemChange);
  return () => {
    listeners.delete(listener);
    query?.removeEventListener?.('change', onSystemChange);
  };
}

export function useTheme(): { theme: ThemeChoice; resolved: 'light' | 'dark' } {
  const theme = useSyncExternalStore(subscribe, read, () => 'system' as const);
  return { theme, resolved: resolvedTheme(theme) };
}
