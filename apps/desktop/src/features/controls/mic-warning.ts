import type { ProfileConfigDto, SlotDto } from '@/lib/ipc/types';

function mutesMic(slot: SlotDto): boolean {
  return slot.action?.kind === 'micMute';
}

/** Whether any profile binds Mic mute on any slot. */
export function anySlotMutesMic(profiles: readonly ProfileConfigDto[]): boolean {
  return profiles.some((profile) =>
    [
      profile.press,
      profile.hold,
      profile.doublePress,
      ...profile.buttons.flatMap((button) => [button.press, button.hold]),
    ].some(mutesMic),
  );
}
