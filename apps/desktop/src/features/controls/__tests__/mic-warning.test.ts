import { describe, expect, it } from 'vitest';
import { mockGetConfig } from '@/lib/ipc/mock';
import type { ProfileConfigDto, SlotDto } from '@/lib/ipc/types';
import { anySlotMutesMic } from '../mic-warning';

const unbound = (): SlotDto => ({ action: null, label: null, deviceLabel: '', overridden: false });

function cleared(change?: (profile: ProfileConfigDto) => void): ProfileConfigDto[] {
  return structuredClone(mockGetConfig().profiles).map((profile) => {
    const next = {
      ...profile,
      press: unbound(),
      hold: unbound(),
      doublePress: unbound(),
      buttons: profile.buttons.map(() => ({ press: unbound(), hold: unbound() })),
    } as ProfileConfigDto;
    change?.(next);
    return next;
  });
}

describe('anySlotMutesMic', () => {
  it('is true for the defaults and false only when no slot is mic mute', () => {
    expect(anySlotMutesMic(mockGetConfig().profiles)).toBe(true);
    expect(anySlotMutesMic(cleared())).toBe(false);
    expect(
      anySlotMutesMic(
        cleared((profile) => {
          if (profile.id === 'code')
            profile.doublePress = { ...unbound(), action: { kind: 'micMute' } };
        }),
      ),
    ).toBe(true);
    expect(
      anySlotMutesMic(
        cleared((profile) => {
          if (profile.id === 'general')
            profile.press = { ...unbound(), action: { kind: 'playPause' } };
        }),
      ),
    ).toBe(false);
  });
});
