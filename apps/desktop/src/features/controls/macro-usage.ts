import type { ConfigDto, ProfileConfigDto, SlotDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';

const g = strings.controls.gestures;

export interface MacroUsage {
  profileName: string;
  controlName: string;
}

/** Bindings a macro can sit on. Knob Hold is fixed. */
const CONTROLS: { name: string; slot: (profile: ProfileConfigDto) => SlotDto }[] = [
  { name: g.press.name, slot: (profile) => profile.press },
  { name: g.doublePress.name, slot: (profile) => profile.doublePress },
  { name: g.button1.name, slot: (profile) => profile.buttons[0].press },
  { name: g.button1Hold.name, slot: (profile) => profile.buttons[0].hold },
  { name: g.button2.name, slot: (profile) => profile.buttons[1].press },
  { name: g.button2Hold.name, slot: (profile) => profile.buttons[1].hold },
  { name: g.button3.name, slot: (profile) => profile.buttons[2].press },
  { name: g.button3Hold.name, slot: (profile) => profile.buttons[2].hold },
];

/** Profiles and controls whose saved action runs this macro. */
export function macroUsage(config: ConfigDto, id: string): MacroUsage[] {
  const found: MacroUsage[] = [];
  for (const profile of config.profiles) {
    for (const control of CONTROLS) {
      const slot = control.slot(profile);
      if (slot.action?.kind === 'macro' && slot.action.id === id) {
        found.push({ profileName: profile.name, controlName: control.name });
      }
    }
  }
  return found;
}
