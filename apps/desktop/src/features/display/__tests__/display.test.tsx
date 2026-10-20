import { connectionFixture } from '../../../test/connection';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@kivori/ui/components/sonner';
import type { ConfigDto, DeskStatusDto } from '../../../lib/ipc/types';

const h = vi.hoisted(() => ({
  setMode: vi.fn<(mode: string) => Promise<void>>(() => Promise.resolve()),
  setSettings: vi.fn<(...args: unknown[]) => Promise<void>>(() => Promise.resolve()),
  setBrightness: vi.fn<(step: number) => Promise<void>>(() => Promise.resolve()),
}));

vi.mock('../../../lib/ipc', () => ({
  setDisplayMode: h.setMode,
  setDisplaySettings: h.setSettings,
  setBrightness: h.setBrightness,
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
  mic: { muted: 'unknown', inUse: null, live: false, devices: [] },
  recentApp: null,
};

const config: ConfigDto = {
  version: 1,
  revision: 1,
  notice: null,
  profiles: [],
  macros: [],
  display: { defaultView: 'buddy', secondaryView: 'system', brightness: 5 },
  buddy: { reactions: true, intensity: 'normal' },
};

const connection = connectionFixture(['deskStatusV1']);

const live = (): ReturnType<typeof within> =>
  within(screen.getByRole('radiogroup', { name: 'Show now' }));

function view(d: DeskStatusDto, c: ConfigDto | null = config): ReactElement {
  return (
    <>
      <DisplayPage desk={d} config={c} connection={connection} />
      <Toaster />
    </>
  );
}

beforeEach(() => {
  h.setMode.mockReset().mockResolvedValue(undefined);
  h.setSettings.mockReset().mockResolvedValue(undefined);
  h.setBrightness.mockReset().mockResolvedValue(undefined);
  localStorage.clear();
});

describe('DisplayPage', () => {
  it.each([null, { ...connection, capabilities: [] }])(
    'blocks live view changes without effective support, but saves settings offline (%s)',
    async (status) => {
      const user = userEvent.setup();
      render(<DisplayPage desk={desk} config={config} connection={status} />);
      expect(screen.queryByRole('radiogroup', { name: 'Show now' })).not.toBeInTheDocument();
      expect(screen.queryByTestId('on-device')).not.toBeInTheDocument();
      await user.click(
        within(screen.getByRole('radiogroup', { name: 'Brightness' })).getByRole('radio', {
          name: 'Step 3',
        }),
      );
      expect(h.setBrightness).toHaveBeenCalledWith(3);
      expect(h.setMode).not.toHaveBeenCalled();
    },
  );

  it('desk status alone supports all five live views, without advertising reaction or media-text support', async () => {
    const user = userEvent.setup();
    render(view(desk));
    const live = screen.getByRole('radiogroup', { name: 'Show now' });
    expect(within(live).getAllByRole('radio')).toHaveLength(5);
    await user.click(within(live).getByRole('radio', { name: /^Media/ }));
    expect(h.setMode).toHaveBeenCalledWith('media');
  });

  it('drops pending live selections when the session generation changes', async () => {
    const user = userEvent.setup();
    const page = render(<DisplayPage desk={desk} config={config} connection={connection} />);
    await user.click(
      within(screen.getByRole('radiogroup', { name: 'Show now' })).getByRole('radio', {
        name: /^Media/,
      }),
    );
    expect(screen.getByText('Applying…')).toBeInTheDocument();
    page.rerender(
      <DisplayPage
        desk={desk}
        config={config}
        connection={{ ...connection, connectionGeneration: 2 }}
      />,
    );
    expect(screen.queryByText('Applying…')).not.toBeInTheDocument();
    expect(live().getByRole('radio', { name: /^Buddy/ })).toHaveAttribute('aria-checked', 'true');
  });

  it('checks the mode the device reports', () => {
    render(view(desk));
    expect(live().getByRole('radio', { name: /^Buddy/ })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByTestId('on-device').closest('label')).toHaveAttribute('data-mode', 'buddy');
  });

  it('shows the saved brightness step and saves a new one at once', async () => {
    const user = userEvent.setup();
    render(view(desk));
    const group = screen.getByRole('radiogroup', { name: 'Brightness' });
    expect(within(group).getByRole('radio', { name: 'Step 5' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await user.click(within(group).getByRole('radio', { name: 'Step 1' }));
    expect(h.setBrightness).toHaveBeenCalledWith(1);
    expect(within(group).getByRole('radio', { name: 'Step 1' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });

  it('calls setDisplayMode, highlights optimistically, then follows desk://status', async () => {
    const user = userEvent.setup();
    const { rerender } = render(view(desk));
    await user.click(live().getByRole('radio', { name: /^Clock/ }));
    expect(h.setMode).toHaveBeenCalledWith('clock');
    expect(live().getByRole('radio', { name: /^Clock/ })).toHaveAttribute('aria-checked', 'true');
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
    await user.click(live().getByRole('radio', { name: /^Clock/ }));
    expect(await screen.findByText('unknown display mode: clock')).toBeInTheDocument();
    expect(live().getByRole('radio', { name: /^Buddy/ })).toHaveAttribute('aria-checked', 'true');
  });

  it('falls back to the reported mode when no status arrives in time', async () => {
    vi.useFakeTimers();
    try {
      render(view(desk));
      await act(async () => {
        live()
          .getByRole('radio', { name: /^System/ })
          .click();
      });
      expect(live().getByRole('radio', { name: /^System/ })).toHaveAttribute(
        'aria-checked',
        'true',
      );
      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000);
      });
      expect(live().getByRole('radio', { name: /^Buddy/ })).toHaveAttribute('aria-checked', 'true');
    } finally {
      vi.useRealTimers();
    }
  });

  it('shows the saved default and double-press views', () => {
    render(view(desk));
    const defaults = screen.getByRole('radiogroup', { name: 'Default view' });
    expect(within(defaults).getByRole('radio', { name: 'Buddy' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    const double = screen.getByRole('radiogroup', { name: 'Double press shows' });
    expect(within(double).getByRole('radio', { name: 'System' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    // A double press toggling a view with itself would do nothing.
    expect(within(double).getByRole('radio', { name: 'Buddy' })).toHaveAttribute(
      'aria-disabled',
      'true',
    );
  });

  it('saves a new default view', async () => {
    const user = userEvent.setup();
    render(view(desk));
    await user.click(
      within(screen.getByRole('radiogroup', { name: 'Default view' })).getByRole('radio', {
        name: 'Clock',
      }),
    );
    expect(h.setSettings).toHaveBeenCalledWith('clock', 'system');
  });

  it('swaps the two when the default is set to the double-press view', async () => {
    const user = userEvent.setup();
    render(view(desk));
    await user.click(
      within(screen.getByRole('radiogroup', { name: 'Default view' })).getByRole('radio', {
        name: 'System',
      }),
    );
    expect(h.setSettings).toHaveBeenCalledWith('system', 'buddy');
  });

  it('saves a new double-press view, including cycling through every view', async () => {
    const user = userEvent.setup();
    render(view(desk));
    const double = screen.getByRole('radiogroup', { name: 'Double press shows' });
    await user.click(within(double).getByRole('radio', { name: 'Media' }));
    expect(h.setSettings).toHaveBeenLastCalledWith('buddy', 'media');
    await user.click(within(double).getByRole('radio', { name: 'Every view in turn' }));
    expect(h.setSettings).toHaveBeenLastCalledWith('buddy', 'cycle');
  });

  it('reverts a rejected display setting and toasts the native error', async () => {
    h.setSettings.mockRejectedValueOnce('Settings could not be saved. Nothing was changed.');
    const user = userEvent.setup();
    render(view(desk));
    const defaults = screen.getByRole('radiogroup', { name: 'Default view' });
    await user.click(within(defaults).getByRole('radio', { name: 'Clock' }));
    expect(await screen.findByText(/Nothing was changed/)).toBeInTheDocument();
    expect(within(defaults).getByRole('radio', { name: 'Buddy' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });

  it('has no axe violations', async () => {
    const { container } = render(<DisplayPage desk={desk} config={config} connection={null} />);
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
