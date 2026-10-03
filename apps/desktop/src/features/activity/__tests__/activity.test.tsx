import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ActivityEventDto } from '../../../lib/ipc/types';

type Deferred<T> = {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
};

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const h = vi.hoisted(() => ({
  calls: [] as string[],
  history: null as Promise<unknown[]> | null,
  subscribe: null as Promise<() => void> | null,
  emit: null as ((event: unknown) => void) | null,
}));

vi.mock('../../../lib/ipc', () => ({
  getActivityLog: (limit: number): Promise<unknown[]> => {
    h.calls.push(`history:${limit}`);
    return h.history ?? Promise.resolve([]);
  },
  onActivityLog: (handler: (event: unknown) => void): Promise<() => void> => {
    h.calls.push('subscribe');
    h.emit = handler;
    return h.subscribe ?? Promise.resolve(() => {});
  },
}));

import { strings } from '../../../lib/i18n/strings';
import { ActivityPage as Log, VIEW_LIMIT } from '../ActivityPage';

function event(overrides: Partial<ActivityEventDto> = {}): ActivityEventDto {
  return {
    id: 1,
    at: '2026-09-13T10:00:00Z',
    type: 'connectionAttempted',
    summary: 'Connection attempt started.',
    severity: 'info',
    source: 'connection',
    outcome: 'started',
    metadata: null,
    ...overrides,
  };
}

async function finishSetup(events: ActivityEventDto[] = []): Promise<void> {
  h.subscribe ??= Promise.resolve(() => {});
  h.history ??= Promise.resolve(events);
  await waitFor(() =>
    expect(screen.getByRole('status')).toHaveTextContent(strings.log.status.live),
  );
}

type User = ReturnType<typeof userEvent.setup>;

async function pickSeverity(user: User, name: string): Promise<void> {
  const group = screen.getByRole('group', { name: strings.log.filters.severity });
  await user.click(within(group).getByRole('button', { name }));
}

async function pickSource(user: User, name: string): Promise<void> {
  await user.click(screen.getByRole('combobox', { name: strings.log.filters.source }));
  await user.click(await screen.findByRole('option', { name }));
}

afterEach(() => {
  h.calls = [];
  h.history = null;
  h.subscribe = null;
  h.emit = null;
  vi.clearAllMocks();
});

