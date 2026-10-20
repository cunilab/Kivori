import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { UpdateStatusDto } from '../../../lib/ipc/types';

const bridge = vi.hoisted(() => ({
  get: vi.fn(),
  on: vi.fn(),
  onShow: vi.fn(),
  check: vi.fn(),
  install: vi.fn(),
  snooze: vi.fn(),
  skip: vi.fn(),
  auto: vi.fn(),
}));
vi.mock('../../../lib/ipc', () => ({
  getUpdateStatus: bridge.get,
  onUpdateStatus: bridge.on,
  onUpdateDialogRequested: bridge.onShow,
  checkForUpdates: bridge.check,
  installUpdate: bridge.install,
  snoozeUpdate: bridge.snooze,
  skipUpdateVersion: bridge.skip,
  setAutoUpdateCheck: bridge.auto,
}));

import { AppUpdates } from '../AppUpdates';
import { megabytes, resetUpdateDialog, UpdateDialog } from '../UpdateDialog';

const idle: UpdateStatusDto = {
  state: 'idle',
  reason: null,
  currentVersion: '0.1.0',
  lastChecked: null,
  autoCheck: true,
  update: null,
  progress: null,
};
const release = { version: '0.2.0', notes: '* Smoother knob', date: '2026-10-12T00:00:00Z' };
const available: UpdateStatusDto = { ...idle, state: 'available', update: release };

let emit: (status: UpdateStatusDto) => void = () => {};
let showFromTray: () => void = () => {};

beforeEach(() => {
  resetUpdateDialog();
  bridge.get.mockReset().mockResolvedValue(idle);
  bridge.on.mockReset().mockImplementation((handler: typeof emit) => {
    emit = handler;
    return Promise.resolve(() => {});
  });
  bridge.onShow.mockReset().mockImplementation((handler: () => void) => {
    showFromTray = handler;
    return Promise.resolve(() => {});
  });
  for (const command of [bridge.check, bridge.install, bridge.snooze, bridge.skip, bridge.auto]) {
    command.mockReset().mockResolvedValue(idle);
  }
});

