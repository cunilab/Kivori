import { useCallback, useEffect, useState } from 'react';
import { completeOnboarding, getOnboarding, restartOnboarding } from '@/lib/ipc';

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
