import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@/components/ui/sonner';
import type { ConfigDto, DeskStatusDto } from '../../../lib/ipc/types';

const h = vi.hoisted(() => ({
  setMode: vi.fn<(mode: string) => Promise<void>>(() => Promise.resolve()),
  setSettings: vi.fn<(...args: unknown[]) => Promise<void>>(() => Promise.resolve()),
}));

vi.mock('../../../lib/ipc', () => ({
  setDisplayMode: h.setMode,
  setDisplaySettings: h.setSettings,
  setBuddySettings: vi.fn(() => Promise.resolve()),
  playMascotAction: vi.fn(() => Promise.resolve()),
}));

import { DisplayPage } from '../DisplayPage';

const desk: DeskStatusDto = {
  mode: 'buddy',
  volumePercent: null,
  muted: null,
  media: null,
  cpuPercent: null,
  ramPercent: null,
  highLoad: false,
  pressAction: 'playPause',
  holdAction: 'mute',
  doublePressAction: 'nextView',
  buttonActions: ['previousTrack', 'playPause', 'nextTrack'],
  profile: null,
  pinned: false,
  rotateLabel: 'Volume',
  buttonLabels: ['Previous', 'Play/Pause', 'Next'],
  buttonHoldActions: [null, null, null],
  profileId: 'general',
  mediaTitle: null,
  mediaArtist: null,
  lastAction: null,
};

const config: ConfigDto = {
  version: 1,
  revision: 1,
  notice: null,
  profiles: [],
  display: { defaultView: 'buddy', secondaryView: 'system' },
  buddy: { reactions: true, intensity: 'normal' },
};

function view(d: DeskStatusDto, c: ConfigDto | null = config): ReactElement {
  return (
    <>
      <DisplayPage desk={d} config={c} connection={null} />
      <Toaster />
    </>
  );
}

beforeEach(() => {
  h.setMode.mockReset().mockResolvedValue(undefined);
  h.setSettings.mockReset().mockResolvedValue(undefined);
  localStorage.clear();
});

describe('DisplayPage', () => {
  it('checks the mode the device reports', () => {
    render(view(desk));
    expect(screen.getByRole('radio', { name: /^Buddy/ })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByTestId('on-device').closest('label')).toHaveAttribute('data-mode', 'buddy');
  });

  it('calls setDisplayMode, highlights optimistically, then follows desk://status', async () => {
    const user = userEvent.setup();
    const { rerender } = render(view(desk));
    await user.click(screen.getByRole('radio', { name: /^Clock/ }));
    expect(h.setMode).toHaveBeenCalledWith('clock');
    expect(screen.getByRole('radio', { name: /^Clock/ })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByText('Applying…')).toBeInTheDocument();
    // The device still shows Buddy until the native status says otherwise.
    expect(screen.getByTestId('on-device').closest('label')).toHaveAttribute('data-mode', 'buddy');
    rerender(view({ ...desk, mode: 'clock' }));
    expect(screen.queryByText('Applying…')).not.toBeInTheDocument();
    expect(screen.getByTestId('on-device').closest('label')).toHaveAttribute('data-mode', 'clock');
  });

  it('reverts the highlight and toasts the native rejection', async () => {
    h.setMode.mockRejectedValueOnce('unknown display mode: clock');
    const user = userEvent.setup();
    render(view(desk));
    await user.click(screen.getByRole('radio', { name: /^Clock/ }));
    expect(await screen.findByText('unknown display mode: clock')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /^Buddy/ })).toHaveAttribute('aria-checked', 'true');
  });

  it('falls back to the reported mode when no status arrives in time', async () => {
    vi.useFakeTimers();
    try {
      render(view(desk));
      await act(async () => {
        screen.getByRole('radio', { name: /^System/ }).click();
      });
      expect(screen.getByRole('radio', { name: /^System/ })).toHaveAttribute(
        'aria-checked',
        'true',
      );
      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000);
      });
      expect(screen.getByRole('radio', { name: /^Buddy/ })).toHaveAttribute('aria-checked', 'true');
    } finally {
      vi.useRealTimers();
    }
  });

  it('shows the saved default and double-press views', () => {
    render(view(desk));
    const defaults = screen.getByRole('group', { name: 'Default view' });
    expect(within(defaults).getByRole('button', { name: 'Buddy' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    const double = screen.getByRole('group', { name: 'Double press shows' });
    expect(within(double).getByRole('button', { name: 'System' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    // A double press toggling a view with itself would do nothing.
    expect(within(double).getByRole('button', { name: 'Buddy' })).toBeDisabled();
  });

  it('saves a new default view', async () => {
    const user = userEvent.setup();
    render(view(desk));
    await user.click(
      within(screen.getByRole('group', { name: 'Default view' })).getByRole('button', {
        name: 'Clock',
      }),
    );
    expect(h.setSettings).toHaveBeenCalledWith('clock', 'system');
  });

  it('swaps the two when the default is set to the double-press view', async () => {
    const user = userEvent.setup();
    render(view(desk));
    await user.click(
      within(screen.getByRole('group', { name: 'Default view' })).getByRole('button', {
        name: 'System',
      }),
    );
    expect(h.setSettings).toHaveBeenCalledWith('system', 'buddy');
  });

  it('saves a new double-press view, including cycling through every view', async () => {
    const user = userEvent.setup();
    render(view(desk));
    const double = screen.getByRole('group', { name: 'Double press shows' });
    await user.click(within(double).getByRole('button', { name: 'Media' }));
    expect(h.setSettings).toHaveBeenLastCalledWith('buddy', 'media');
    await user.click(within(double).getByRole('button', { name: 'Every view in turn' }));
    expect(h.setSettings).toHaveBeenLastCalledWith('buddy', 'cycle');
  });

  it('reverts a rejected display setting and toasts the native error', async () => {
    h.setSettings.mockRejectedValueOnce('Settings could not be saved. Nothing was changed.');
    const user = userEvent.setup();
    render(view(desk));
    const defaults = screen.getByRole('group', { name: 'Default view' });
    await user.click(within(defaults).getByRole('button', { name: 'Clock' }));
    expect(await screen.findByText(/Nothing was changed/)).toBeInTheDocument();
    expect(within(defaults).getByRole('button', { name: 'Buddy' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('has no axe violations', async () => {
    const { container } = render(<DisplayPage desk={desk} config={config} connection={null} />);
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
