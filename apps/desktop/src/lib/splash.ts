import { strings } from './i18n/strings';

/// The splash status line for a phase token from the native core (`startup::StartupPhase::token`).
/// Anything unknown reads as the first phase, so the line is never blank or made up.
export function splashStatusText(phase: string | null): string {
  return phase === 'looking' ? strings.splash.looking : strings.splash.starting;
}
