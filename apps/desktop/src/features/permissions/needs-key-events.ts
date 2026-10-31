import type { ActionSpec, ConfigDto, MacroSpec } from '@/lib/ipc/types';

/** Whether running `action` makes Kivori press keys (a shortcut, or a macro with one). */
function pressesKeys(action: ActionSpec | null, macros: readonly MacroSpec[]): boolean {
  if (!action) return false;
  if (action.kind === 'shortcut') return true;
  if (action.kind !== 'macro') return false;
  const found = macros.find((macro) => macro.id === action.id);
  return !!found?.steps.some((step) => step.kind === 'action' && step.action.kind === 'shortcut');
}

/**
 * Whether the user has chosen bindings that make Kivori press keys: a shortcut in their own
 * profile or in a slot they changed, a shortcut knob, or a macro that sends one. The built-in
 * profiles' own shortcuts (Browser, Code, Zoom, ...) do not count: they only matter while that
 * app is in front, and a failed one asks on the spot instead.
 *
 * Media keys, volume, mute and launching never press keys here, so they never count.
 */
export function needsKeyEvents(config: ConfigDto | null): boolean {
  if (!config) return false;
  return config.profiles.some((profile) => {
    const slots = [
      profile.press,
      profile.hold,
      ...profile.buttons.flatMap(({ press, hold }) => [press, hold === 'pin' ? null : hold]),
    ];
    const bound = slots.some(
      (slot) =>
        !!slot && (profile.custom || slot.overridden) && pressesKeys(slot.action, config.macros),
    );
    const knob =
      (profile.custom || profile.rotate.overridden) && profile.rotate.spec.kind === 'shortcuts';
    return bound || knob;
  });
}
