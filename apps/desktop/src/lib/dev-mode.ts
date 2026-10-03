import { useSyncExternalStore } from 'react';

// Developer mode: a per-computer preference (localStorage), off by default. It reveals Activity,
// technical device details, buddy social reactions and, in dev builds only, Device Studio. Same
// store pattern as the theme: a module-level subscription instead of a provider.

const KEY = 'kivori.developerMode';
const listeners = new Set<() => void>();
// Used only when storage refuses a write, so the switch still works for this session.
let sessionValue: boolean | null = null;

function read(): boolean {
  if (sessionValue !== null) return sessionValue;
  try {
    return localStorage.getItem(KEY) === 'true';
  } catch {
    return false;
  }
}

export function setDevMode(on: boolean): void {
  try {
    if (on) localStorage.setItem(KEY, 'true');
    else localStorage.removeItem(KEY);
    sessionValue = null;
  } catch {
    sessionValue = on;
  }
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useDevMode(): boolean {
  return useSyncExternalStore(subscribe, read, () => false);
}
