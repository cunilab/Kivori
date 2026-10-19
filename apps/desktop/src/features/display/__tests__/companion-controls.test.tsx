import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConfigDto } from '../../../lib/ipc/types';

const h = vi.hoisted(() => ({
  setBuddy: vi.fn<(...args: unknown[]) => Promise<void>>(() => Promise.resolve()),
  play: vi.fn<(...args: unknown[]) => Promise<void>>(() => Promise.resolve()),
}));

vi.mock('../../../lib/ipc', () => ({
  setBuddySettings: h.setBuddy,
  playMascotAction: h.play,
}));

import { CompanionControls } from '../CompanionControls';

const config: ConfigDto = {
  version: 1,
  revision: 1,
  notice: null,
  profiles: [],
  macros: [],
  display: { defaultView: 'buddy', secondaryView: 'system', brightness: 5 },
  buddy: { reactions: true, intensity: 'normal' },
};

beforeEach(() => {
  h.setBuddy.mockClear().mockResolvedValue(undefined);
  h.play.mockClear();
  localStorage.clear();
});

describe('CompanionControls', () => {
  it('shows the saved settings to everyone and saves a change through setBuddySettings', async () => {
    const user = userEvent.setup();
    render(<CompanionControls config={config} />);
    expect(screen.getByRole('switch', { name: 'Reactions' })).toBeChecked();
    expect(screen.getByRole('button', { name: 'Normal' })).toHaveAttribute('aria-pressed', 'true');
    expect(h.setBuddy).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'High' }));
    expect(h.setBuddy).toHaveBeenLastCalledWith(true, 'high');
    expect(screen.getByRole('button', { name: 'High' })).toHaveAttribute('aria-pressed', 'true');

    await user.click(screen.getByRole('switch', { name: 'Reactions' }));
    expect(h.setBuddy).toHaveBeenLastCalledWith(false, 'high');
    // Nothing is kept in the webview any more.
    expect(localStorage.length).toBe(0);
  });

  it('follows the saved config once it arrives', () => {
    const { rerender } = render(<CompanionControls config={config} />);
    rerender(
      <CompanionControls
        config={{ ...config, revision: 2, buddy: { reactions: false, intensity: 'low' } }}
      />,
    );
    expect(screen.getByRole('switch', { name: 'Reactions' })).not.toBeChecked();
    expect(screen.getByRole('button', { name: 'Low' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('waits for the config before offering the settings', () => {
    render(<CompanionControls config={null} />);
    expect(screen.getByRole('switch', { name: 'Reactions' })).toHaveAttribute(
      'aria-disabled',
      'true',
    );
  });

  it('keeps direct reactions out of saved settings even in developer mode', () => {
    localStorage.setItem('kivori.developerMode', 'true');
    render(<CompanionControls config={config} />);
    expect(screen.queryByRole('button', { name: 'Greet' })).not.toBeInTheDocument();
  });

  it('reverts a failed save, says so, and retries the same settings', async () => {
    h.setBuddy.mockRejectedValueOnce(new Error('disk full'));
    const user = userEvent.setup();
    render(<CompanionControls config={config} />);

    await user.click(screen.getByRole('button', { name: 'Low' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(/could not save/i);
    expect(screen.getByRole('button', { name: 'Normal' })).toHaveAttribute('aria-pressed', 'true');

    await user.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(h.setBuddy).toHaveBeenCalledTimes(2));
    expect(h.setBuddy).toHaveBeenLastCalledWith(true, 'low');
    await waitFor(() => expect(screen.queryByText(/could not save/i)).not.toBeInTheDocument());
  });

  it('has no axe accessibility violations', async () => {
    const { container } = render(<CompanionControls config={config} />);
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
