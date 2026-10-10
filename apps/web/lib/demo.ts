// Pure logic behind the interactive Kivori on the home page (components/kivori-demo.tsx): detents,
// profiles, what the screen shows and what the readout says. No DOM here, so it is unit-tested.
// Everything mirrors what the real device does at a selling-point level; nothing is sent anywhere.

export type ProfileId = 'general' | 'browser' | 'code' | 'music' | 'meeting';
export type View = 'buddy' | 'happy' | 'volume' | 'tabs' | 'media' | 'clock' | 'system';
export type MoodName = 'mood-idle' | 'mood-happy' | 'mood-listening' | 'mood-attentive';
/** Status light: green check (Confirmed) or amber question mark (Unverified). */
export type Flash = 'ok' | 'unverified';

/** Window event the waitlist form fires after a successful signup; the demo buddy celebrates. */
export const JOINED_EVENT = 'kivori:joined';

export const TAB_COUNT = 6;
/** One detent is this many degrees of knob travel. */
export const DETENT_DEG = 15;
/** Volume points per detent. */
export const VOLUME_STEP = 2;

export interface DemoState {
  profile: ProfileId;
  vol: number;
  muted: boolean;
  playing: boolean;
  tab: number;
  view: View;
}

export interface Readout {
  lead: string;
  rest: string;
}

export interface Outcome {
  state: DemoState;
  flash: Flash | null;
  say: Readout;
  /** Milliseconds until the screen returns to the buddy; null when the view is the buddy already. */
  revertMs: number | null;
}

export const INITIAL_STATE: DemoState = {
  profile: 'general',
  vol: 42,
  muted: false,
  playing: false,
  tab: 2,
  view: 'buddy',
};

export interface ProfileInfo {
  /** Profile name drawn top left of the screen; empty for the default profile. */
  label: string;
  /** What turning the knob does here. */
  turn: string;
}

export const PROFILES: Record<ProfileId, ProfileInfo> = {
  general: { label: '', turn: 'Volume' },
  browser: { label: 'BROWSER', turn: 'Tabs' },
  code: { label: 'CODE', turn: 'Volume' },
  music: { label: 'MEDIA', turn: 'Volume' },
  meeting: { label: 'ZOOM', turn: 'Volume' },
};

export const APPS: readonly { id: ProfileId; label: string; letter: string; color: string }[] = [
  { id: 'general', label: 'Desktop', letter: 'D', color: '#8f9bb0' },
  { id: 'browser', label: 'Browser', letter: 'B', color: '#4fb7ff' },
  { id: 'code', label: 'Code', letter: 'C', color: '#b69cff' },
  { id: 'music', label: 'Music', letter: 'M', color: '#3cc47c' },
  { id: 'meeting', label: 'Meeting', letter: 'Z', color: '#f5a524' },
];

export const clamp = (value: number, min: number, max: number): number =>
  Math.max(min, Math.min(max, value));

/** Volume after one detent in `dir` (+1 clockwise, -1 counter-clockwise), kept within 0..100. */
export function detentVolume(vol: number, dir: number): number {
  return clamp(vol + Math.sign(dir) * VOLUME_STEP, 0, 100);
}

/** Next browser tab after one detent, wrapping around the strip. */
export function detentTab(tab: number, dir: number): number {
  return (((tab + Math.sign(dir)) % TAB_COUNT) + TAB_COUNT) % TAB_COUNT;
}

/** The buddy's face for the current state. */
export function moodFor(state: DemoState): MoodName {
  if (state.view === 'happy') return 'mood-happy';
  if (state.profile === 'meeting') return 'mood-attentive';
  if (state.playing || state.profile === 'music') return 'mood-listening';
  return 'mood-idle';
}

export interface ScreenLabels {
  profileLabel: string;
  muted: boolean;
  columns: readonly (readonly [x: number, heading: string, value: string])[];
}

/** The profile name, MUTED tag and the TURN / PRESS / HOLD labels under the buddy. */
export function screenLabels(state: DemoState): ScreenLabels {
  const info = PROFILES[state.profile];
  return {
    profileLabel: info.label,
    muted: state.muted,
    columns: [
      [40, 'TURN', info.turn],
      [120, 'PRESS', 'Play/Pause'],
      [200, 'HOLD', 'Mute'],
    ],
  };
}

const settle = (
  state: DemoState,
  view: View,
  ms: number,
): { state: DemoState; revertMs: number } => ({
  state: { ...state, view },
  revertMs: ms,
});

