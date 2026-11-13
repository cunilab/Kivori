import { describe, expect, it } from 'vitest';
import { mockGetConfig } from '@/lib/ipc/mock';
import type { ActionSpec, ConfigDto, SlotDto } from '@/lib/ipc/types';
import { macroUsage } from '../macro-usage';

const slot = (action: ActionSpec | null): SlotDto => ({
  action,
  label: null,
  deviceLabel: '',
  overridden: true,
});

function config(change: (c: ConfigDto) => void): ConfigDto {
  const c = structuredClone(mockGetConfig());
  change(c);
  return c;
}

describe('macroUsage', () => {
  it('lists the controls that run a macro and is empty when nothing binds it', () => {
    expect(macroUsage(structuredClone(mockGetConfig()), 'not-a-real-macro')).toEqual([]);

    const used = config((c) => {
      const general = c.profiles.find((p) => p.id === 'general');
      const photoshop = c.profiles.find((p) => p.id === 'photoshop');
      if (!general || !photoshop) throw new Error('missing profile');
      general.press = slot({ kind: 'macro', id: 'quiet' });
      // Built-in profiles share one buttons array until a profile replaces its own.
      general.buttons = [
        { ...general.buttons[0], hold: slot({ kind: 'macro', id: 'quiet' }) },
        { ...general.buttons[1], hold: slot({ kind: 'micMute' }) },
        general.buttons[2],
      ] as typeof general.buttons;
      photoshop.buttons[0].press = slot({ kind: 'macro', id: 'quiet' });
      photoshop.buttons[1].hold = slot({ kind: 'micMute' });
    });

    expect(macroUsage(used, 'quiet')).toEqual([
      { profileName: 'General', controlName: 'Press' },
      { profileName: 'General', controlName: 'Left button hold' },
      { profileName: 'Photoshop', controlName: 'Left button' },
    ]);
    expect(macroUsage(used, 'other')).toEqual([]);
  });
});
