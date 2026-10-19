import { act, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axe from 'axe-core';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import type { DeviceInternalsDto } from '../../../lib/ipc/types';
import { connectionFixture } from '../../../test/connection';
import { parseDeviceInternals } from '../../../lib/ipc/validate';

const bridge = vi.hoisted(() => ({ get: vi.fn() }));
vi.mock('../../../lib/ipc', () => ({ getDeviceInternals: bridge.get }));

import { DeviceInternals, formatBytes } from '../DeviceInternals';
import { getLatestInternals, internalsForBundle, setLatestInternals } from '../internals-latest';

const view = (): ReturnType<typeof render> =>
  render(
    <TooltipProvider>
      <DeviceInternals connection={connectionFixture()} />
    </TooltipProvider>,
  );

const snapshot: NonNullable<DeviceInternalsDto['snapshot']> = {
  appImageBytes: 742_880,
  appPartitionBytes: 4_128_768,
  assetBlobBytes: 65_536,
  assetBlobCapBytes: 131_072,
  flashTotalBytes: 4_194_304,
  ramTotalBytes: 320_512,
  ramStaticBytes: 125_600,
  ramFreeBytes: 188_000,
  stackFreeMinBytes: 150_000,
  busyPercent: 14,
  composeMs: 6,
  flushMs: 21,
  fps: 30,
  chipModel: 'esp32c3',
  chipRevision: { major: 0, minor: 4 },
  temperatureDeciCelsius: null,
  uptimeMs: 3_723_000,
  resetReason: 'powerOn',
  edgeQueueHighWater: 6,
  droppedEdges: 0,
  invalidTransitions: 0,
  framesIn: 3910,
  framesOut: 7802,
  framesRejected: 0,
};

const reporting: DeviceInternalsDto = {
  connected: true,
  supported: true,
  connectionGeneration: 1,
  snapshotAgeMs: 0,
  snapshot,
};

beforeEach(() => {
  bridge.get.mockReset().mockResolvedValue(reporting);
});

describe('formatBytes', () => {
  it('uses B, KiB and MiB', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(2048)).toBe('2.0 KiB');
    expect(formatBytes(4_194_304)).toBe('4.00 MiB');
  });
});

