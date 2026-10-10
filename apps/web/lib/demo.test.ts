import { describe, expect, it } from 'vitest';
import {
  INITIAL_STATE,
  celebrate,
  detentTab,
  detentVolume,
  drainDetents,
  hold,
  moodFor,
  press,
  pressKey,
  screenLabels,
  switchApp,
  turn,
  type DemoState,
} from './demo';

const at = (patch: Partial<DemoState>): DemoState => ({ ...INITIAL_STATE, ...patch });

describe('detents', () => {
  it('moves volume by 2 per detent and clamps at both ends', () => {
    expect(detentVolume(42, 1)).toBe(44);
    expect(detentVolume(42, -1)).toBe(40);
    expect(detentVolume(99, 1)).toBe(100);
    expect(detentVolume(1, -1)).toBe(0);
  });

  it('wraps the browser tab strip', () => {
    expect(detentTab(5, 1)).toBe(0);
    expect(detentTab(0, -1)).toBe(5);
    expect(detentTab(2, 1)).toBe(3);
  });

  it('drains whole detents from accumulated degrees', () => {
    expect(drainDetents(31)).toEqual({ steps: 2, rest: 1 });
    expect(drainDetents(-31)).toEqual({ steps: -2, rest: -1 });
    expect(drainDetents(10)).toEqual({ steps: 0, rest: 10 });
  });
});

describe('profiles and mood', () => {
  it('picks the mood from the profile, playback and view', () => {
    expect(moodFor(INITIAL_STATE)).toBe('mood-idle');
    expect(moodFor(at({ profile: 'meeting' }))).toBe('mood-attentive');
    expect(moodFor(at({ profile: 'music' }))).toBe('mood-listening');
    expect(moodFor(at({ playing: true }))).toBe('mood-listening');
    expect(moodFor(at({ profile: 'meeting', view: 'happy' }))).toBe('mood-happy');
  });

  it('labels the screen per profile', () => {
    expect(screenLabels(at({ profile: 'browser' })).profileLabel).toBe('BROWSER');
    expect(screenLabels(at({ profile: 'browser' })).columns[0]).toEqual([40, 'TURN', 'Tabs']);
    expect(screenLabels(INITIAL_STATE).columns[0]).toEqual([40, 'TURN', 'Volume']);
    expect(screenLabels(at({ muted: true })).muted).toBe(true);
  });
});

describe('view state machine', () => {
  it('turning shows the volume gauge with a green check, and unmutes', () => {
    const out = turn(at({ muted: true }), 1);
    expect(out.state).toMatchObject({ vol: 44, muted: false, view: 'volume' });
    expect(out.flash).toBe('ok');
    expect(out.revertMs).toBe(1300);
  });

  it('turning in a browser flips tabs with an amber question mark', () => {
    const out = turn(at({ profile: 'browser', tab: 5 }), 1);
    expect(out.state).toMatchObject({ tab: 0, view: 'tabs', vol: 42 });
    expect(out.flash).toBe('unverified');
  });

  it('press toggles playback and shows media only while playing', () => {
    const on = press(INITIAL_STATE);
    expect(on.state).toMatchObject({ playing: true, view: 'media' });
    const off = press(on.state);
    expect(off.state).toMatchObject({ playing: false, view: 'buddy' });
    expect(off.revertMs).toBeNull();
  });

  it('hold toggles mute', () => {
    const muted = hold(INITIAL_STATE);
    expect(muted.state.muted).toBe(true);
    expect(muted.say.lead).toBe('Muted.');
    expect(hold(muted.state).say.rest).toBe('Back to 42%.');
  });

  it('keys: middle is play or pause, side keys skip', () => {
    expect(pressKey(INITIAL_STATE, 1).state.playing).toBe(true);
    const next = pressKey(INITIAL_STATE, 2);
    expect(next.say.lead).toBe('Next track.');
    expect(pressKey(INITIAL_STATE, 0).say.lead).toBe('Previous track.');
  });

  it('switching apps changes profile and returns to the buddy', () => {
    const out = switchApp(at({ view: 'volume' }), 'browser');
    expect(out.state).toMatchObject({ profile: 'browser', view: 'buddy' });
    expect(out.flash).toBeNull();
  });

  it('celebrates a signup with the happy face and a check', () => {
    const out = celebrate(at({ profile: 'meeting' }));
    expect(moodFor(out.state)).toBe('mood-happy');
    expect(out.flash).toBe('ok');
    expect(out.revertMs).toBe(2200);
  });
});
