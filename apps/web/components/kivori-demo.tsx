'use client';

import { useCallback, useEffect, useRef, useState, type ReactElement } from 'react';
import {
  APPS,
  AUTOPLAY,
  AUTOPLAY_LOOP_MS,
  DETENT_DEG,
  INITIAL_STATE,
  JOINED_EVENT,
  WHEEL_PER_DETENT,
  celebrate,
  drainDetents,
  hold,
  press,
  pressKey,
  switchApp,
  turn,
  type DemoState,
  type Flash,
  type Outcome,
  type ProfileId,
  type Readout,
  type ScriptAction,
} from '@/lib/demo';
import {
  FALLBACK_BG,
  SCREEN_NAMES,
  drawScreen,
  sampleBackground,
  type ScreenImages,
} from '@/lib/demo-canvas';
import './kivori-demo.css';

const HOLD_MS = 650;
const FLASH_MS = 1300;
const MOVE_TOLERANCE_DEG = 6;

const FIRST_SAY: Readout = {
  lead: 'Turn the knob',
  rest: 'or press a key. Kivori shows what happened, and how sure it is.',
};

interface Drag {
  last: number;
  acc: number;
  moved: number;
  startedAt: number;
  held: boolean;
  holdTimer: ReturnType<typeof setTimeout>;
}

/**
 * The device that reacts to you. State lives in a ref and the canvas is redrawn imperatively
 * (no re-render per frame); React state only carries what the page text needs. The idle autoplay
 * runs until the first touch and is skipped under prefers-reduced-motion.
 */
