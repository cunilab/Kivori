import { describe, expect, it } from 'vitest';
import { canMirrorState, controlSupported, hasCapability, sessionState } from '../device-support';
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
  it('distinguishes setup readiness from transport connection', () => {
    const pending = connectionFixture();
    pending.device!.identity = null;
    const unknown = connectionFixture();
    unknown.device!.identity!.known = false;
    expect(sessionState(pending)).toBe('pending');
    expect(sessionState(unknown)).toBe('unknown');
    expect(sessionState(connectionFixture(['productIdentityV1']))).toBe('attention');
    expect(sessionState(connectionFixture([]))).toBe('attention');
    const legacy = connectionFixture([]);
    legacy.device!.identity!.source = 'legacy';
    expect(sessionState(legacy)).toBe('ready');
    expect(sessionState(connectionFixture(['deskStatusV1']))).toBe('ready');
    expect(sessionState(null)).toBe('offline');
    for (const connection of ['disconnected', 'connecting', 'incompatible', 'error'] as const) {
      expect(sessionState(connectionFixture(undefined, { connection }))).toBe('offline');
    }
  });

  it('uses control-specific effective capabilities, including both prerequisites for button gestures', () => {
    const dial = connectionFixture(['physicalInputV1']);
    expect(controlSupported(dial, 'rotate')).toBe(true);
    for (const control of ['press', 'hold', 'doublePress', 'button1', 'button2Hold'] as const) {
      expect(controlSupported(dial, control)).toBe(false);
    }
    const press = connectionFixture(['physicalInputV1', 'buttonInputV1']);
    expect(controlSupported(press, 'press')).toBe(true);
    expect(controlSupported(press, 'hold')).toBe(true);
    expect(controlSupported(press, 'doublePress')).toBe(false);
    expect(
      controlSupported(connectionFixture(['physicalInputV1', 'doublePressV1']), 'doublePress'),
    ).toBe(false);
    expect(
      controlSupported(
        connectionFixture(['physicalInputV1', 'buttonInputV1', 'doublePressV1']),
        'doublePress',
      ),
    ).toBe(true);
    const buttons = connectionFixture(['physicalInputV1', 'contextButtonsV1']);
    expect(controlSupported(buttons, 'button1')).toBe(true);
    expect(controlSupported(buttons, 'button2Hold')).toBe(true);
    expect(controlSupported(buttons, 'press')).toBe(false);
    expect(
      controlSupported(connectionFixture(['contextButtonsV1', 'buttonInputV1']), 'button1'),
    ).toBe(false);
    expect(controlSupported(null, 'rotate')).toBe(false);
    const unknown = connectionFixture();
    unknown.device!.identity!.known = false;
    expect(controlSupported(unknown, 'rotate')).toBe(false);
    expect(
      controlSupported(connectionFixture(undefined, { connection: 'disconnected' }), 'press'),
    ).toBe(false);
  });
});
