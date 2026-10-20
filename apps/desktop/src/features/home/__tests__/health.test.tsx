import { act, render, screen, waitFor, within } from '@testing-library/react';
import axe from 'axe-core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import { setDevMode } from '../../../lib/dev-mode';
import { mockGetDiagnostics } from '../../../lib/ipc/mock';
import type { ConnectionStatusDto, DiagnosticsDto } from '../../../lib/ipc/types';
import { connectionFixture } from '../../../test/connection';

const bridge = vi.hoisted(() => ({ diagnostics: vi.fn(), internals: vi.fn(), activity: vi.fn() }));
vi.mock('../../../lib/ipc', () => ({
  getDiagnostics: bridge.diagnostics,
  getDeviceInternals: bridge.internals,
  getActivityLog: bridge.activity,
}));
import { HomePage } from '../HomePage';

const report = (
  generation = 1,
  state: ConnectionStatusDto['connection'] = 'connected',
): DiagnosticsDto => ({
  ...mockGetDiagnostics(),
  connection: { ...mockGetDiagnostics().connection, generation, state, reconnects: 7 },
});
const view = (connection: ConnectionStatusDto | null = connectionFixture()) => (
  <TooltipProvider>
    <HomePage desk={null} connection={connection} onNavigate={() => {}} />
  </TooltipProvider>
);
const total = (): HTMLElement =>
  within(screen.getByRole('region', { name: 'Link health' })).getByText('Total reconnects')
    .nextElementSibling as HTMLElement;

beforeEach(() => {
  setDevMode(false);
  vi.clearAllMocks();
  bridge.diagnostics.mockReset().mockResolvedValue(report());
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  setDevMode(false);
});