export function KivoriDemo(): ReactElement {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const dialRef = useRef<HTMLDivElement>(null);
  const knobRef = useRef<HTMLDivElement>(null);
  const stateRef = useRef<DemoState>(INITIAL_STATE);
  const angleRef = useRef(0);
  const touchedRef = useRef(false);
  const timersRef = useRef<ReturnType<typeof setTimeout>[]>([]);
  const revertRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const flashRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const redrawRef = useRef<() => void>(() => undefined);

  const [profile, setProfile] = useState<ProfileId>('general');
  const [volume, setVolume] = useState(INITIAL_STATE.vol);
  const [say, setSay] = useState<Readout>(FIRST_SAY);
  const [light, setLight] = useState<{ on: boolean; kind: Flash }>({ on: false, kind: 'ok' });
  const [quiet, setQuiet] = useState(false);
  const [downKey, setDownKey] = useState<number | null>(null);

  // Applies a pure Outcome: new state, status light, readout and the revert-to-buddy timer.
  const apply = useCallback((outcome: Outcome): void => {
    stateRef.current = outcome.state;
    setProfile(outcome.state.profile);
    setVolume(outcome.state.vol);
    setSay(outcome.say);
    redrawRef.current();
    clearTimeout(revertRef.current);
    if (outcome.revertMs !== null) {
      revertRef.current = setTimeout(() => {
        stateRef.current = { ...stateRef.current, view: 'buddy' };
        redrawRef.current();
      }, outcome.revertMs);
    }
    if (outcome.flash) {
      setLight({ on: true, kind: outcome.flash });
      clearTimeout(flashRef.current);
      flashRef.current = setTimeout(() => setLight((l) => ({ ...l, on: false })), FLASH_MS);
    }
  }, []);

  const touch = useCallback((): void => {
    if (touchedRef.current) return;
    touchedRef.current = true;
    timersRef.current.forEach(clearTimeout);
    setQuiet(true);
  }, []);

  const doTurn = useCallback(
    (dir: 1 | -1): void => {
      angleRef.current += dir * DETENT_DEG;
      if (dialRef.current) dialRef.current.style.transform = `rotate(${angleRef.current}deg)`;
      apply(turn(stateRef.current, dir));
    },
    [apply],
  );
  const doPress = useCallback((): void => apply(press(stateRef.current)), [apply]);
  const doHold = useCallback((): void => apply(hold(stateRef.current)), [apply]);
  const doKey = useCallback(
    (index: 0 | 1 | 2): void => {
      setDownKey(index);
      setTimeout(() => setDownKey(null), 140);
      apply(pressKey(stateRef.current, index));
    },
    [apply],
  );
  const doApp = useCallback(
    (id: ProfileId): void => apply(switchApp(stateRef.current, id)),
    [apply],
  );

  // Canvas, fonts and frames.
  useEffect(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;
    const images: ScreenImages = {};
    let background = FALLBACK_BG;
    const family =
      getComputedStyle(canvas).getPropertyValue('--font-silkscreen').trim() ||
      'Silkscreen, ui-monospace, monospace';
    const time = (): string =>
      new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
    const redraw = (): void =>
      drawScreen({ ctx, images, background, family, time: time() }, stateRef.current);
    redrawRef.current = redraw;
    redraw();
    let alive = true;
    for (const name of SCREEN_NAMES) {
      const image = new Image();
      image.onload = () => {
        if (!alive) return;
        if (name === 'mood-idle') background = sampleBackground(image);
        redraw();
      };
      image.src = `/media/screens/${name}.webp`;
      images[name] = image;
    }
    void document.fonts
      ?.load(`700 16px ${family}`)
      .then(() => alive && redraw())
      .catch(() => undefined);
    return () => {
      alive = false;
      redrawRef.current = () => undefined;
    };
  }, []);

  // Wheel needs a non-passive listener to stop the page from scrolling while the pointer is on the knob.
  useEffect(() => {
    const knob = knobRef.current;
    if (!knob) return;
    let acc = 0;
    const onWheel = (event: WheelEvent): void => {
      event.preventDefault();
      touch();
      acc -= event.deltaY;
      const { steps, rest } = drainDetents(acc, WHEEL_PER_DETENT);
      acc = rest;
      for (let i = 0; i < Math.abs(steps); i++) doTurn(steps > 0 ? 1 : -1);
    };
    knob.addEventListener('wheel', onWheel, { passive: false });
    return () => knob.removeEventListener('wheel', onWheel);
  }, [doTurn, touch]);

  // Idle demo: plays on a loop until the visitor touches anything.
  useEffect(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const timers = timersRef.current;
    const run = (action: ScriptAction): void => {
      if (action.kind === 'turn') doTurn(action.dir);
      else if (action.kind === 'app') doApp(action.profile);
      else doKey(action.index);
    };
    const script = (): void => {
      for (const [ms, action] of AUTOPLAY) {
        timers.push(setTimeout(() => !touchedRef.current && run(action), ms));
      }
      timers.push(setTimeout(() => !touchedRef.current && script(), AUTOPLAY_LOOP_MS));
    };
    timers.push(setTimeout(script, 1200));
    return () => timers.forEach(clearTimeout);
  }, [doApp, doKey, doTurn]);

  // A waitlist signup elsewhere on the page makes the buddy celebrate.
  useEffect(() => {
    const onJoined = (): void => {
      touch();
      apply(celebrate(stateRef.current));
    };
    window.addEventListener(JOINED_EVENT, onJoined);
    return () => {
      window.removeEventListener(JOINED_EVENT, onJoined);
      clearTimeout(revertRef.current);
      clearTimeout(flashRef.current);
    };
  }, [apply, touch]);

  // Knob pointer: drag to turn, tap to press, long press to hold.
  const dragRef = useRef<Drag | null>(null);
  const centre = (): [number, number] => {
    const rect = knobRef.current?.getBoundingClientRect();
    return rect ? [rect.left + rect.width / 2, rect.top + rect.height / 2] : [0, 0];
  };
  const onPointerDown = (event: React.PointerEvent<HTMLDivElement>): void => {
    touch();
    event.currentTarget.setPointerCapture(event.pointerId);
    const [cx, cy] = centre();
    const holdTimer = setTimeout(() => {
      const drag = dragRef.current;
      if (drag && drag.moved < MOVE_TOLERANCE_DEG) {
        drag.held = true;
        doHold();
      }
    }, HOLD_MS);
    dragRef.current = {
      last: Math.atan2(event.clientY - cy, event.clientX - cx),
      acc: 0,
      moved: 0,
      startedAt: performance.now(),
      held: false,
      holdTimer,
    };
  };
  const onPointerMove = (event: React.PointerEvent<HTMLDivElement>): void => {
    const drag = dragRef.current;
    if (!drag) return;
    const [cx, cy] = centre();
    const angle = Math.atan2(event.clientY - cy, event.clientX - cx);
    let delta = angle - drag.last;
    if (delta > Math.PI) delta -= 2 * Math.PI;
    if (delta < -Math.PI) delta += 2 * Math.PI;
    drag.last = angle;
    const degrees = (delta * 180) / Math.PI;
    drag.acc += degrees;
    drag.moved += Math.abs(degrees);
    const { steps, rest } = drainDetents(drag.acc);
    drag.acc = rest;
    for (let i = 0; i < Math.abs(steps); i++) doTurn(steps > 0 ? 1 : -1);
  };
  const onPointerUp = (): void => {
    const drag = dragRef.current;
    if (!drag) return;
    clearTimeout(drag.holdTimer);
    if (
      !drag.held &&
      drag.moved < MOVE_TOLERANCE_DEG &&
      performance.now() - drag.startedAt < HOLD_MS
    ) {
      doPress();
    }
    dragRef.current = null;
  };
  const onPointerCancel = (): void => {
    if (dragRef.current) clearTimeout(dragRef.current.holdTimer);
    dragRef.current = null;
  };
  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
    const { key } = event;
    if (key === 'ArrowUp' || key === 'ArrowRight') {
      event.preventDefault();
      touch();
      doTurn(1);
    } else if (key === 'ArrowDown' || key === 'ArrowLeft') {
      event.preventDefault();
      touch();
      doTurn(-1);
    } else if (key === 'Enter' || key === ' ') {
      event.preventDefault();
      touch();
      doPress();
    } else if (key === 'm' || key === 'M') {
      touch();
      doHold();
    }
  };

  const keyLabels = [
    'Left key: previous track',
    'Middle key: play or pause',
    'Right key: next track',
  ];

  return (
    <div className="kd" aria-label="Interactive Kivori" role="group">
      <div className="kd-stagelight" aria-hidden="true" />
      <div className="kd-device">
        <div className="kd-well">
          <div
            ref={knobRef}
            className="kd-knob"
            role="slider"
            tabIndex={0}
            aria-label="Knob. Turn to change volume, press to play or pause, hold to mute."
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={volume}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerCancel}
            onKeyDown={onKeyDown}
          >
            <div className="kd-grip" />
            <div ref={dialRef} className="kd-dial">
              <div className="kd-notch" />
            </div>
          </div>
          <div className="kd-hint" data-quiet={quiet} aria-hidden="true">
            turn me<span className="kd-more">&nbsp;· press me</span>
          </div>
        </div>
        <div className="kd-visor" />
        <div className="kd-bezel">
          <canvas
            ref={canvasRef}
            className="kd-screen"
            width={480}
            height={480}
            aria-hidden="true"
          />
          <div className="kd-light" data-on={light.on} data-kind={light.kind} aria-hidden="true">
            {light.kind === 'ok' ? '✓' : '?'}
          </div>
        </div>
        <div className="kd-keys">
          {([0, 1, 2] as const).map((index) => (
            <button
              key={index}
              type="button"
              className="kd-key"
              data-down={downKey === index}
              aria-label={keyLabels[index]}
              onClick={() => {
                touch();
                doKey(index);
              }}
            />
          ))}
        </div>
      </div>

      <div className="kd-apps">
        <div className="kd-apps-label">Pretend you switched to&hellip;</div>
        <div className="kd-taskbar" role="group" aria-label="App in front">
          {APPS.map((app) => (
            <button
              key={app.id}
              type="button"
              className="kd-app"
              aria-pressed={profile === app.id}
              onClick={() => {
                touch();
                doApp(app.id);
              }}
            >
              <i style={{ background: app.color }}>{app.letter}</i>
              {app.label}
            </button>
          ))}
        </div>
        <p className="kd-readout" aria-live="polite">
          <b>{say.lead}</b> {say.rest}
        </p>
      </div>
    </div>
  );
}