describe('Activity view', () => {
  it('waits for the subscription handle before requesting history', async () => {
    const subscription = deferred<() => void>();
    h.subscribe = subscription.promise;
    h.history = Promise.resolve([]);
    render(<Log />);
    expect(h.calls).toEqual(['subscribe']);
    await act(async () => subscription.resolve(() => {}));
    await waitFor(() => expect(h.calls).toEqual(['subscribe', `history:${VIEW_LIMIT}`]));
  });

  it('keeps a live event arriving during history exactly once and prefers its copy by id', async () => {
    const history = deferred<unknown[]>();
    h.history = history.promise;
    render(<Log />);
    await finishSetup();
    act(() => h.emit?.(event({ id: 2, summary: 'Live copy wins.' })));
    await act(async () => history.resolve([event({ id: 2, summary: 'History copy loses.' })]));
    await waitFor(() => expect(screen.getByText(/Live copy wins/)).toBeInTheDocument());
    expect(screen.queryByText(/History copy loses/)).not.toBeInTheDocument();
    expect(screen.getAllByRole('row')).toHaveLength(2);
  });

  it('sorts merged history and live entries newest first by numeric id', async () => {
    const history = deferred<unknown[]>();
    h.history = history.promise;
    render(<Log />);
    await finishSetup();
    act(() => h.emit?.(event({ id: 3, summary: 'third' })));
    await act(async () =>
      history.resolve([event({ id: 2, summary: 'second' }), event({ id: 1, summary: 'first' })]),
    );
    await waitFor(() => expect(screen.getAllByRole('row')).toHaveLength(4));
    expect(
      screen
        .getAllByRole('row')
        .slice(1)
        .map((row) => row.textContent),
    ).toEqual([
      expect.stringContaining('third'),
      expect.stringContaining('second'),
      expect.stringContaining('first'),
    ]);
  });

  it('combines independent filters without deleting session history', async () => {
    h.history = Promise.resolve([
      event({ id: 1, severity: 'info', source: 'connection', summary: 'connection info' }),
      event({ id: 2, severity: 'error', source: 'device', summary: 'device error' }),
      event({ id: 3, severity: 'error', source: 'connection', summary: 'connection error' }),
    ]);
    const user = userEvent.setup();
    render(<Log />);
    await finishSetup();
    await waitFor(() => expect(screen.getByText(/connection info/)).toBeInTheDocument());
    await pickSeverity(user, 'Error');
    await pickSource(user, 'Connection');
    expect(screen.getByText(/connection error/)).toBeInTheDocument();
    expect(screen.queryByText(/device error/)).not.toBeInTheDocument();
    expect(screen.queryByText(/connection info/)).not.toBeInTheDocument();
    await pickSeverity(user, 'All');
    await pickSource(user, 'All sources');
    expect(screen.getByText(/connection info/)).toBeInTheDocument();
    expect(screen.getByText(/device error/)).toBeInTheDocument();
  });

  it('distinguishes an empty session from filters with no matches', async () => {
    const empty = render(<Log />);
    await finishSetup();
    expect(screen.getByText(strings.log.empty)).toBeInTheDocument();
    empty.unmount();
    h.history = Promise.resolve([event({ severity: 'info', source: 'connection' })]);
    const populated = render(<Log />);
    await finishSetup();
    const user = userEvent.setup();
    await pickSeverity(user, 'Error');
    expect(populated.getByText(strings.log.noMatches)).toBeInTheDocument();
    expect(populated.queryByText(strings.log.empty)).not.toBeInTheDocument();
    await user.click(populated.getByRole('button', { name: strings.log.clearFilters }));
    expect(populated.queryByText(strings.log.noMatches)).not.toBeInTheDocument();
  });

  it('shows starting, live, and safe unavailable setup statuses without error text', async () => {
    const subscription = deferred<() => void>();
    h.subscribe = subscription.promise;
    render(<Log />);
    expect(screen.getByRole('status')).toHaveTextContent(strings.log.status.starting);
    await act(async () => subscription.reject(new Error('COM9 secret flasher output')));
    await waitFor(() =>
      expect(screen.getByRole('status')).toHaveTextContent(strings.log.status.unavailable),
    );
    expect(screen.queryByText(/COM9|secret|flasher output/)).not.toBeInTheDocument();
  });

  it('shows the same safe unavailable status when history rejects after subscribing', async () => {
    h.subscribe = Promise.resolve(() => {});
    h.history = Promise.reject(new Error('/home/secret/raw-error.txt'));
    render(<Log />);
    await waitFor(() =>
      expect(screen.getByRole('status')).toHaveTextContent(strings.log.status.unavailable),
    );
    expect(screen.queryByText(/home|secret|raw-error/)).not.toBeInTheDocument();
  });

  it('unsubscribes on cleanup after setup and when setup resolves after unmount', async () => {
    const unlisten = vi.fn();
    h.subscribe = Promise.resolve(unlisten);
    const mounted = render(<Log />);
    await finishSetup();
    mounted.unmount();
    expect(unlisten).toHaveBeenCalledOnce();
    const lateUnlisten = vi.fn();
    const lateSubscription = deferred<() => void>();
    h.subscribe = lateSubscription.promise;
    const pending = render(<Log />);
    pending.unmount();
    await act(async () => lateSubscription.resolve(lateUnlisten));
    expect(lateUnlisten).toHaveBeenCalledOnce();
  });

  it('retains the newest 256 entries in the session log', async () => {
    h.history = Promise.resolve(
      Array.from({ length: VIEW_LIMIT + 4 }, (_, index) =>
        event({ id: index + 1, summary: `entry ${index + 1}` }),
      ),
    );
    render(<Log />);
    await finishSetup();
    await waitFor(() => expect(screen.getAllByRole('row')).toHaveLength(VIEW_LIMIT + 1));
    expect(screen.queryByText(/entry 1$/)).not.toBeInTheDocument();
    expect(screen.getByText(new RegExp(`entry ${VIEW_LIMIT + 4}$`))).toBeInTheDocument();
  });

  it('renders only fixed allowlisted metadata and rejects runtime-cast sensitive extras', async () => {
    const unsafe = {
      ...event({
        metadata: {
          connection: 'connected',
          retryCount: 2,
          elapsedMs: 120,
          diagnosticCategory: 'checksum',
          diagnosticCode: 7,
          firmwareVersion: '1.2.3',
          protocolVersion: { major: 1, minor: 0 },
          deviceIdHashShort: 'deadbeef',
          capabilities: 3,
          state: 'happy',
          personality: 'cozy',
          selfPlay: true,
          action: 'greet',
          seed: 9,
          appliedAtMs: 42,
          autonomous: false,
          protocolCategory: 'checksum',
          payloadLen: 8,
          sequence: 4,
          skipped: 1,
          reported: 'idle',
          rawPayload: '0xdeadbeef',
          deviceId: '0123456789abcdef0123456789abcdef',
          port: 'COM9',
          windowsPath: 'C:\\Users\\secret\\flash.bin',
          posixPath: '/home/secret/flash.bin',
          token: 'super-secret-token',
          flasherOutput: 'esptool raw output',
        } as unknown as NonNullable<ActivityEventDto['metadata']>,
      }),
    } as unknown as ActivityEventDto;
    h.history = Promise.resolve([unsafe]);
    const view = render(<Log />);
    await finishSetup();
    await waitFor(() => expect(screen.getByText(/deadbeef/)).toBeInTheDocument());
    const text = view.container.textContent ?? '';
    expect(text).toContain('checksum');
    expect(text).toContain('1.2.3');
    expect(text).not.toMatch(
      /0xdeadbeef|0123456789abcdef|COM9|C:\\Users|\/home\/secret|super-secret|esptool/i,
    );
  });

  it('renders desk action events with their action detail', async () => {
    h.history = Promise.resolve([
      event({
        id: 7,
        type: 'deskActionUnverified',
        summary: 'Desk action sent; result unknown.',
        source: 'action',
        outcome: 'observed',
        metadata: { retryCount: 0, elapsedMs: 0, action: 'playPause' },
      }),
    ]);
    render(<Log />);
    await finishSetup();
    expect(await screen.findByText('Desk action unverified')).toBeInTheDocument();
    expect(screen.getByText(/Desk action sent; result unknown/)).toBeInTheDocument();
    expect(screen.getByText('action playPause')).toBeInTheDocument();
  });

  it('has no axe violations for empty, populated, and filtered log states', async () => {
    const empty = render(<Log />);
    await finishSetup();
    expect(
      (await axe.run(empty.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
    empty.unmount();
    h.history = Promise.resolve([event({ severity: 'error', source: 'device' })]);
    const populated = render(<Log />);
    await finishSetup();
    const user = userEvent.setup();
    await pickSeverity(user, 'Info');
    expect(
      (await axe.run(populated.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
  });
});
