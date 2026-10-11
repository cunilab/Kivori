import type { DeviceInternalsDto } from '@/lib/ipc/types';

// The newest internals the card received, so "Copy diagnostics" (a sibling card) can include them
// without polling the device itself. Module state, like the dev-mode store.
let latest: DeviceInternalsDto | null = null;

export function setLatestInternals(value: DeviceInternalsDto | null): void {
  latest = value;
}

export function getLatestInternals(): DeviceInternalsDto | null {
  return latest;
}
