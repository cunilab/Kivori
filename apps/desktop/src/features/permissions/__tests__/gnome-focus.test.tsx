import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as ipc from '../../../lib/ipc/gnome';
import type { GnomeFocusStatus } from '../../../lib/ipc/gnome';
import { GnomeFocusHelp } from '../GnomeFocusHelp';
import { strings } from '@/lib/i18n/strings';

const t = strings.gnomeFocus;

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
    expect(await screen.findByText(t.title)).toBeVisible();
    expect(screen.getByText(t.privacy)).toBeVisible();
    expect(screen.getByText(t.once)).toBeVisible();
    expect(ipc.enableGnomeFocus).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: t.enable }));
    expect(await screen.findByText(t.logoutTitle)).toBeVisible();
    expect(ipc.enableGnomeFocus).toHaveBeenCalledTimes(1);
    expect(ipc.logoutGnome).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: t.logout }));
    await waitFor(() => expect(ipc.logoutGnome).toHaveBeenCalledTimes(1));
  });

  it.each<GnomeFocusStatus>(['disabled', 'unavailable'])(
    'offers Enable when %s',
    async (status) => {
      vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue(status);
      render(<GnomeFocusHelp />);
      expect(await screen.findByRole('button', { name: t.enable })).toBeVisible();
    },
  );

  it('keeps security facts in a disclosure that is closed by default', async () => {
    render(<GnomeFocusHelp />);
    const summary = await screen.findByText(t.howItWorks);
    const disclosure = summary.closest('details');
    expect(disclosure).not.toHaveAttribute('open');
    expect(screen.getByText(t.privacy)).toBeVisible();
    for (const fact of Object.values(t.details)) {
      expect(screen.getByText(fact)).not.toBeVisible();
    }
    fireEvent.click(summary);
    expect(disclosure).toHaveAttribute('open');
    for (const fact of Object.values(t.details)) {
      expect(screen.getByText(fact)).toBeVisible();
    }
  });

  it('offers an update for an outdated extension and then asks for the one log-out', async () => {
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('updateAvailable');
    render(<GnomeFocusHelp />);
    expect(await screen.findByText(t.updateTitle)).toBeVisible();
    expect(screen.getByText(t.updateOnce)).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: t.update }));
    expect(await screen.findByText(t.logoutTitle)).toBeVisible();
    expect(ipc.enableGnomeFocus).toHaveBeenCalledTimes(1);
    expect(ipc.logoutGnome).not.toHaveBeenCalled();
  });

  it('offers a log-out when the desktop cannot confirm setup', async () => {
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('cliMissing');
    render(<GnomeFocusHelp />);
    expect(await screen.findByText(t.cliMissingTitle)).toBeVisible();
    expect(screen.getByText(t.cliMissing)).toBeVisible();
    expect(screen.getByText(t.privacy)).toBeVisible();
    expect(screen.getByRole('button', { name: t.logout })).toBeVisible();
  });

  it('explains an unsupported version without an install action', async () => {
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('unsupported');
    render(<GnomeFocusHelp />);
    expect(await screen.findByText(t.unsupported)).toBeVisible();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('shows a failed status read instead of silently hiding setup, then recovers', async () => {
    vi.mocked(ipc.getGnomeFocusStatus).mockRejectedValueOnce(new Error('Status command failed'));
    render(<GnomeFocusHelp />);
    expect(await screen.findByText(t.readFailed)).toBeVisible();
    expect(screen.getByRole('button', { name: t.enable })).toBeVisible();
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
    });
    await waitFor(() => expect(screen.queryByText(t.readFailed)).not.toBeInTheDocument());
    expect(screen.getByRole('button', { name: t.enable })).toBeVisible();
  });

  it('shows an installation failure and permits retry', async () => {
    vi.mocked(ipc.enableGnomeFocus).mockRejectedValueOnce(new Error('Disk is full'));
    render(<GnomeFocusHelp />);
    fireEvent.click(await screen.findByRole('button', { name: t.enable }));
    expect(await screen.findByText(t.installFailed)).toHaveAttribute('role', 'alert');
    expect(screen.queryByText('Disk is full')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: t.enable }));
    expect(await screen.findByText(t.logoutTitle)).toBeVisible();
  });

  it('refreshes on returning from GNOME and disappears once working', async () => {
    render(<GnomeFocusHelp />);
    await screen.findByRole('button', { name: t.enable });
    vi.mocked(ipc.getGnomeFocusStatus).mockResolvedValue('working');
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
    });
    expect(screen.queryByTestId('gnome-focus-help')).not.toBeInTheDocument();
  });
});
