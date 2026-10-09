import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import type { ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui/tooltip';
import type { ConnectionStatusDto, DeskStatusDto } from '../../../lib/ipc/types';

vi.mock('../../../lib/ipc', () => ({ playMascotAction: vi.fn(() => Promise.resolve()) }));

import { HomePage } from '../HomePage';

function desk(overrides: Partial<DeskStatusDto> = {}): DeskStatusDto {
  return {
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
    ...overrides,
  };
}

function connection(overrides: Partial<ConnectionStatusDto> = {}): ConnectionStatusDto {
  return {
    connection: 'connected',
    desired: 'idle',
    reported: 'idle',
    device: {
      firmwareVersion: '0.4.0',
      protocolVersion: { major: 1, minor: 3 },
      deviceIdHashShort: 'deadbeef',
    },
    incompatibleReason: null,
    retryCount: 0,
    connectionGeneration: 1,
    mascotInteraction: true,
    mascotAction: null,
    ...overrides,
  };
}

function view(d: DeskStatusDto | null, c: ConnectionStatusDto | null = connection()): ReactElement {
  return (
    <TooltipProvider>
      <HomePage desk={d} connection={c} onNavigate={() => {}} />
    </TooltipProvider>
  );
}

describe('HomePage', () => {
  it('renders every null as an explicit "—" with a reason, never 0', async () => {
    render(view(desk()));
    for (const id of ['desk-volume', 'desk-cpu', 'desk-ram', 'desk-media', 'desk-title']) {
      expect(screen.getByTestId(id)).toHaveTextContent(/^—$/);
    }
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
    expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
    expect(screen.queryByText('High load')).not.toBeInTheDocument();
    const user = userEvent.setup();
    await user.hover(within(screen.getByTestId('desk-media')).getByRole('button'));
    expect(
      await screen.findByText('This computer can’t report playback for the current player.'),
    ).toBeInTheDocument();
  });

  it('shows live values, mute, high load and what is playing', () => {
    render(
      view(
        desk({
          volumePercent: 35,
          muted: true,
          media: 'paused',
          cpuPercent: 91,
          ramPercent: 50,
          highLoad: true,
          mediaTitle: 'Weightless',
          mediaArtist: 'Marconi Union',
        }),
      ),
    );
    expect(screen.getByTestId('desk-volume')).toHaveTextContent('35%');
    expect(screen.getByTestId('desk-muted')).toHaveTextContent('Muted');
    expect(screen.getByTestId('desk-cpu')).toHaveTextContent('91%');
    expect(screen.getByTestId('desk-ram')).toHaveTextContent('50%');
    expect(screen.getByText('High load')).toBeInTheDocument();
    expect(screen.getByTestId('desk-media')).toHaveTextContent('Paused');
    expect(screen.getByTestId('desk-title')).toHaveTextContent('Weightless');
    expect(screen.getByTestId('desk-artist')).toHaveTextContent('Marconi Union');
  });

  it('distinguishes a player that shared no title from an unknown one', () => {
    render(view(desk({ media: 'playing', mediaTitle: '', mediaArtist: '' })));
    expect(screen.getByTestId('desk-title')).toHaveTextContent('Untitled');
    expect(screen.getByTestId('desk-artist')).toHaveTextContent('Unknown artist');
  });

  it('never presents an unverified action as a success', () => {
    render(
      view(
        desk({
          lastAction: { action: 'shortcut', result: 'unverified', permissionRequired: false },
        }),
      ),
    );
    const badge = screen.getByText('Sent — result unknown');
    expect(badge).toHaveAttribute('data-tone', 'neutral');
    expect(badge.className).not.toMatch(/success/);
    expect(badge.querySelector('svg.lucide-circle-check, svg.lucide-check')).toBeNull();
    expect(screen.queryByText('Confirmed')).not.toBeInTheDocument();
    expect(screen.queryByText(/Accessibility/)).not.toBeInTheDocument();
  });

  it('labels confirmed and started results as the only successes', () => {
    const first = render(
      view(
        desk({
          lastAction: { action: 'mute', result: 'stateConfirmed', permissionRequired: false },
        }),
      ),
    );
    expect(screen.getByText('Confirmed')).toHaveAttribute('data-tone', 'success');
    first.unmount();
    render(
      view(
        desk({
          lastAction: { action: 'launch', result: 'executionConfirmed', permissionRequired: false },
        }),
      ),
    );
    expect(screen.getByText('Started')).toHaveAttribute('data-tone', 'success');
  });

  it('shows the Accessibility hint when permission is required', () => {
    render(
      view(
        desk({ lastAction: { action: 'playPause', result: 'error', permissionRequired: true } }),
      ),
    );
    expect(screen.getByText('Failed')).toHaveAttribute('data-tone', 'error');
    expect(
      screen.getByText(/System Settings → Privacy & Security → Accessibility/),
    ).toBeInTheDocument();
  });

  it('draws the real keycap mascot on the Buddy screen, not the old blob face', () => {
    const { container } = render(view(desk({ mode: 'buddy' })));
    expect(screen.getByTestId('buddy-mascot')).toHaveAttribute(
      'src',
      expect.stringContaining('mascot.svg'),
    );
    // The old blob blinked its CSS eyes; the mascot is one image and nothing blinks.
    expect(container.querySelector('.motion-safe\\:animate-blink')).toBeNull();
    expect(screen.getByRole('img').querySelectorAll('img')).toHaveLength(1);
  });

  it('guides the user to plug in Kivori when disconnected', () => {
    render(view(desk(), connection({ connection: 'disconnected', device: null, reported: null })));
    expect(screen.getByText('Plug in your Kivori')).toBeInTheDocument();
  });

  it('explains an incompatible device and its reason', () => {
    render(
      view(
        desk(),
        connection({
          connection: 'incompatible',
          incompatibleReason: 'protocol v2 is unsupported',
        }),
      ),
    );
    expect(screen.getByText('Kivori needs a firmware update')).toBeInTheDocument();
    expect(screen.getByText('protocol v2 is unsupported')).toBeInTheDocument();
  });

  it('shows skeletons, not values, while loading', () => {
    render(view(null, null));
    expect(screen.queryByTestId('desk-cpu')).not.toBeInTheDocument();
  });

  it('has no axe violations', async () => {
    const { container } = render(
      view(
        desk({
          volumePercent: 10,
          lastAction: { action: 'mute', result: 'unverified', permissionRequired: true },
        }),
      ),
    );
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
