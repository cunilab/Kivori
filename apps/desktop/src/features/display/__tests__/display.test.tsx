import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@/components/ui/sonner';
import type { DeskStatusDto } from '../../../lib/ipc/types';

const h = vi.hoisted(() => ({
  setMode: vi.fn<(mode: string) => Promise<void>>(() => Promise.resolve()),
}));

vi.mock('../../../lib/ipc', () => ({
  setDisplayMode: h.setMode,
  configureCompanion: vi.fn(() => Promise.resolve()),
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
  mediaTitle: null,
  mediaArtist: null,
  lastAction: null,
};

function view(d: DeskStatusDto): ReactElement {
  return (
    <>
      <DisplayPage desk={d} connection={null} />
      <Toaster />
    </>
  );
}

beforeEach(() => {
  h.setMode.mockReset().mockResolvedValue(undefined);
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

  it('has no axe violations', async () => {
    const { container } = render(<DisplayPage desk={desk} connection={null} />);
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
