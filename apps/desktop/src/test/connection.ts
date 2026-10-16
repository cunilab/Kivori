import { CAPABILITY_NAMES, type CapabilityName, type ConnectionStatusDto } from '../lib/ipc/types';

/** Test-only sessions: fixtures never enter the shipping product catalog. */
export function connectionFixture(
  capabilities: CapabilityName[] = [...CAPABILITY_NAMES],
  overrides: Partial<ConnectionStatusDto> = {},
): ConnectionStatusDto {
  return {
    connection: 'connected',
    desired: 'idle',
    reported: 'idle',
    device: {
      presentation: { displayName: 'Kivori Pip', model: 'KV-P1', art: null },
      firmwareVersion: '0.1.0',
      protocolVersion: { major: 0, minor: 1 },
      deviceIdHashShort: 'deadbeef',
      identity: {
        product: 'Kivori Pip',
        board: 'C3 SuperMini',
        hwRev: null,
        source: 'chip',
        known: true,
      },
    },
    incompatibleReason: null,
    retryCount: 0,
    connectionGeneration: 1,
    capabilities,
    mascotInteraction: capabilities.includes('mascotInteraction'),
    mascotAction: null,
    host: 'active',
    ...overrides,
  };
}
