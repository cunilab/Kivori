import { useCallback, useEffect, useRef, useState } from 'react';
import { completeOnboarding, getAccessibility, getOnboarding, restartOnboarding } from '@/lib/ipc';
import type { AccessibilityState, DeskActionDto, DeskStatusDto } from '@/lib/ipc/types';

/**
 * Whether first-run setup is finished. `completed` is `null` until the native side answers, so the
 * caller can tell "not known yet" from "needs setup".
 */
export function useOnboarding(): {
  completed: boolean | null;
  complete: () => Promise<void>;
  restart: () => Promise<void>;
} {
  const [completed, setCompleted] = useState<boolean | null>(null);
  useEffect(() => {
    let active = true;
    void getOnboarding()
      .then((state) => {
        if (active) setCompleted(state.completed);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);
  const complete = useCallback(async () => {
    setCompleted((await completeOnboarding()).completed);
  }, []);
  const restart = useCallback(async () => {
    setCompleted((await restartOnboarding()).completed);
  }, []);
  return { completed, complete, restart };
}

/** How often the permission step re-reads Accessibility while the user is in System Settings. */
export const ACCESSIBILITY_POLL_MS = 1000;

/**
 * The Accessibility permission. Read once on mount (so the stepper knows whether to include the
 * permission step), then re-read every second while `watch` is true. `null` = not known yet.
 */
export function useAccessibility(watch: boolean): {
  state: AccessibilityState | null;
  refresh: (next?: AccessibilityState) => void;
} {
  const [state, setState] = useState<AccessibilityState | null>(null);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const read = useCallback(() => {
    void getAccessibility()
      .then((next) => {
        if (alive.current) setState(next);
      })
      .catch(() => {});
  }, []);
  useEffect(read, [read]);
  useEffect(() => {
    if (!watch || state === 'granted' || state === 'notApplicable') return;
    const timer = setInterval(read, ACCESSIBILITY_POLL_MS);
    return () => clearInterval(timer);
  }, [watch, state, read]);
  return {
    state,
    refresh: (next) => (next ? setState(next) : read()),
  };
}

/** The tour's three checks, in the order the person is asked to do them. */
export const TOUR_STEPS = ['knob', 'press', 'buttons'] as const;
export type TourStep = (typeof TOUR_STEPS)[number];

const sameAction = (a: DeskActionDto | null, b: DeskActionDto | null): boolean =>
  a === b ||
  (a !== null &&
    b !== null &&
    a.action === b.action &&
    a.result === b.result &&
    a.permissionRequired === b.permissionRequired);

/**
 * Watches `desk://status` for the person using the device: the knob is a change of
 * `volumePercent`; a press and a button are each a new settled `lastAction` (a `processing` result
 * is not counted, so one press never ticks two steps). Steps are done in order, and each one only
 * looks at changes that happen after the previous one finished.
 */
export function useTour(desk: DeskStatusDto | null, active: boolean): { done: number } {
  const [done, setDone] = useState(0);
  const baseline = useRef<{ volume: number | null; last: DeskActionDto | null } | null>(null);

  useEffect(() => {
    // Each visit starts from what the device shows now, never from an earlier reading.
    if (!active) baseline.current = null;
  }, [active]);

  useEffect(() => {
    if (!active || !desk || done >= TOUR_STEPS.length) return;
    const now = { volume: desk.volumePercent, last: desk.lastAction };
    const before = baseline.current;
    if (before === null) {
      baseline.current = now;
      return;
    }
    const step = TOUR_STEPS[done];
    const knobTurned =
      before.volume !== null && now.volume !== null && now.volume !== before.volume;
    const actionRan =
      now.last !== null && now.last.result !== 'processing' && !sameAction(now.last, before.last);
    if ((step === 'knob' && knobTurned) || (step !== 'knob' && actionRan)) {
      baseline.current = now;
      setDone(done + 1);
    } else if (before.volume === null && now.volume !== null) {
      // The volume was unknown at the start; the first reading is the reference, not a turn.
      baseline.current = { ...before, volume: now.volume };
    }
  }, [desk, done, active]);

  return { done };
}