describe('Home link health', () => {
  it('stays absent and reads nothing with developer mode off', () => {
    render(view());
    expect(screen.queryByRole('region', { name: 'Link health' })).not.toBeInTheDocument();
    expect(bridge.diagnostics).not.toHaveBeenCalled();
    expect(bridge.internals).not.toHaveBeenCalled();
    expect(bridge.activity).not.toHaveBeenCalled();
  });

  it('shows native reconnect totals and current status, including measured zero retries', async () => {
    setDevMode(true);
    const page = render(view());
    await waitFor(() => expect(total()).toHaveTextContent(/^7$/));
    const region = screen.getByRole('region', { name: 'Link health' });
    expect(within(region).getByText('Connected')).toBeInTheDocument();
    expect(within(region).getByText('Consecutive retries').nextElementSibling).toHaveTextContent(
      /^0$/,
    );
    expect(within(region).getByText(/first connection is not counted/)).toBeInTheDocument();
    expect(bridge.internals).not.toHaveBeenCalled();
    expect(bridge.activity).not.toHaveBeenCalled();
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
  });

  it.each(['disconnected', 'connecting', 'incompatible', 'error'] as const)(
    'keeps native totals separate from consecutive attempts while %s',
    async (state) => {
      setDevMode(true);
      bridge.diagnostics.mockResolvedValue(report(1, state));
      render(view(connectionFixture([], { connection: state, retryCount: 3 })));
      await waitFor(() => expect(total()).toHaveTextContent(/^7$/));
      expect(
        within(screen.getByRole('region', { name: 'Link health' })).getByText('Consecutive retries')
          .nextElementSibling,
      ).toHaveTextContent(/^3$/);
    },
  );

  it('keeps unknown status and totals unavailable rather than inventing zero', () => {
    setDevMode(true);
    render(view(null));
    expect(total()).toHaveTextContent('Unknown');
    expect(
      within(screen.getByRole('region', { name: 'Link health' })).getAllByLabelText(/^Unknown\. /),
    ).toHaveLength(3);
    expect(bridge.diagnostics).not.toHaveBeenCalled();
  });

  it('rejects mismatched generations and reports read failures as unavailable', async () => {
    setDevMode(true);
    bridge.diagnostics.mockResolvedValue(report(2));
    const page = render(view());
    await waitFor(() => expect(bridge.diagnostics).toHaveBeenCalledOnce());
    expect(total()).toHaveTextContent('Unknown');
    bridge.diagnostics.mockRejectedValue(new Error('Unavailable'));
    page.rerender(view(connectionFixture([], { connectionGeneration: 2 })));
    await waitFor(() => expect(bridge.diagnostics).toHaveBeenCalledTimes(2));
    expect(total()).toHaveTextContent('Unknown');
  });

  it('removes a previously known total when reading diagnostics fails', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    setDevMode(true);
    render(view());
    await waitFor(() => expect(total()).toHaveTextContent(/^7$/));
    bridge.diagnostics.mockRejectedValue(new Error('Unavailable'));
    await act(() => vi.advanceTimersByTimeAsync(1100));
    expect(total()).toHaveTextContent('Unknown');
  });

  it('clears ended-session totals and rejects an old in-flight reply after reconnect', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    setDevMode(true);
    const page = render(view());
    await waitFor(() => expect(total()).toHaveTextContent(/^7$/));
    let resolveOld!: (value: DiagnosticsDto) => void;
    bridge.diagnostics.mockReturnValueOnce(
      new Promise<DiagnosticsDto>((resolve) => {
        resolveOld = resolve;
      }),
    );
    await act(() => vi.advanceTimersByTimeAsync(1100));
    let resolveNew!: (value: DiagnosticsDto) => void;
    bridge.diagnostics.mockReturnValueOnce(
      new Promise<DiagnosticsDto>((resolve) => {
        resolveNew = resolve;
      }),
    );
    page.rerender(view(connectionFixture([], { connectionGeneration: 2 })));
    expect(total()).toHaveTextContent('Unknown');
    await act(async () => {
      resolveOld(report());
    });
    expect(total()).toHaveTextContent('Unknown');
    await act(async () => {
      resolveNew({ ...report(2), connection: { ...report(2).connection, reconnects: 8 } });
    });
    expect(total()).toHaveTextContent(/^8$/);
    expect(bridge.internals).not.toHaveBeenCalled();
  });

  it('ignores pending reads resolved after hiding or unmounting', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    setDevMode(true);
    let resolve!: (value: DiagnosticsDto) => void;
    bridge.diagnostics.mockReturnValueOnce(
      new Promise<DiagnosticsDto>((done) => {
        resolve = done;
      }),
    );
    const page = render(view());
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    act(() => document.dispatchEvent(new Event('visibilitychange')));
    await act(async () => {
      resolve(report());
    });
    expect(total()).toHaveTextContent('Unknown');
    page.unmount();
    hidden.mockReturnValue(false);
    bridge.diagnostics.mockReturnValueOnce(
      new Promise<DiagnosticsDto>((done) => {
        resolve = done;
      }),
    );
    const nextPage = render(view());
    const calls = bridge.diagnostics.mock.calls.length;
    nextPage.unmount();
    await act(async () => {
      resolve(report());
    });
    await act(() => vi.advanceTimersByTimeAsync(3000));
    expect(bridge.diagnostics).toHaveBeenCalledTimes(calls);
    expect(screen.queryByRole('region', { name: 'Link health' })).not.toBeInTheDocument();
  });

  it('stops cache reads while hidden, disabled or unmounted', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    setDevMode(true);
    const page = render(view());
    await waitFor(() => expect(total()).toHaveTextContent(/^7$/));
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    act(() => document.dispatchEvent(new Event('visibilitychange')));
    expect(total()).toHaveTextContent('Unknown');
    const calls = bridge.diagnostics.mock.calls.length;
    await act(() => vi.advanceTimersByTimeAsync(3000));
    expect(bridge.diagnostics).toHaveBeenCalledTimes(calls);
    hidden.mockReturnValue(false);
    act(() => document.dispatchEvent(new Event('visibilitychange')));
    await act(() => vi.advanceTimersByTimeAsync(1100));
    expect(bridge.diagnostics.mock.calls.length).toBeGreaterThan(calls);
    act(() => setDevMode(false));
    const stopped = bridge.diagnostics.mock.calls.length;
    await act(() => vi.advanceTimersByTimeAsync(3000));
    expect(bridge.diagnostics).toHaveBeenCalledTimes(stopped);
    expect(screen.queryByRole('region', { name: 'Link health' })).not.toBeInTheDocument();
    act(() => setDevMode(true));
    await waitFor(() => expect(total()).toHaveTextContent(/^7$/));
    const beforeUnmount = bridge.diagnostics.mock.calls.length;
    page.unmount();
    await act(() => vi.advanceTimersByTimeAsync(3000));
    expect(bridge.diagnostics).toHaveBeenCalledTimes(beforeUnmount);
    expect(bridge.internals).not.toHaveBeenCalled();
    expect(bridge.activity).not.toHaveBeenCalled();
  });
});
