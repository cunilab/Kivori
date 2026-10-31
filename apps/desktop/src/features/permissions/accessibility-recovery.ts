import type { AccessibilityState } from '@/lib/ipc/types';

/**
 * Where the Accessibility permission step is in asking:
 * - `fresh`: nothing asked yet (or it is on / was just reset).
 * - `tried`: the user was sent to the macOS prompt or System Settings.
 * - `stale`: they came back and macOS still says it is off, so it probably holds an older approval
 *   for this app (those are tied to the exact build, so an update or rebuild invalidates them).
 */
export type RecoveryPhase = 'fresh' | 'tried' | 'stale';

export type RecoveryEvent =
  /** The user pressed Allow access or Open System Settings. */
  | { type: 'asked' }
  /** The window regained focus and macOS was read again. */
  | { type: 'returned'; state: AccessibilityState }
  /** Any other read (the poll); it can only clear the recovery, never raise it. */
  | { type: 'read'; state: AccessibilityState }
  /** The reset ran: start over, the next ask is a fresh one. */
  | { type: 'reset' };

/** The next phase. Pure: asking never happens in here, so nothing can prompt in a loop. */
export function recover(phase: RecoveryPhase, event: RecoveryEvent): RecoveryPhase {
  switch (event.type) {
    case 'asked':
      return phase === 'fresh' ? 'tried' : phase;
    case 'returned':
      if (event.state !== 'missing') return 'fresh';
      return phase === 'tried' ? 'stale' : phase;
    case 'read':
      return event.state === 'missing' ? phase : 'fresh';
    case 'reset':
      return 'fresh';
  }
}
