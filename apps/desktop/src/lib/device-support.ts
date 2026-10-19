import type { CapabilityName, ConnectionStatusDto } from './ipc/types';

/** Saved metadata never authorizes live features, including during reconnect/identity exchange. */
export function hasCapability(
  connection: ConnectionStatusDto | null,
  capability: CapabilityName,
): boolean {
  return (
    connection?.connection === 'connected' &&
    connection.device?.identity?.known === true &&
    connection.capabilities.includes(capability)
  );
}

/** Baseline state sync predates capability negotiation; preserve the backend's legacy path. */
export function canMirrorState(connection: ConnectionStatusDto | null): boolean {
  return (
    connection?.connection === 'connected' &&
    connection.device?.identity?.known === true &&
    (connection.device.identity.source === 'legacy' ||
      connection.capabilities.some((capability) => capability !== 'productIdentityV1'))
  );
}
