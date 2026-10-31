import { connectionFixture } from '../../../test/connection';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import type { ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import type { ConnectionStatusDto, DeskStatusDto } from '../../../lib/ipc/types';

vi.mock('../../../lib/ipc', () => ({
  playMascotAction: vi.fn(() => Promise.resolve()),
  getDiagnostics: vi.fn(() => new Promise(() => {})),
  getAccessibility: vi.fn(() => Promise.resolve('missing')),
  requestAccessibility: vi.fn(() => Promise.resolve('missing')),
  resetAccessibility: vi.fn(() => Promise.resolve('missing')),
  openAccessibilitySettings: vi.fn(() => Promise.resolve()),
}));

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
    mic: { muted: 'unknown', inUse: null, live: false, devices: [] },
    recentApp: null,
    ...overrides,
  };
}

function connection(overrides: Partial<ConnectionStatusDto> = {}): ConnectionStatusDto {
  return connectionFixture(undefined, overrides);
}

function view(d: DeskStatusDto | null, c: ConnectionStatusDto | null = connection()): ReactElement {
  return (
    <TooltipProvider>
      <HomePage desk={d} connection={c} onNavigate={() => {}} />
    </TooltipProvider>
  );
}

describe('HomePage', () => {
  it.each(['pending', 'unknown', 'identity-only', 'dial'] as const)(
    'shows neutral product status without reactions or screen claims for %s, preserving host monitoring',
    (kind) => {
      localStorage.setItem('kivori.developerMode', 'true');
      const session = connectionFixture(
        kind === 'dial' ? ['physicalInputV1', 'productIdentityV1'] : ['productIdentityV1'],
      );
      if (kind === 'pending') session.device!.identity = null;
      if (kind === 'unknown') session.device!.identity!.known = false;
      if (kind === 'dial') session.device!.identity!.product = 'Test dial without screen';
      session.reported = null;
      render(view(desk({ mediaTitle: 'A host-only reading' }), session));
      expect(screen.queryByTestId('buddy-mascot')).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Change view' })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Say hi' })).not.toBeInTheDocument();
      expect(screen.queryByText('Showing')).not.toBeInTheDocument();
      expect(screen.getByTestId('desk-title')).toHaveTextContent('A host-only reading');
      if (kind === 'pending')
        expect(screen.getByText('Reading device identity…')).toBeInTheDocument();
      if (kind === 'unknown')
        expect(screen.getByText('This device is not supported by this app')).toBeInTheDocument();
      if (kind === 'identity-only')
        expect(screen.getByText('Device firmware needs attention')).toBeInTheDocument();
      localStorage.clear();
    },
  );

  it('survives pending, resolved and disconnected transitions without changing hook order', () => {
    const pending = connectionFixture();
    pending.device!.identity = null;
    const page = render(view(desk(), pending));
    page.rerender(view(desk(), connectionFixture()));
    expect(screen.getByRole('img', { name: 'Kivori Pip · KV-P1' })).toBeInTheDocument();
    page.rerender(view(desk(), connection({ connection: 'disconnected', device: null })));
    expect(screen.getByText('Plug in your Kivori')).toBeInTheDocument();
  });

  it('tints a meter by its level', () => {
    render(view(desk({ cpuPercent: 31, ramPercent: 83, volumePercent: 95 })));
    expect(screen.getByTestId('desk-cpu')).toHaveAttribute('data-tone', 'normal');
    expect(screen.getByTestId('desk-ram')).toHaveAttribute('data-tone', 'warning');
    expect(screen.getByTestId('desk-volume')).toHaveAttribute('data-tone', 'normal');
  });

  it('renders every null as an explicit "Unknown" with a reason, never 0', async () => {
    render(view(desk()));
    for (const id of ['desk-volume', 'desk-cpu', 'desk-ram', 'desk-media', 'desk-title']) {
      expect(screen.getByTestId(id)).toHaveTextContent(/^Unknown$/);
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

  it('shows the microphones: muted, a Live cue only when in use and unmuted, unknown shows none', () => {
    const mic = (m: Partial<DeskStatusDto['mic']>): DeskStatusDto['mic'] => ({
      muted: 'no',
      inUse: null,
      live: false,
      devices: [
        { name: 'Built-in', state: 'live' },
        { name: 'USB headset', state: 'live' },
      ],
      ...m,
    });
    const { unmount } = render(view(desk({ mic: mic({}) })));
    expect(screen.getByTestId('mic-muted')).toHaveTextContent('On');
    expect(screen.queryByTestId('mic-live')).not.toBeInTheDocument();
    expect(screen.getByTestId('mic-devices')).toHaveTextContent('USB headset');
    // The scope is stated: the computer's microphones, not an app's own mute button.
    expect(screen.getAllByText(/not an app’s own mute button/).length).toBeGreaterThan(0);
    unmount();

    const live = render(view(desk({ mic: mic({ inUse: true, live: true }) })));
    expect(screen.getByTestId('mic-live')).toHaveTextContent('Live');
    live.unmount();

    const muted = render(
      view(
        desk({
          mic: mic({
            muted: 'yes',
            devices: [
              { name: 'Built-in', state: 'muted' },
              { name: 'USB headset', state: 'muted' },
            ],
          }),
        }),
      ),
    );
    expect(screen.getByTestId('mic-muted')).toHaveTextContent('Muted');
    expect(screen.queryByTestId('mic-live')).not.toBeInTheDocument();
    muted.unmount();

    // A device that stayed live is named, and the whole is never "Muted".
    const partial = render(
      view(
        desk({
          mic: mic({
            muted: 'partial',
            devices: [
              { name: 'Built-in', state: 'muted' },
              { name: 'USB headset', state: 'live' },
            ],
          }),
        }),
      ),
    );
    expect(screen.getByTestId('mic-muted')).toHaveTextContent('Partly muted');
    expect(screen.getByTestId('mic-devices')).toHaveTextContent('USB headset');
    partial.unmount();

    // Unknown: nothing claims live or muted.
    render(view(desk()));
    expect(screen.getByTestId('mic-muted')).toHaveTextContent('Unknown');
    expect(screen.queryByTestId('mic-live')).not.toBeInTheDocument();
    expect(screen.getByTestId('mic-none')).toHaveTextContent(/can’t read/);
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

  it('shows the Accessibility hint when permission is required', async () => {
    render(
      view(
        desk({ lastAction: { action: 'playPause', result: 'error', permissionRequired: true } }),
      ),
    );
    expect(screen.getByText('Failed')).toHaveAttribute('data-tone', 'error');
    expect(
      await screen.findByText(/System Settings → Privacy & Security → Accessibility/),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Allow access' })).toBeInTheDocument();
  });

  it('shows catalog name/model and a neutral figure when the product has no artwork', () => {
    render(view(desk({ mode: 'buddy' })));
    const art = screen.getByRole('img', { name: 'Kivori Pip · KV-P1' });
    expect(within(art).getByText('Kivori Pip')).toBeInTheDocument();
    expect(within(art).getByText('KV-P1')).toBeInTheDocument();
    expect(within(art).getByTestId('product-art-neutral')).toBeInTheDocument();
    expect(art.querySelector('img')).toBeNull();
    expect(screen.queryByTestId('buddy-mascot')).not.toBeInTheDocument();
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
          incompatibleReason: 'protocol v1 is unsupported',
        }),
      ),
    );
    expect(screen.getByText('Kivori needs a firmware update')).toBeInTheDocument();
    expect(screen.getByText('protocol v1 is unsupported')).toBeInTheDocument();
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
