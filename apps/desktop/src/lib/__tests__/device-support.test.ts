import { describe, expect, it } from 'vitest';
import { canMirrorState, hasCapability } from '../device-support';
import { connectionFixture } from '../../test/connection';

describe('current device support', () => {
  it('never trusts retained capabilities while disconnected, pending or unknown', () => {
    const known = connectionFixture();
    for (const connection of [
      null,
      { ...known, connection: 'disconnected' as const },
      { ...known, device: { ...known.device!, identity: null } },
      {
        ...known,
        device: { ...known.device!, identity: { ...known.device!.identity!, known: false } },
      },
      connectionFixture(['productIdentityV1']),
    ]) {
      expect(hasCapability(connection, 'mascotInteraction')).toBe(false);
      expect(canMirrorState(connection)).toBe(false);
    }
  });
  it('preserves baseline legacy sync while keeping capability-specific features separate', () => {
    const legacy = connectionFixture([], {
      device: {
        ...connectionFixture().device!,
        identity: {
          ...connectionFixture().device!.identity!,
          source: 'legacy',
        },
      },
    });
    expect(canMirrorState(legacy)).toBe(true);
    expect(hasCapability(legacy, 'mascotInteraction')).toBe(false);
    const screen = connectionFixture(['deskStatusV1']);
    expect(hasCapability(screen, 'deskStatusV1')).toBe(true);
    expect(hasCapability(screen, 'mascotInteraction')).toBe(false);
    expect(hasCapability(connectionFixture(['idleLifeV1']), 'mascotInteraction')).toBe(false);
  });
});
