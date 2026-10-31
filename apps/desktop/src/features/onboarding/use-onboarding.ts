import { useCallback, useEffect, useState } from 'react';
import {
  completeOnboarding,
  getOnboarding,
  markAccessibilityAsked,
  restartOnboarding,
} from '@/lib/ipc';
import type { OnboardingDto } from '@/lib/ipc/types';

/**
 * Whether first-run setup is finished, and whether the one automatic Accessibility ask was made.
 * `completed` is `null` until the native side answers, so the caller can tell "not known yet"
 * from "needs setup".
 */
export function useOnboarding(): {
  completed: boolean | null;
  accessibilityAsked: boolean | null;
  complete: () => Promise<void>;
  restart: () => Promise<void>;
  /** Records that the one automatic Accessibility ask was shown. */
  markAccessibilityAsked: () => Promise<void>;
} {
  const [state, setState] = useState<OnboardingDto | null>(null);
  useEffect(() => {
    let active = true;
    void getOnboarding()
      .then((next) => {
        if (active) setState(next);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);
  const complete = useCallback(async () => {
    setState(await completeOnboarding());
  }, []);
  const restart = useCallback(async () => {
    setState(await restartOnboarding());
  }, []);
  const markAsked = useCallback(async () => {
    setState(await markAccessibilityAsked());
  }, []);
  return {
    completed: state?.completed ?? null,
    accessibilityAsked: state?.accessibilityAsked ?? null,
    complete,
    restart,
    markAccessibilityAsked: markAsked,
  };
}
