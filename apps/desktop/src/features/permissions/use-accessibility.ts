import { useCallback, useEffect, useRef, useState } from 'react';
import { getAccessibility, markAccessibilityAsked } from '@/lib/ipc';
import type { AccessibilityState } from '@/lib/ipc/types';
import { recover, type RecoveryEvent, type RecoveryPhase } from './accessibility-recovery';

/** How often the permission notice re-reads Accessibility while the user is in System Settings. */
export const ACCESSIBILITY_POLL_MS = 1000;

export interface AccessibilityControl {
  state: AccessibilityState | null;
  stale: boolean;
  refresh: (next?: AccessibilityState) => void;
  /** The user was just sent to the macOS prompt or to System Settings. */
  asked: () => void;
  /** The stale approval was reset (the caller shows the new state through `refresh`). */
  reset: () => void;
}

/**
 * The Accessibility permission. Read once on mount (so it knows whether there is a permission at
 * all), re-read every second while `watch` is true (also after a grant, so a revocation
 * in System Settings shows here too) and whenever the window regains focus (so coming back from
 * System Settings flips it without a restart). Reads never prompt. `null` = not known yet.
 *
 * `stale` turns true when the user was sent to ask, came back, and macOS still says it is off:
 * the cue to offer the reset. Asking itself only ever happens from a button, never from here, and
 * every ask is recorded (`asked`) so the one automatic ask is never repeated after it.
 */
export function useAccessibility(watch: boolean): AccessibilityControl {
  const [state, setState] = useState<AccessibilityState | null>(null);
  const [phase, setPhase] = useState<RecoveryPhase>('fresh');
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const apply = useCallback((event: RecoveryEvent) => setPhase((now) => recover(now, event)), []);
  const read = useCallback(
    (type: 'read' | 'returned' = 'read') => {
      void getAccessibility()
        .then((next) => {
          if (!alive.current) return;
          setState(next);
          apply({ type, state: next });
        })
        .catch(() => {});
    },
    [apply],
  );
  useEffect(() => read(), [read]);
  useEffect(() => {
    if (!watch || state === 'notApplicable') return;
    const timer = setInterval(read, ACCESSIBILITY_POLL_MS);
    return () => clearInterval(timer);
  }, [watch, state, read]);
  useEffect(() => {
    if (state === 'notApplicable') return;
    const back = (): void => {
      if (document.visibilityState !== 'hidden') read('returned');
    };
    window.addEventListener('focus', back);
    document.addEventListener('visibilitychange', back);
    return () => {
      window.removeEventListener('focus', back);
      document.removeEventListener('visibilitychange', back);
    };
  }, [state, read]);
  return {
    state,
    stale: phase === 'stale',
    refresh: (next) => {
      if (!next) return read();
      setState(next);
      apply({ type: 'read', state: next });
    },
    asked: () => {
      apply({ type: 'asked' });
      markAccessibilityAsked().catch(() => {});
    },
    reset: () => apply({ type: 'reset' }),
  };
}
