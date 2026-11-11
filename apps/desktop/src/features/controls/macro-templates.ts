import type { StartupPlatform, StepSpec } from '@/lib/ipc/types';

export interface MacroTemplate {
  key: 'quiet' | 'copyPaste' | 'selectCopy';
  steps: StepSpec[];
}

/**
 * Shortcut token for the primary modifier. Built-in profiles and the key recorder store Command
 * as `Meta` (`Ctrl` on Windows and Linux). The parser accepts `Command` as the same modifier.
 */
function modifier(platform: StartupPlatform | null): 'Ctrl' | 'Meta' {
  return platform === 'macos' ? 'Meta' : 'Ctrl';
}

function shortcut(keys: string): StepSpec {
  return { kind: 'action', action: { kind: 'shortcut', keys } };
}

function wait(ms: number): StepSpec {
  return { kind: 'delay', ms };
}

/** The three starter macros. Pass the startup platform so copy and paste use the right modifier. */
export function macroTemplates(platform: StartupPlatform | null): MacroTemplate[] {
  const mod = modifier(platform);
  return [
    {
      key: 'quiet',
      steps: [
        { kind: 'action', action: { kind: 'micMute' } },
        { kind: 'action', action: { kind: 'systemMute' } },
      ],
    },
    {
      key: 'copyPaste',
      steps: [shortcut(`${mod}+C`), wait(150), shortcut(`${mod}+V`)],
    },
    {
      key: 'selectCopy',
      steps: [shortcut(`${mod}+A`), wait(100), shortcut(`${mod}+C`)],
    },
  ];
}

/** Windows, Linux, and unknown platforms. Use `macroTemplates('macos')` for Command (`Meta`). */
export const MACRO_TEMPLATES: MacroTemplate[] = macroTemplates(null);
