import type { FirmwareStatusDto, FlashFailure } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';

/** Failures where the device itself likely needs the BOOT-button restore. */
const NEEDS_RESTORE: readonly FlashFailure[] = ['noDownloadMode', 'unknown'];

/** The plain-language message for a failed attempt; an unrecognized or missing token reads as unknown. */
export function failureMessage(failure: FlashFailure | null): string {
  return strings.firmwareUpdate.failures[failure ?? 'unknown'];
}

/** Whether a status asks for the BOOT-button restore call to action. */
export function needsRestore(status: Pick<FirmwareStatusDto, 'phase' | 'failure'> | null): boolean {
  return status?.phase === 'failed' && NEEDS_RESTORE.includes(status.failure ?? 'unknown');
}
