import { describe, expect, it } from 'vitest';
import { MACRO_TEMPLATES, macroTemplates } from '../macro-templates';

describe('macroTemplates', () => {
  it('uses Ctrl except on macOS, where Command is stored as Meta', () => {
    const copy = (platform: 'windows' | 'linux' | 'macos' | 'other' | null) =>
      macroTemplates(platform).find((template) => template.key === 'copyPaste')?.steps;
    const select = (platform: 'windows' | 'linux' | 'macos' | 'other' | null) =>
      macroTemplates(platform).find((template) => template.key === 'selectCopy')?.steps;

    expect(MACRO_TEMPLATES).toEqual(macroTemplates(null));
    expect(copy(null)).toEqual(copy('windows'));
    expect(copy('linux')).toEqual(copy('windows'));
    expect(copy('other')).toEqual(copy('windows'));
    expect(copy('windows')).toEqual([
      { kind: 'action', action: { kind: 'shortcut', keys: 'Ctrl+C' } },
      { kind: 'delay', ms: 150 },
      { kind: 'action', action: { kind: 'shortcut', keys: 'Ctrl+V' } },
    ]);
    expect(select('windows')).toEqual([
      { kind: 'action', action: { kind: 'shortcut', keys: 'Ctrl+A' } },
      { kind: 'delay', ms: 100 },
      { kind: 'action', action: { kind: 'shortcut', keys: 'Ctrl+C' } },
    ]);
    expect(copy('macos')).toEqual([
      { kind: 'action', action: { kind: 'shortcut', keys: 'Meta+C' } },
      { kind: 'delay', ms: 150 },
      { kind: 'action', action: { kind: 'shortcut', keys: 'Meta+V' } },
    ]);
    expect(select('macos')).toEqual([
      { kind: 'action', action: { kind: 'shortcut', keys: 'Meta+A' } },
      { kind: 'delay', ms: 100 },
      { kind: 'action', action: { kind: 'shortcut', keys: 'Meta+C' } },
    ]);
    expect(macroTemplates('macos').find((template) => template.key === 'quiet')?.steps).toEqual(
      macroTemplates('windows').find((template) => template.key === 'quiet')?.steps,
    );
  });
});