describe('DeviceInternals card', () => {
  it('shows sizes with percentages and every group', async () => {
    view();
    await screen.findByText('Flash');
    expect(screen.getByTestId('internals-app-image')).toHaveTextContent(
      '725.5 KiB of 3.94 MiB (18.0%)',
    );
    expect(screen.getByTestId('internals-asset-blob')).toHaveTextContent(
      '64.0 KiB of 128.0 KiB (50.0%)',
    );
    expect(screen.getByTestId('internals-ram-static')).toHaveTextContent('(39.2%)');
    expect(
      screen.getByRole('progressbar', { name: 'Asset blob / build limit' }),
    ).toBeInTheDocument();
    expect(screen.getByText('ESP32-C3 rev v0.4')).toBeInTheDocument();
    expect(screen.getByText('Power on')).toBeInTheDocument();
    expect(screen.getByText('14%')).toBeInTheDocument();
    expect(screen.getByText('30 fps')).toBeInTheDocument();
    expect(screen.getByText('3910')).toBeInTheDocument();
  });

  it('names the tiles and explains the scope of memory and runtime measurements', async () => {
    const page = view();
    const flash = await screen.findByRole('region', { name: 'Flash' });
    expect(within(flash).getByText(/asset blob is included in the image/)).toBeInTheDocument();
    const memory = screen.getByRole('region', { name: 'Memory' });
    expect(within(memory).getByText('Stack headroom now')).toBeInTheDocument();
    expect(within(memory).getByText('Lowest stack headroom since boot')).toBeInTheDocument();
    expect(within(memory).getByText(/this firmware has no heap/)).toBeInTheDocument();
    const runtime = screen.getByRole('region', { name: 'Runtime and frames' });
    expect(within(runtime).getByText(/run-loop ticks that did work/)).toBeInTheDocument();
    expect(within(runtime).getByText('14%')).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Chip' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Input and link' })).toBeInTheDocument();
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
  });

  it.each([null, 0])('omits ratios for unavailable or zero capacities (%s)', async (capacity) => {
    bridge.get.mockResolvedValue({
      ...reporting,
      snapshot: {
        ...snapshot,
        appPartitionBytes: capacity,
        assetBlobCapBytes: capacity,
        ramTotalBytes: capacity,
      },
    });
    view();
    await screen.findByText('Flash');
    expect(screen.getByTestId('internals-app-image')).toHaveTextContent(/^725.5 KiB$/);
    expect(screen.getByTestId('internals-asset-blob')).toHaveTextContent(/^64.0 KiB$/);
    expect(screen.getByTestId('internals-ram-static')).toHaveTextContent(/^122.7 KiB$/);
    expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
  });

  it('shows over-cap usage honestly while keeping the bar within its bounds', async () => {
    bridge.get.mockResolvedValue({
      ...reporting,
      snapshot: { ...snapshot, assetBlobBytes: 196_608 },
    });
    view();
    await screen.findByText('Flash');
    expect(screen.getByTestId('internals-asset-blob')).toHaveTextContent('(150.0%)');
    expect(screen.getByRole('progressbar', { name: 'Asset blob / build limit' })).toHaveAttribute(
      'aria-valuenow',
      '100',
    );
  });

  it('preserves measured zero and unavailable sizes without inventing usage', async () => {
    bridge.get.mockResolvedValue({
      ...reporting,
      snapshot: { ...snapshot, appImageBytes: null, ramStaticBytes: 0, busyPercent: 0 },
    });
    view();
    await screen.findByText('Flash');
    expect(
      within(screen.getByTestId('internals-app-image')).getByText('Unavailable'),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('progressbar', { name: 'App image / app partition' }),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('internals-ram-static')).toHaveTextContent('0 B of 313.0 KiB (0.0%)');
    expect(
      screen.getByRole('progressbar', { name: 'Static RAM / usable data RAM' }),
    ).toHaveAttribute('aria-valuenow', '0');
    expect(
      within(screen.getByRole('region', { name: 'Runtime and frames' })).getByText('0%'),
    ).toBeInTheDocument();
  });

  it('says Unavailable for what the device cannot measure, never 0', async () => {
    view();
    await screen.findByText('Flash');
    // Only the temperature is unavailable in this snapshot.
    expect(screen.getAllByLabelText(/^Unavailable\./)).toHaveLength(1);
    expect(screen.queryByText('0.0 °C')).not.toBeInTheDocument();
  });

  it('tells a device without the capability that its firmware does not report internals', async () => {
    bridge.get.mockResolvedValue({
      connected: true,
      supported: false,
      connectionGeneration: 1,
      snapshotAgeMs: null,
      snapshot: null,
    });
    view();
    await screen.findByText(/firmware doesn't report internals/);
    expect(screen.queryByText('Flash')).not.toBeInTheDocument();
  });

  it('asks to connect when no device is connected, and waits for the first answer', async () => {
    bridge.get.mockResolvedValue({
      connected: false,
      supported: false,
      connectionGeneration: 1,
      snapshotAgeMs: null,
      snapshot: null,
    });
    const { unmount } = view();
    await screen.findByText('Connect your Kivori to see its internals.');
    unmount();
    bridge.get.mockResolvedValue({
      connected: true,
      supported: true,
      connectionGeneration: 1,
      snapshotAgeMs: null,
      snapshot: null,
    });
    view();
    await screen.findByText('Waiting for the device to answer…');
  });

  it('polls about once a second while mounted and stops when it unmounts', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { unmount } = view();
      await waitFor(() => expect(bridge.get).toHaveBeenCalledTimes(1));
      await vi.advanceTimersByTimeAsync(1100);
      await waitFor(() => expect(bridge.get.mock.calls.length).toBeGreaterThanOrEqual(2));
      unmount();
      const calls = bridge.get.mock.calls.length;
      await vi.advanceTimersByTimeAsync(3000);
      expect(bridge.get).toHaveBeenCalledTimes(calls);
      expect(getLatestInternals()).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('clears a mounted card on reconnect and rejects the old session reply arriving later', async () => {
    let resolveOld!: (value: DeviceInternalsDto) => void;
    bridge.get.mockReturnValueOnce(
      new Promise<DeviceInternalsDto>((resolve) => {
        resolveOld = resolve;
      }),
    );
    const page = view();
    bridge.get.mockResolvedValue({ ...reporting, connectionGeneration: 2, snapshot: null });
    page.rerender(
      <TooltipProvider>
        <DeviceInternals connection={connectionFixture(undefined, { connectionGeneration: 2 })} />
      </TooltipProvider>,
    );
    await waitFor(() => expect(bridge.get).toHaveBeenCalledTimes(2));
    await act(async () => {
      resolveOld(reporting);
    });
    expect(screen.queryByText('Flash')).not.toBeInTheDocument();
    expect(internalsForBundle(2, true).status).toBe('unavailable');
    page.unmount();
    expect(getLatestInternals()).toBeNull();
  });

  it('clears measured values as soon as a still-mounted card disconnects', async () => {
    const page = view();
    await screen.findByText('Flash');
    page.rerender(
      <TooltipProvider>
        <DeviceInternals connection={connectionFixture([], { connection: 'disconnected' })} />
      </TooltipProvider>,
    );
    expect(screen.queryByText('Flash')).not.toBeInTheDocument();
    expect(getLatestInternals()).toBeNull();
  });

  it('labels stale native measurements and excludes them from the bundle', async () => {
    bridge.get.mockResolvedValue({ ...reporting, snapshotAgeMs: 6000 });
    view();
    await screen.findByText(/internals are stale/);
    expect(screen.queryByText('Flash')).not.toBeInTheDocument();
    expect(internalsForBundle(1, true)).toMatchObject({ status: 'stale', data: null });
  });

  it('invalidates the copy cache when polling fails', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      view();
      await screen.findByText('Flash');
      bridge.get.mockRejectedValue(new Error('IPC failed'));
      await act(() => vi.advanceTimersByTimeAsync(1100));
      await screen.findByText('Device internals are unavailable right now.');
      expect(getLatestInternals()).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('counts time in flight and rejects replies received after hiding the window', async () => {
    let resolve!: (value: DeviceInternalsDto) => void;
    bridge.get.mockReturnValueOnce(
      new Promise<DeviceInternalsDto>((done) => {
        resolve = done;
      }),
    );
    const clock = vi.spyOn(performance, 'now').mockReturnValue(100);
    try {
      const page = view();
      bridge.get.mockResolvedValue({ ...reporting, snapshotAgeMs: 6100 });
      clock.mockReturnValue(6200);
      await act(async () => {
        resolve(reporting);
      });
      await screen.findByText(/internals are stale/);
      expect(internalsForBundle(1, true).status).toBe('stale');
      page.unmount();
      bridge.get.mockReturnValueOnce(
        new Promise<DeviceInternalsDto>((done) => {
          resolve = done;
        }),
      );
      view();
      const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
      try {
        act(() => document.dispatchEvent(new Event('visibilitychange')));
        await act(async () => {
          resolve(reporting);
        });
        expect(getLatestInternals()).toBeNull();
        expect(screen.queryByText('Flash')).not.toBeInTheDocument();
      } finally {
        hidden.mockRestore();
      }
    } finally {
      clock.mockRestore();
    }
  });

  it('ages a native measurement without treating cache reads as device replies', () => {
    const clock = vi.spyOn(performance, 'now').mockReturnValue(100);
    try {
      setLatestInternals({ ...reporting, snapshotAgeMs: 4000 });
      expect(internalsForBundle(1, true)).toEqual({
        status: 'fresh',
        ageMs: 4000,
        data: {
          connected: true,
          supported: true,
          snapshot: reporting.snapshot,
          snapshotAgeMs: 4000,
        },
      });
      expect(internalsForBundle(1, true).data).not.toHaveProperty('connectionGeneration');
      expect(getLatestInternals()?.connectionGeneration).toBe(1);
      clock.mockReturnValue(1200);
      expect(internalsForBundle(1, true)).toEqual({ status: 'stale', ageMs: 5100, data: null });
      expect(internalsForBundle(2, true)).toEqual({
        status: 'unavailable',
        ageMs: null,
        data: null,
      });
      expect(internalsForBundle(1, false)).toEqual({
        status: 'unavailable',
        ageMs: null,
        data: null,
      });
    } finally {
      clock.mockRestore();
      setLatestInternals(null);
    }
  });

  it('keeps the latest answer for Copy diagnostics while mounted', async () => {
    view();
    await screen.findByText('Flash');
    expect(getLatestInternals()).toMatchObject({ ...reporting, snapshotAgeMs: expect.any(Number) });
  });
});

describe('parseDeviceInternals', () => {
  it('accepts a snapshot, keeping null as null', () => {
    expect(parseDeviceInternals(JSON.parse(JSON.stringify(reporting)))).toEqual(reporting);
  });

  it('accepts a missing snapshot', () => {
    const none = {
      connected: true,
      supported: true,
      connectionGeneration: 1,
      snapshotAgeMs: null,
      snapshot: null,
    };
    expect(parseDeviceInternals(none)).toEqual(none);
  });

  it('rejects unknown tokens and malformed figures', () => {
    const bad = (patch: object): unknown => ({
      ...reporting,
      snapshot: { ...snapshot, ...patch },
    });
    expect(() => parseDeviceInternals(bad({ resetReason: 'cosmicRay' }))).toThrow();
    expect(() => parseDeviceInternals(bad({ chipModel: 'esp8266' }))).toThrow();
    expect(() => parseDeviceInternals(bad({ framesIn: -1 }))).toThrow();
    expect(() => parseDeviceInternals({ ...reporting, connectionGeneration: -1 })).toThrow();
    expect(() => parseDeviceInternals({ ...reporting, snapshotAgeMs: 1.5 })).toThrow();
    expect(() => parseDeviceInternals({ ...reporting, snapshotAgeMs: undefined })).toThrow();
    expect(() => parseDeviceInternals(bad({ busyPercent: 101 }))).toThrow();
    expect(() => parseDeviceInternals(bad({ temperatureDeciCelsius: 3.5 }))).toThrow();
    expect(() => parseDeviceInternals(null)).toThrow();
  });
});
