import { describe, expect, it } from 'vitest';
import { mockGetConfig } from '@/lib/ipc/mock';
import type { ActionSpec, ConfigDto, SlotDto } from '@/lib/ipc/types';
import { needsKeyEvents } from '../needs-key-events';

const slot = (action: ActionSpec | null, overridden = true): SlotDto => ({
  action,
  label: null,
  deviceLabel: '',
  overridden,
});

function config(change: (c: ConfigDto) => void): ConfigDto {
  const c = structuredClone(mockGetConfig());
  change(c);
  return c;
}

describe('needsKeyEvents', () => {
  it('is false for the default bindings, even though built-in profiles hold shortcuts', () => {
    expect(needsKeyEvents(mockGetConfig())).toBe(false);
    expect(needsKeyEvents(null)).toBe(false);
  });

  it('is true once the user binds a shortcut', () => {
    expect(
      needsKeyEvents(
        config((c) => {
          c.profiles[0]!.press = slot({ kind: 'shortcut', keys: 'Ctrl+M' });
        }),
      ),
    ).toBe(true);
  });

  it('stays false for media, mute and launch bindings', () => {
    expect(
      needsKeyEvents(
        config((c) => {
          c.profiles[0]!.press = slot({ kind: 'playPause' });
          c.profiles[0]!.hold = slot({ kind: 'launch', target: 'notes' });
        }),
      ),
    ).toBe(false);
  });

  it('counts a shortcut knob the user chose, and a macro that sends a shortcut', () => {
    expect(
      needsKeyEvents(
        config((c) => {
          c.profiles[0]!.rotate = {
            spec: { kind: 'shortcuts', cw: 'Ctrl+]', ccw: 'Ctrl+[', label: 'Nav' },
            deviceLabel: 'Nav',
            overridden: true,
          };
        }),
      ),
    ).toBe(true);
    expect(
      needsKeyEvents(
        config((c) => {
          c.macros = [
            {
              id: 'm1',
              name: 'M',
              steps: [{ kind: 'action', action: { kind: 'shortcut', keys: 'Ctrl+M' } }],
            },
          ];
          c.profiles[0]!.hold = slot({ kind: 'macro', id: 'm1' });
        }),
      ),
    ).toBe(true);
  });

  it('ignores a built-in profile slot the user did not change', () => {
    expect(
      needsKeyEvents(
        config((c) => {
          c.profiles[0]!.press = slot({ kind: 'shortcut', keys: 'Ctrl+M' }, false);
        }),
      ),
    ).toBe(false);
  });
});
