import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui/tooltip';
import type { DiagnosticsDto } from '../../../lib/ipc/types';
import { parseDiagnostics } from '../../../lib/ipc/validate';

const bridge = vi.hoisted(() => ({ get: vi.fn() }));
vi.mock('../../../lib/ipc', () => ({ getDiagnostics: bridge.get }));

import { Diagnostics } from '../Diagnostics';

const disconnected: DiagnosticsDto = {
  versions: {
    app: '0.1.0',
    firmware: null,
    protocol: '1.4',
    negotiatedMinor: null,
    capabilities: [],
    deviceHash: null,
  },
  connection: {
    state: 'disconnected',
    connectedForSecs: null,
    reconnects: 0,
    retryCount: 0,
    lastPongAgeMs: null,
    rttMs: null,
  },
  health: {
    deviceUptimeMs: null,
    freeBytes: null,
    malformedFrames: 0,
    sequenceGaps: 0,
    deviceErrors: 0,
  },
  host: {
    systemVolume: 'available',
    appVolume: 'unsupported',
    media: 'observable',
    focus: 'detecting',
    inputPermission: 'unknown',
  },
  config: { status: 'ok', schemaVersion: 1, customBindings: 0, macros: 0 },
};

const connected: DiagnosticsDto = {
  ...disconnected,
  versions: {
    ...disconnected.versions,
    firmware: '0.4.0',
    negotiatedMinor: 3,
    capabilities: ['mascotInteraction', 'deskStatusV1'],
    deviceHash: '3fa9c1d2',
  },
  connection: { ...disconnected.connection, state: 'connected', rttMs: 6, lastPongAgeMs: 400 },
  health: { ...disconnected.health, freeBytes: 2048, deviceUptimeMs: 65_000 },
};

const view = (): ReturnType<typeof render> =>
  render(
    <TooltipProvider>
      <Diagnostics />
    </TooltipProvider>,
  );

beforeEach(() => {
  bridge.get.mockReset().mockResolvedValue(disconnected);
});

describe('Diagnostics card', () => {
  it('shows unknown values as placeholders, never zero or a guess', async () => {
    view();
    await screen.findByText('0.1.0');
    expect(screen.getByRole('button', { name: 'Copy diagnostics' })).toBeEnabled();
    // firmware, negotiated minor, hash, connected for, last reply, rtt, uptime, memory
    expect(screen.getAllByLabelText(/^— /)).toHaveLength(8);
    expect(screen.queryByText('0 ms')).not.toBeInTheDocument();
  });

  it('shows known values once the device reports them', async () => {
    bridge.get.mockResolvedValue(connected);
    view();
    await screen.findByText('3fa9c1d2');
    expect(screen.getByText('mascotInteraction, deskStatusV1')).toBeInTheDocument();
    expect(screen.getByText('6 ms')).toBeInTheDocument();
    expect(screen.getByText('2.0 KiB')).toBeInTheDocument();
  });

  it('polls again every second while mounted', async () => {
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
    } finally {
      vi.useRealTimers();
    }
  });

  it('copies the diagnostics as JSON that parses back to the same data', async () => {
    bridge.get.mockResolvedValue(connected);
    const writeText = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    // userEvent installs its own clipboard stub; replace it after setup.
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    view();
    await screen.findByText('3fa9c1d2');
    await user.click(screen.getByRole('button', { name: 'Copy diagnostics' }));
    await screen.findByText('Copied');
    const copied = JSON.parse(writeText.mock.calls[0][0] as string) as unknown;
    expect(parseDiagnostics(copied)).toEqual(connected);
    expect(Object.keys(copied as object)).toEqual([
      'versions',
      'connection',
      'health',
      'host',
      'config',
    ]);
    expect(JSON.stringify(copied)).not.toMatch(/"(port|path|deviceId)"/i);
  });
});
