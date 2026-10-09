import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConfigDto } from '../../../lib/ipc/types';

const h = vi.hoisted(() => ({
  setBuddy: vi.fn<(...args: unknown[]) => Promise<void>>(() => Promise.resolve()),
}));
vi.mock('../../../lib/ipc', () => ({ setBuddySettings: h.setBuddy }));

import { migrateLegacyBuddy } from '../legacy-buddy';

const config: ConfigDto = {
  version: 1,
  revision: 0,
  notice: null,
  profiles: [],
  macros: [],
  display: { defaultView: 'buddy', secondaryView: 'system' },
  buddy: { reactions: true, intensity: 'normal' },
};
const KEYS = ['kivori.mascot.personality', 'kivori.mascot.selfPlay'];
const left = (): (string | null)[] => KEYS.map((key) => localStorage.getItem(key));

beforeEach(() => {
  h.setBuddy.mockClear().mockResolvedValue(undefined);
  localStorage.clear();
});

describe('migrateLegacyBuddy', () => {
  it('does nothing without legacy keys', () => {
    migrateLegacyBuddy(config);
    expect(h.setBuddy).not.toHaveBeenCalled();
  });

  it('saves the old settings once and removes the keys', async () => {
    localStorage.setItem(KEYS[0]!, 'playful');
    localStorage.setItem(KEYS[1]!, 'false');
    migrateLegacyBuddy(config);
    expect(h.setBuddy).toHaveBeenCalledWith(false, 'high');
    await vi.waitFor(() => expect(left()).toEqual([null, null]));
    migrateLegacyBuddy(config);
    expect(h.setBuddy).toHaveBeenCalledTimes(1);
  });

  it('maps calm and cozy and defaults self-play to on', () => {
    localStorage.setItem(KEYS[0]!, 'calm');
    migrateLegacyBuddy(config);
    expect(h.setBuddy).toHaveBeenCalledWith(true, 'low');
  });

  it('keeps the keys when the save fails, so the next launch retries', async () => {
    h.setBuddy.mockRejectedValueOnce(new Error('disk full'));
    localStorage.setItem(KEYS[0]!, 'calm');
    migrateLegacyBuddy(config);
    await vi.waitFor(() => expect(h.setBuddy).toHaveBeenCalled());
    await Promise.resolve();
    expect(left()).toEqual(['calm', null]);
  });

  it('drops the keys without saving when they already match', () => {
    localStorage.setItem(KEYS[0]!, 'cozy');
    migrateLegacyBuddy(config);
    expect(h.setBuddy).not.toHaveBeenCalled();
    expect(left()).toEqual([null, null]);
  });

  it('never overrides a recovered or migrated config', () => {
    for (const notice of ['recoveredCorrupt', 'recoveredNewerVersion', 'migrated'] as const) {
      localStorage.setItem(KEYS[0]!, 'playful');
      migrateLegacyBuddy({ ...config, notice });
      expect(left()).toEqual([null, null]);
    }
    expect(h.setBuddy).not.toHaveBeenCalled();
  });
});
