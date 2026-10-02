import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DeskStatusDto } from '../../../lib/ipc/types';

const h = vi.hoisted(() => ({
  status: null as unknown,
  setMode: vi.fn<(mode: string) => Promise<void>>(() => Promise.resolve()),
}));

vi.mock('../../../lib/ipc', () => ({
  getDeskStatus: () => Promise.resolve(h.status),
  onDeskStatus: () => Promise.resolve(() => {}),
  setDisplayMode: h.setMode,
}));

import { DeskPanel } from '../DeskPanel';

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
    lastAction: null,
    ...overrides,
  };
}

beforeEach(() => {
  h.setMode.mockClear();
  h.setMode.mockResolvedValue(undefined);
});

describe('DeskPanel', () => {
  it('renders every null as an explicit unknown, never 0', async () => {
    h.status = desk();
    render(<DeskPanel />);
    await screen.findByText('Press: Play/Pause · Hold: Mute');
    for (const id of ['desk-volume', 'desk-cpu', 'desk-ram']) {
      expect(screen.getByTestId(id)).toHaveTextContent(/^—$/);
    }
    expect(screen.getByTestId('desk-media')).toHaveTextContent('Not observable on this computer');
    expect(screen.queryByText(/High load/)).not.toBeInTheDocument();
  });

  it('shows values, Muted and the High load chip', async () => {
    h.status = desk({
      volumePercent: 35,
      muted: true,
      media: 'paused',
      cpuPercent: 91,
      ramPercent: 50,
      highLoad: true,
    });
    render(<DeskPanel />);
    expect(await screen.findByText('35% · Muted')).toBeInTheDocument();
    expect(screen.getByText('Paused')).toBeInTheDocument();
    expect(screen.getByTestId('desk-cpu')).toHaveTextContent('91%');
    expect(screen.getByText('High load')).toBeInTheDocument();
    expect(screen.getByTestId('desk-ram')).toHaveTextContent('50%');
  });

  it('never presents an unverified action as a success', async () => {
    h.status = desk({
      lastAction: { action: 'shortcut', result: 'unverified', permissionRequired: false },
    });
    render(<DeskPanel />);
    const badge = await screen.findByText('Sent — result unknown');
    expect(badge).toHaveAttribute('data-tone', 'neutral');
    expect(screen.queryByText('Confirmed')).not.toBeInTheDocument();
    expect(badge.querySelector('svg.lucide-circle-check, svg.lucide-check')).toBeNull();
    expect(screen.queryByText(/Accessibility/)).not.toBeInTheDocument();
  });

  it('labels confirmed and started results, with success only for those', async () => {
    h.status = desk({
      lastAction: { action: 'mute', result: 'stateConfirmed', permissionRequired: false },
    });
    const view = render(<DeskPanel />);
    expect(await screen.findByText('Confirmed')).toHaveAttribute('data-tone', 'success');
    view.unmount();
    h.status = desk({
      lastAction: { action: 'launch', result: 'executionConfirmed', permissionRequired: false },
    });
    render(<DeskPanel />);
    expect(await screen.findByText('Started')).toHaveAttribute('data-tone', 'success');
  });

  it('shows the Accessibility hint when permission is required', async () => {
    h.status = desk({
      lastAction: { action: 'playPause', result: 'error', permissionRequired: true },
    });
    render(<DeskPanel />);
    expect(await screen.findByText('Failed')).toBeInTheDocument();
    expect(
      screen.getByText(/System Settings → Privacy & Security → Accessibility/),
    ).toBeInTheDocument();
  });

  it('calls setDisplayMode with the clicked mode and shows a native rejection', async () => {
    h.status = desk();
    h.setMode.mockRejectedValueOnce('unknown display mode: clock');
    const user = userEvent.setup();
    render(<DeskPanel />);
    await screen.findByText('Press: Play/Pause · Hold: Mute');
    expect(screen.getByRole('button', { name: 'Buddy' })).toHaveAttribute('aria-pressed', 'true');
    await user.click(screen.getByRole('button', { name: 'Clock' }));
    expect(h.setMode).toHaveBeenCalledWith('clock');
    expect(await screen.findByRole('alert')).toHaveTextContent('unknown display mode: clock');
  });

  it('has no axe violations', async () => {
    h.status = desk({
      volumePercent: 10,
      lastAction: { action: 'mute', result: 'unverified', permissionRequired: true },
    });
    const { container } = render(<DeskPanel />);
    await waitFor(() => screen.getByText('10%'));
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
