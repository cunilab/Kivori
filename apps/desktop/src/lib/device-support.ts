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

/** Readiness for setup; a transport connection alone does not make a device usable. */
export function sessionState(
  connection: ConnectionStatusDto | null,
): 'offline' | 'pending' | 'unknown' | 'attention' | 'ready' {
  if (connection?.connection !== 'connected') return 'offline';
  const identity = connection.device?.identity;
  if (!identity) return 'pending';
  if (!identity.known) return 'unknown';
  return canMirrorState(connection) ? 'ready' : 'attention';
}

export type ControlGesture =
  'rotate' | 'press' | 'hold' | 'doublePress' | `button${1 | 2 | 3}` | `button${1 | 2 | 3}Hold`;

/** The same live-control support rule for saved binding rows and setup guidance. */
export function controlSupported(
  connection: ConnectionStatusDto | null,
  key: ControlGesture,
): boolean {
  if (!hasCapability(connection, 'physicalInputV1')) return false;
  switch (key) {
    case 'rotate':
      return true;
    case 'press':
    case 'hold':
      return hasCapability(connection, 'buttonInputV1');
    case 'doublePress':
      return (
        hasCapability(connection, 'buttonInputV1') && hasCapability(connection, 'doublePressV1')
      );
    default:
      return hasCapability(connection, 'contextButtonsV1');
  }
}
