import type { ProfileConfigDto } from '@/lib/ipc/types';

/** Whether any profile still opens the profile picker with Hold + turn. */
export function anyProfileKeepsPicker(profiles: readonly ProfileConfigDto[]): boolean {
  return profiles.some((profile) => profile.heldRotate.spec.kind === 'profilePicker');
}