describe('update dialog', () => {
  it('opens once per version with the title, plain-text notes and the three choices', async () => {
    bridge.get.mockResolvedValue({
      ...available,
      update: { ...release, notes: '<img src=x onerror=alert(1)> **bold**' },
    });
    render(<UpdateDialog />);
    expect(
      await screen.findByRole('alertdialog', { name: 'Kivori 0.2.0 is ready' }),
    ).toBeInTheDocument();
    const notes = screen.getByTestId('release-notes');
    expect(notes).toHaveTextContent('<img src=x onerror=alert(1)> **bold**');
    expect(notes.querySelector('img')).toBeNull();
    expect(screen.getByRole('button', { name: 'Install & restart' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Later' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Skip this version' })).toBeEnabled();
  });

  it('stays closed while nothing is offered and opens when an update is found', async () => {
    render(<UpdateDialog />);
    await waitFor(() => expect(bridge.on).toHaveBeenCalled());
    expect(screen.queryByRole('alertdialog')).toBeNull();
    act(() => emit(available));
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
  });

  it('does not reopen by itself for the same version, but the tray item reopens it', async () => {
    const user = userEvent.setup();
    bridge.get.mockResolvedValue(available);
    const { unmount } = render(<UpdateDialog />);
    await screen.findByRole('alertdialog');
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    unmount();
    render(<UpdateDialog />);
    await waitFor(() => expect(bridge.onShow).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    act(() => showFromTray());
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
  });

  it('Later and Skip this version call their commands and close', async () => {
    const user = userEvent.setup();
    bridge.get.mockResolvedValue(available);
    render(<UpdateDialog />);
    await user.click(await screen.findByRole('button', { name: 'Later' }));
    expect(bridge.snooze).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    act(() => showFromTray());
    await user.click(await screen.findByRole('button', { name: 'Skip this version' }));
    expect(bridge.skip).toHaveBeenCalledTimes(1);
  });

  it('shows indeterminate progress without a size, then a byte count, then the restart', async () => {
    const user = userEvent.setup();
    bridge.get.mockResolvedValue(available);
    render(<UpdateDialog />);
    await user.click(await screen.findByRole('button', { name: 'Install & restart' }));
    expect(bridge.install).toHaveBeenCalledTimes(1);
    act(() =>
      emit({ ...available, state: 'downloading', progress: { downloaded: 10, total: null } }),
    );
    const bar = await screen.findByRole('progressbar');
    expect(bar).not.toHaveAttribute('aria-valuenow');
    expect(screen.getByText('Downloading the update…')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Later' })).toBeNull();
    act(() =>
      emit({
        ...available,
        state: 'downloading',
        progress: { downloaded: 2_500_000, total: 10_000_000 },
      }),
    );
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');
    expect(screen.getByText('Downloading the update… 2.5 MB of 10.0 MB')).toBeInTheDocument();
    act(() => emit({ ...available, state: 'readyToRestart' }));
    expect(screen.getByText(/Kivori restarts as soon as the device is free/)).toBeInTheDocument();
  });

  it('says plainly when the install failed and offers Try again and Later', async () => {
    const user = userEvent.setup();
    bridge.get.mockResolvedValue({ ...available, state: 'failed' });
    render(<UpdateDialog />);
    // A failed install does not open the dialog by itself; the tray item (or View update) does.
    await act(async () => {});
    expect(screen.queryByRole('alertdialog')).toBeNull();
    act(() => showFromTray());
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The update could not be installed. Nothing was changed.',
    );
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(bridge.install).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: 'Later' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Skip this version' })).toBeNull();
  });

  it('closes when the offer is withdrawn elsewhere', async () => {
    bridge.get.mockResolvedValue(available);
    render(<UpdateDialog />);
    await screen.findByRole('alertdialog');
    act(() => emit(idle));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
  });

  it('formats download sizes for people', () => {
    expect(megabytes(0)).toBe('0.0 MB');
    expect(megabytes(12_345_678)).toBe('12.3 MB');
  });
});

describe('app updates in Settings', () => {
  it('shows the last check, the automatic switch and Check now', async () => {
    const user = userEvent.setup();
    render(<AppUpdates />);
    expect(await screen.findByTestId('update-last-checked')).toHaveTextContent('Not yet');
    const toggle = screen.getByRole('switch', { name: 'Check for updates automatically' });
    expect(toggle).toBeChecked();
    await user.click(toggle);
    expect(bridge.auto).toHaveBeenCalledWith(false);
    await user.click(screen.getByRole('button', { name: 'Check now' }));
    expect(bridge.check).toHaveBeenCalledTimes(1);
  });

  it('says nothing about a failed check and keeps Check now usable', async () => {
    bridge.check.mockRejectedValue(new Error('offline'));
    const user = userEvent.setup();
    render(<AppUpdates />);
    await user.click(await screen.findByRole('button', { name: 'Check now' }));
    expect(screen.queryByRole('alert')).toBeNull();
    act(() => emit({ ...idle, state: 'checking' }));
    expect(screen.getByRole('button', { name: 'Check now' })).toBeDisabled();
    expect(screen.getByText('Checking…')).toBeInTheDocument();
    act(() => emit(idle));
    expect(screen.getByRole('button', { name: 'Check now' })).toBeEnabled();
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('tells a development build, a build without a key and macOS apart, with nothing to press', async () => {
    const cases: [Partial<UpdateStatusDto>, string][] = [
      [{ state: 'disabled', reason: 'devBuild' }, 'development build'],
      [{ state: 'disabled', reason: 'noKey' }, 'cannot verify them yet'],
      [{ state: 'unsupported' }, 'not available on macOS yet'],
    ];
    for (const [overrides, text] of cases) {
      bridge.get.mockResolvedValue({ ...idle, ...overrides });
      const { unmount } = render(<AppUpdates />);
      expect(await screen.findByText(new RegExp(text))).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Check now' })).toBeDisabled();
      expect(screen.getByRole('switch')).toHaveAttribute('aria-disabled', 'true');
      unmount();
    }
  });

  it('offers to view an available update, which opens the dialog', async () => {
    const user = userEvent.setup();
    bridge.get.mockResolvedValue(available);
    render(
      <>
        <AppUpdates />
        <UpdateDialog />
      </>,
    );
    expect(await screen.findByText('Kivori 0.2.0 is ready.')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    await user.click(screen.getByRole('button', { name: 'View update' }));
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
  });
});
