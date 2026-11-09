import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as ipc from '../../../lib/ipc/gnome';
import type { GnomeFocusStatus } from '../../../lib/ipc/gnome';
import { GnomeFocusHelp } from '../GnomeFocusHelp';

vi.mock('../../../lib/ipc/gnome', () => ({
  getGnomeFocusStatus: vi.fn(),
  enableGnomeFocus: vi.fn(),
  logoutGnome: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('notInstalled');
  vi.mocked(ipc.enableGnomeFocus).mockResolvedValue('needsLogout');
  vi.mocked(ipc.logoutGnome).mockResolvedValue();
});
afterEach(() => vi.clearAllMocks());

describe('GNOME Wayland profile setup', () => {
  it.each<GnomeFocusStatus>(['notApplicable', 'working'])('hides on %s', async (status) => {
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue(status);
    const { container } = render(<GnomeFocusHelp />);
    await waitFor(() => expect(ipc.getGnomeFocusStatus).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
    expect(ipc.enableGnomeFocus).not.toHaveBeenCalled();
  });

  it('explains privacy and the one-time logout before explicit opt-in', async () => {
    render(<GnomeFocusHelp />);
    expect(await screen.findByText("Let Kivori see which app you're using")).toBeVisible();
    expect(screen.getByText(/No window titles or contents/)).toBeVisible();
    expect(screen.getByText('One log-out is needed the first time.')).toBeVisible();
    expect(ipc.enableGnomeFocus).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Enable' }));
    expect(await screen.findByText('Log out and back in once to finish')).toBeVisible();
    expect(ipc.enableGnomeFocus).toHaveBeenCalledTimes(1);
    expect(ipc.logoutGnome).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Log out now' }));
    await waitFor(() => expect(ipc.logoutGnome).toHaveBeenCalledTimes(1));
  });

  it.each<GnomeFocusStatus>(['disabled', 'unavailable'])(
    'offers Enable when %s',
    async (status) => {
      vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue(status);
      render(<GnomeFocusHelp />);
      expect(await screen.findByRole('button', { name: 'Enable' })).toBeVisible();
    },
  );

  it('states exactly what is shared and with whom', async () => {
    render(<GnomeFocusHelp />);
    expect(await screen.findByText(/only learns which app is in front/)).toBeVisible();
    expect(screen.getByText(/Other programs you run could ask/)).toBeVisible();
  });

  it('offers an update for an outdated extension and then asks for the one log-out', async () => {
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('updateAvailable');
    render(<GnomeFocusHelp />);
    expect(await screen.findByText('Update Kivori App Profiles')).toBeVisible();
    expect(screen.getByText('One log-out is needed to finish the update.')).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'Update' }));
    expect(await screen.findByText('Log out and back in once to finish')).toBeVisible();
    expect(ipc.enableGnomeFocus).toHaveBeenCalledTimes(1);
    expect(ipc.logoutGnome).not.toHaveBeenCalled();
  });

  it('says plainly when gnome-extensions is missing instead of insisting on a log-out title', async () => {
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('cliMissing');
    render(<GnomeFocusHelp />);
    expect(await screen.findByText("Kivori can't check the extension")).toBeVisible();
    expect(screen.getByText(/gnome-extensions tool/)).toBeVisible();
    expect(screen.queryByText('Log out and back in once to finish')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Log out now' })).toBeVisible();
  });

  it('explains an unsupported version without an install action', async () => {
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('unsupported');
    render(<GnomeFocusHelp />);
    expect(await screen.findByText(/supports GNOME 46–50/)).toBeVisible();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('shows an installation failure and permits retry', async () => {
    vi.mocked(ipc.enableGnomeFocus).mockRejectedValueOnce(new Error('Disk is full'));
    render(<GnomeFocusHelp />);
    fireEvent.click(await screen.findByRole('button', { name: 'Enable' }));
    expect(await screen.findByRole('alert', { name: '' })).toBeVisible();
    expect(await screen.findByText('Disk is full')).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'Enable' }));
    expect(await screen.findByText('Log out and back in once to finish')).toBeVisible();
  });

  it('refreshes on returning from GNOME and disappears once working', async () => {
    render(<GnomeFocusHelp />);
    await screen.findByRole('button', { name: 'Enable' });
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('working');
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
    });
    expect(screen.queryByTestId('gnome-focus-help')).not.toBeInTheDocument();
  });
});
