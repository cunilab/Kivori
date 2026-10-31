import userEvent from '@testing-library/user-event';
import { act, render, screen, waitFor } from '@testing-library/react';
import axe from 'axe-core';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import { resetPageTabs } from '../../../components/kivori/page-tabs';
import { setDevMode } from '../../../lib/dev-mode';
import type { AppInfoDto, ConnectionStatusDto, DeviceBuildDto } from '../../../lib/ipc/types';

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
    capabilities: [],
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

beforeEach(() => {
  resetPageTabs();
  localStorage.clear();
});

describe('DevicePage', () => {
  it('shows customer device details without developer protocol/state rows', () => {
    render(
      view(
        status({
          connection: 'connected',
          desired: 'busy',
          reported: 'happy',
          device: {
            presentation: null,
            firmwareVersion: '0.1.2',
            protocolVersion: { major: 0, minor: 1 },
            deviceIdHashShort: 'deadbeef',
            identity: null,
          },
        }),
      ),
    );
    expect(screen.getByRole('status')).toHaveTextContent('Connected');
    expect(screen.getByTestId('device-firmware')).toHaveTextContent('0.1.2');
    expect(screen.getByTestId('device-product')).toHaveTextContent('Kivori');
    expect(screen.queryByText('Buddy (requested)')).not.toBeInTheDocument();
    expect(screen.queryByText('Protocol')).not.toBeInTheDocument();
    expect(screen.queryByText('deadbeef')).not.toBeInTheDocument();
  });

  it('shows the product, board and identity source, with the revision unknown until provisioned', () => {
    render(
      view(
        status({
          connection: 'connected',
          device: {
            presentation: null,
            firmwareVersion: '0.1.2',
            protocolVersion: { major: 0, minor: 1 },
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
    expect(screen.getByTestId('device-hw-rev')).toHaveTextContent(/^Unknown$/);
    expect(screen.getByTestId('device-identity-source')).toHaveTextContent('Firmware build');
  });

  it('shows an unknown product by its number and a provisioned revision', () => {
    render(
      view(
        status({
          connection: 'connected',
          device: {
            presentation: null,
            firmwareVersion: '0.1.2',
            protocolVersion: { major: 0, minor: 1 },
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

  it('shows reconnecting with the attempt count and an empty state instead of blank rows', () => {
    render(view(status({ retryCount: 3 })));
    expect(screen.getByRole('status')).toHaveTextContent('Reconnecting…');
    expect(screen.getByRole('status')).toHaveTextContent('3');
    expect(screen.getByTestId('device-empty')).toHaveTextContent(
      'Connect your Kivori to see its details',
    );
    expect(screen.queryByTestId('device-firmware')).not.toBeInTheDocument();
    expect(screen.queryByTestId('device-board')).not.toBeInTheDocument();
    expect(screen.queryByText('Unknown')).not.toBeInTheDocument();
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

  it('splits the page into Overview and Firmware tabs', async () => {
    const user = userEvent.setup();
    render(view(status({ connection: 'disconnected', device: null })));
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      'Overview',
      'Firmware',
    ]);
    expect(screen.getAllByTestId('device-empty')).toHaveLength(1);
    expect(screen.queryByRole('heading', { name: 'Firmware update' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: 'Firmware' }));
    await waitFor(() => expect(screen.queryByTestId('device-empty')).not.toBeInTheDocument());
    expect(screen.getByText('Connect your device to enable flashing.')).toBeInTheDocument();
  });

  it('collapses device-only Internals rows into one line while disconnected', async () => {
    setDevMode(true);
    const user = userEvent.setup();
    render(view(status({ connection: 'disconnected', device: null })));
    await user.click(screen.getByRole('tab', { name: 'Internals' }));
    expect(screen.getByText('Buddy (requested)')).toBeInTheDocument();
    expect(
      screen.getByText('Connect your Kivori to see its protocol and reported state.'),
    ).toBeInTheDocument();
    expect(screen.queryByText('Protocol')).not.toBeInTheDocument();
    expect(screen.queryByText('Buddy (reported)')).not.toBeInTheDocument();
  });

  it('keeps the developer switch on Settings, and shows protocol rows and internals only in developer mode', async () => {
    const connected = status({
      connection: 'connected',
      desired: 'busy',
      device: {
        presentation: null,
        firmwareVersion: '0.1.2',
        protocolVersion: { major: 0, minor: 1 },
        deviceIdHashShort: 'deadbeef',
        identity: null,
      },
    });
    const page = render(view(connected));
    expect(screen.queryByRole('switch', { name: 'Developer mode' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Device internals' })).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'Internals' })).not.toBeInTheDocument();
    act(() => setDevMode(true));
    await userEvent.setup().click(screen.getByRole('tab', { name: 'Internals' }));
    expect(screen.getByText('Protocol')).toBeInTheDocument();
    expect(screen.getByText('Buddy (requested)')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Device internals' })).toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: 'Developer mode' })).not.toBeInTheDocument();
    const results = await axe.run(page.container, {
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});

describe('DevicePage firmware build', () => {
  const APP = 'c2ff8c4f68de';
  const appInfo = (sourceId: string | null): AppInfoDto => ({
    appVersion: '0.2.1',
    protocolVersion: { major: 0, minor: 2 },
    supportedMajors: [0],
    deviceStudioEnabled: false,
    sourceId,
  });
  const withBuild = (
    build: DeviceBuildDto | null | undefined,
    sourceId: string | null = APP,
  ): ReactElement => (
    <TooltipProvider>
      <DevicePage
        appInfo={appInfo(sourceId)}
        connection={status({
          connection: 'connected',
          device: {
            presentation: null,
            firmwareVersion: '0.1.2',
            protocolVersion: { major: 0, minor: 2 },
            deviceIdHashShort: 'deadbeef',
            identity: null,
            build,
          },
        })}
      />
    </TooltipProvider>
  );

  beforeEach(() => act(() => setDevMode(false)));

  it('shows the source id and stays quiet when it matches the app', () => {
    render(withBuild({ sourceId: APP, dirty: false }));
    expect(screen.getByTestId('device-build')).toHaveTextContent(/^c2ff8c4f68de$/);
    expect(screen.queryByTestId('device-build-dirty')).not.toBeInTheDocument();
    expect(screen.queryByTestId('device-build-differs')).not.toBeInTheDocument();
  });

  it('warns for everyone when the unit runs a build with uncommitted changes', () => {
    render(withBuild({ sourceId: APP, dirty: true }));
    expect(screen.getByTestId('device-build')).toHaveTextContent(
      'c2ff8c4f68de · with uncommitted changes',
    );
    expect(screen.getByTestId('device-build-dirty')).toHaveTextContent(
      'Built from uncommitted changes',
    );
  });

  it('says a different firmware source only in developer mode, so release users get no false alarm', () => {
    const other = { sourceId: '0123456789ab', dirty: false };
    const page = render(withBuild(other));
    expect(screen.getByTestId('device-build')).toHaveTextContent('0123456789ab');
    expect(screen.queryByTestId('device-build-differs')).not.toBeInTheDocument();
    act(() => setDevMode(true));
    expect(screen.getByTestId('device-build-differs')).toHaveTextContent(
      "The device firmware source (0123456789ab) differs from this app's build (c2ff8c4f68de).",
    );
    page.unmount();
  });

  it('says nothing about a difference when either side does not know its source id', () => {
    act(() => setDevMode(true));
    const first = render(withBuild({ sourceId: null, dirty: false }));
    expect(screen.getByTestId('device-build')).toHaveTextContent('Unknown (built without git)');
    expect(screen.queryByTestId('device-build-differs')).not.toBeInTheDocument();
    first.unmount();
    render(withBuild({ sourceId: '0123456789ab', dirty: false }, null));
    expect(screen.queryByTestId('device-build-differs')).not.toBeInTheDocument();
  });

  it('shows no build row for older firmware, except as "Unknown" in developer mode', () => {
    const page = render(withBuild(null));
    expect(screen.queryByTestId('device-build')).not.toBeInTheDocument();
    page.unmount();
    act(() => setDevMode(true));
    render(withBuild(undefined));
    expect(screen.getByTestId('device-build')).toHaveTextContent('Unknown');
  });
});
