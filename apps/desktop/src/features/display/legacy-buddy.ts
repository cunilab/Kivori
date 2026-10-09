import { setBuddySettings } from '@/lib/ipc';
import type { ConfigDto, Intensity } from '@/lib/ipc/types';

// Before the config file existed the buddy settings lived in the webview's localStorage.
const PERSONALITY_KEY = 'kivori.mascot.personality';
const SELF_PLAY_KEY = 'kivori.mascot.selfPlay';
const LEGACY_INTENSITY: Record<string, Intensity> = {
  calm: 'low',
  cozy: 'normal',
  playful: 'high',
};

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function remove(...keys: string[]): void {
  try {
    for (const key of keys) localStorage.removeItem(key);
  } catch {
    // Storage may be blocked; the settings then simply stay where they were.
  }
}

/**
 * Moves the old localStorage buddy settings into the config once. They are copied only when the
 * config loaded cleanly (`notice === null`); the keys go away after a successful save, or at once
 * when the config was recovered or migrated (the old values must not override that outcome). A
 * failed save keeps the keys so the next launch tries again.
 */
export function migrateLegacyBuddy(config: ConfigDto): void {
  const personality = read(PERSONALITY_KEY);
  const selfPlay = read(SELF_PLAY_KEY);
  if (personality === null && selfPlay === null) return;
  if (config.notice !== null) {
    remove(PERSONALITY_KEY, SELF_PLAY_KEY);
    return;
  }
  const reactions = selfPlay !== 'false';
  const intensity = (personality && LEGACY_INTENSITY[personality]) || 'normal';
  if (reactions === config.buddy.reactions && intensity === config.buddy.intensity) {
    remove(PERSONALITY_KEY, SELF_PLAY_KEY);
    return;
  }
  void setBuddySettings(reactions, intensity)
    .then(() => remove(PERSONALITY_KEY, SELF_PLAY_KEY))
    .catch(() => {});
}
