import { describe, expect, it } from 'vitest';
import { mockGetConfig } from '@/lib/ipc/mock';
import type { ProfileConfigDto } from '@/lib/ipc/types';
import { anyProfileKeepsPicker } from '../picker-warning';

describe('anyProfileKeepsPicker', () => {
  it('is true for the defaults and false only when every profile rebound Hold + turn', () => {
    expect(anyProfileKeepsPicker(mockGetConfig().profiles)).toBe(true);
    const rebound = mockGetConfig().profiles.map((profile): ProfileConfigDto => ({
      ...profile,
      heldRotate: {
        spec: { kind: 'systemVolume' },
        deviceLabel: 'Volume',
        overridden: true,
      },
    }));
    expect(anyProfileKeepsPicker(rebound)).toBe(false);
    rebound[0] = {
      ...rebound[0]!,
      heldRotate: { spec: { kind: 'profilePicker' }, deviceLabel: '', overridden: false },
    };
    expect(anyProfileKeepsPicker(rebound)).toBe(true);
  });
});
