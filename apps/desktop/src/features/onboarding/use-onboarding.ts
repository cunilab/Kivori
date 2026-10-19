import { useCallback, useEffect, useRef, useState } from 'react';
import { completeOnboarding, getAccessibility, getOnboarding, restartOnboarding } from '@/lib/ipc';
import type { AccessibilityState } from '@/lib/ipc/types';

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
