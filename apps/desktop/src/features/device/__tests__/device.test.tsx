import { render, screen } from '@testing-library/react';
import axe from 'axe-core';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import type { ConnectionStatusDto } from '../../../lib/ipc/types';

vi.mock('../../../lib/ipc', () => ({
  getFirmwareStatus: () =>
    Promise.resolve({
      available: true,
      phase: 'idle',
      message: 'Ready.',
      imageSize: 1024,
      failure: null,
      bundledVersion: null,
      advice: 'unknown',
    }),
  flashFirmware: vi.fn(),
  getStartupSettings: () => new Promise(() => {}),
  onStartupChanged: () => Promise.resolve(() => {}),
  getDiagnostics: () => new Promise(() => {}),
  getDeviceInternals: () => new Promise(() => {}),
  getUpdateStatus: () => new Promise(() => {}),
  onUpdateStatus: () => Promise.resolve(() => {}),
  getFirmwareOffer: () => new Promise(() => {}),
  onFirmwareOffer: () => Promise.resolve(() => {}),
}));

import { DevicePage } from '../DevicePage';

function status(overrides: Partial<ConnectionStatusDto> = {}): ConnectionStatusDto {
  return {
    connection: 'disconnected',
    desired: 'idle',
    reported: null,
    device: null,
    incompatibleReason: null,
    retryCount: 0,
    connectionGeneration: 0,
    mascotInteraction: false,
    mascotAction: null,
    host: 'active',
    ...overrides,
  };
}

const view = (s: ConnectionStatusDto): ReactElement => (
  <TooltipProvider>
    <DevicePage connection={s} appInfo={null} />
  </TooltipProvider>
);

// These cases cover the technical rows, which only developer mode shows (app.test covers the gate).
beforeEach(() => localStorage.setItem('kivori.developerMode', 'true'));

describe('DevicePage', () => {
  it('shows connected device details and both buddy state axes', () => {
    render(
      view(
        status({
          connection: 'connected',
          desired: 'busy',
          reported: 'happy',
          device: {
            firmwareVersion: '1.4.2',
            protocolVersion: { major: 1, minor: 0 },
            deviceIdHashShort: 'deadbeef',
            identity: null,
          },
        }),
      ),
    );
    expect(screen.getByRole('status')).toHaveTextContent('Connected');
    expect(screen.getByTestId('device-firmware')).toHaveTextContent('1.4.2');
    expect(screen.getByTestId('desired')).toHaveTextContent('Busy');
    expect(screen.getByTestId('reported')).toHaveTextContent('Happy');
    expect(screen.getByText('deadbeef')).toBeInTheDocument();
  });

  it('shows the product, board and identity source, with the revision unknown until provisioned', () => {
    render(
      view(
        status({
          connection: 'connected',
          device: {
            firmwareVersion: '1.4.2',
            protocolVersion: { major: 1, minor: 6 },
            deviceIdHashShort: 'deadbeef',
            identity: {
              product: 'Kivori Pip',
              board: 'C3 SuperMini',
              hwRev: null,
              source: 'build',
              known: true,
            },
          },
        }),
      ),
    );
    expect(screen.getByTestId('device-product')).toHaveTextContent('Kivori Pip');
    expect(screen.getByTestId('device-board')).toHaveTextContent('C3 SuperMini');
    expect(screen.getByTestId('device-hw-rev')).toHaveTextContent(/^—$/);
    expect(screen.getByTestId('device-identity-source')).toHaveTextContent('Firmware build');
  });

  it('shows an unknown product by its number and a provisioned revision', () => {
    render(
      view(
        status({
          connection: 'connected',
          device: {
            firmwareVersion: '1.4.2',
            protocolVersion: { major: 1, minor: 6 },
            deviceIdHashShort: 'deadbeef',
            identity: {
              product: 'Unknown product (9)',
              board: 'Unknown board (4)',
              hwRev: 2,
              source: 'chip',
              known: false,
            },
          },
        }),
      ),
    );
    expect(screen.getByTestId('device-product')).toHaveTextContent('Unknown product (9)');
    expect(screen.getByTestId('device-hw-rev')).toHaveTextContent('2');
    expect(screen.getByTestId('device-identity-source')).toHaveTextContent('Chip');
  });

  it('shows reconnecting with the attempt count and unknown versions as "—"', () => {
    render(view(status({ retryCount: 3 })));
    expect(screen.getByRole('status')).toHaveTextContent('Reconnecting…');
    expect(screen.getByRole('status')).toHaveTextContent('3');
    expect(screen.getByTestId('device-firmware')).toHaveTextContent(/^—$/);
    expect(screen.getByTestId('reported')).toHaveTextContent(/^—$/);
  });

  it('surfaces an incompatible reason', () => {
    render(
      view(
        status({
          connection: 'incompatible',
          incompatibleReason: 'device protocol major v2 is unsupported',
        }),
      ),
    );
    expect(screen.getByText(/v2 is unsupported/)).toBeInTheDocument();
  });

  it('has no axe violations', async () => {
    const { container } = render(view(status()));
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });

  it('shows the Device internals card only with developer mode on', () => {
    render(view(status()));
    expect(screen.getByRole('heading', { name: 'Device internals' })).toBeInTheDocument();
    localStorage.removeItem('kivori.developerMode');
    render(view(status()));
    expect(screen.getAllByRole('heading', { name: 'Device internals' })).toHaveLength(1);
  });
});
