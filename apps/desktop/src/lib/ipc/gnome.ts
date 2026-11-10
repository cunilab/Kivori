import { isTauri } from './index';

const STATES = [
  'notApplicable',
  'notInstalled',
  'needsLogout',
  'working',
  'disabled',
  'unsupported',
  'unavailable',
  'updateAvailable',
  'cliMissing',
] as const;
export type GnomeFocusStatus = (typeof STATES)[number];

function parse(value: unknown): GnomeFocusStatus {
  if (typeof value === 'string' && STATES.includes(value as GnomeFocusStatus)) {
    return value as GnomeFocusStatus;
  }
  throw new Error('Invalid GNOME focus status.');
}

let mockEnabled = false;
function mockStatus(): GnomeFocusStatus {
  const requested = new URLSearchParams(window.location.search).get('gnome');
  return mockEnabled ? 'needsLogout' : requested ? parse(requested) : 'notApplicable';
}

export async function getGnomeFocusStatus(): Promise<GnomeFocusStatus> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return parse(await invoke<unknown>('get_gnome_focus_status'));
  }
  if (import.meta.env.DEV) return mockStatus();
  throw new Error('Kivori: the native runtime is unavailable.');
}

export async function enableGnomeFocus(): Promise<GnomeFocusStatus> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return parse(await invoke<unknown>('enable_gnome_focus'));
  }
  if (import.meta.env.DEV) {
    // Preview the retry state without installing anything: ?mock=connected&gnome=notInstalled&gnomeError.
    if (new URLSearchParams(window.location.search).has('gnomeError')) {
      throw new Error('Mock installation failed.');
    }
    mockEnabled = true;
    return mockStatus();
  }
  throw new Error('Kivori: the native runtime is unavailable.');
}

export async function logoutGnome(): Promise<void> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<void>('logout_gnome');
  }
  if (import.meta.env.DEV) return;
  throw new Error('Kivori: the native runtime is unavailable.');
}

/** Restrictions apply only on GNOME Wayland; the native dispatcher enforces them too. */
export async function getGnomeShortcutPolicy(keys: string): Promise<string | null> {
  if (!isTauri()) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  const value = await invoke<unknown>('gnome_shortcut_policy', { keys });
  if (value === null || typeof value === 'string') return value;
  throw new Error('Invalid keyboard policy response.');
}