/** One detent of the knob. In a browser it flips tabs (amber: shortcuts cannot be read back). */
export function turn(state: DemoState, dir: number): Outcome {
  if (state.profile === 'browser') {
    const next = settle({ ...state, tab: detentTab(state.tab, dir) }, 'tabs', 1300);
    return {
      ...next,
      flash: 'unverified',
      say: {
        lead: 'Next tab.',
        rest: 'In your browser the knob flips tabs. Shortcuts get an amber ?: Kivori sent the keys but cannot see the result.',
      },
    };
  }
  const vol = detentVolume(state.vol, dir);
  return {
    ...settle({ ...state, muted: false, vol }, 'volume', 1300),
    flash: 'ok',
    say: {
      lead: `Volume ${vol}%.`,
      rest: 'Green check: Kivori read the volume back from your computer, so it knows it worked.',
    },
  };
}

/** Press the knob or the middle key: play or pause. */
export function press(state: DemoState): Outcome {
  const playing = !state.playing;
  if (playing) {
    return {
      ...settle({ ...state, playing }, 'media', 1500),
      flash: 'unverified',
      say: {
        lead: 'Playing.',
        rest: 'Media keys get an amber ? because apps do not always report back. Kivori never pretends.',
      },
    };
  }
  return {
    state: { ...state, playing, view: 'buddy' },
    flash: 'unverified',
    revertMs: null,
    say: { lead: 'Paused.', rest: 'The buddy stops listening.' },
  };
}

/** Hold the knob: mute or unmute. */
export function hold(state: DemoState): Outcome {
  const muted = !state.muted;
  return {
    ...settle({ ...state, muted }, 'volume', 1300),
    flash: 'ok',
    say: muted
      ? { lead: 'Muted.', rest: 'Hold the knob again to unmute.' }
      : { lead: 'Unmuted.', rest: `Back to ${state.vol}%.` },
  };
}

/** The three keys: left = previous, middle = play or pause, right = next. */
export function pressKey(state: DemoState, index: 0 | 1 | 2): Outcome {
  if (index === 1) return press(state);
  return {
    ...settle({ ...state, playing: true }, 'media', 1300),
    flash: 'unverified',
    say: {
      lead: `${index === 0 ? 'Previous' : 'Next'} track.`,
      rest: 'Three keys, your rules: set each one per app in the Kivori app.',
    },
  };
}

const APP_SAY: Record<ProfileId, Readout> = {
  general: {
    lead: 'Back on the desktop.',
    rest: 'Knob does volume, press plays or pauses, hold mutes.',
  },
  browser: { lead: 'Browser in front.', rest: 'The screen now says TURN: Tabs. Try the knob.' },
  code: {
    lead: 'Code editor in front.',
    rest: 'Kivori switched profile. Give the keys your own shortcuts.',
  },
  music: { lead: 'Music in front.', rest: 'The buddy settles in to listen.' },
  meeting: { lead: 'Meeting in front.', rest: 'The buddy puts on its meeting face.' },
};

/** "Pretend you switched to...": a new app in front, so a new profile. */
export function switchApp(state: DemoState, profile: ProfileId): Outcome {
  return {
    state: { ...state, profile, view: 'buddy' },
    flash: null,
    revertMs: null,
    say: APP_SAY[profile],
  };
}

/** A waitlist signup went through: the buddy celebrates with a happy face and a green check. */
export function celebrate(state: DemoState): Outcome {
  return {
    ...settle(state, 'happy', 2200),
    flash: 'ok',
    say: { lead: 'You are on the list.', rest: 'The buddy is thrilled.' },
  };
}

/** Wheel pixels needed for one detent. */
export const WHEEL_PER_DETENT = 40;

/** Splits accumulated knob travel (degrees) into whole detents and the remainder. */
export function drainDetents(
  acc: number,
  unit: number = DETENT_DEG,
): { steps: number; rest: number } {
  const steps = Math.trunc(acc / unit);
  return { steps, rest: acc - steps * unit };
}

/** The idle autoplay script: [delay ms, action] pairs; it stops for good on the first touch. */
export type ScriptAction =
  | { kind: 'turn'; dir: 1 | -1 }
  | { kind: 'app'; profile: ProfileId }
  | { kind: 'key'; index: 0 | 1 | 2 };

export const AUTOPLAY: readonly (readonly [ms: number, action: ScriptAction])[] = [
  [900, { kind: 'turn', dir: 1 }],
  [1100, { kind: 'turn', dir: 1 }],
  [1300, { kind: 'turn', dir: 1 }],
  [2600, { kind: 'app', profile: 'browser' }],
  [3600, { kind: 'turn', dir: 1 }],
  [4300, { kind: 'turn', dir: 1 }],
  [5800, { kind: 'app', profile: 'meeting' }],
  [7400, { kind: 'app', profile: 'music' }],
  [8600, { kind: 'key', index: 2 }],
  [10400, { kind: 'app', profile: 'general' }],
];
export const AUTOPLAY_LOOP_MS = 13000;
