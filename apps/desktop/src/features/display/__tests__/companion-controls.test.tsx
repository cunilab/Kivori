import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const h = vi.hoisted(() => ({
  configure: vi.fn<(...args: unknown[]) => Promise<void>>(() => Promise.resolve()),
  play: vi.fn<(...args: unknown[]) => Promise<void>>(() => Promise.resolve()),
}));

vi.mock('../../../lib/ipc', () => ({
  configureCompanion: h.configure,
  playMascotAction: h.play,
}));

import { CompanionControls } from '../CompanionControls';

beforeEach(() => {
  h.configure.mockClear();
  h.play.mockClear();
  localStorage.clear();
});

describe('CompanionControls', () => {
  it('persists personality and sends all direct social actions', async () => {
    const user = userEvent.setup();
    render(<CompanionControls connected supported />);
    await waitFor(() => expect(h.configure).toHaveBeenCalledWith('cozy', true));

    await user.click(screen.getByRole('button', { name: 'Playful' }));
    await waitFor(() => expect(h.configure).toHaveBeenLastCalledWith('playful', true));
    expect(localStorage.getItem('kivori.mascot.personality')).toBe('playful');

    for (const action of ['Greet', 'Pet', 'Tickle', 'Surprise', 'Comfort'] as const) {
      await user.click(screen.getByRole('button', { name: action }));
    }
    expect(h.play.mock.calls.map(([action]) => action)).toEqual([
      'greet',
      'pet',
      'tickle',
      'surprise',
      'comfort',
    ]);
  });

  it('explains firmware requirement and disables reactions for unsupported devices', () => {
    render(<CompanionControls connected supported={false} />);
    expect(screen.getByText(/update kivori firmware/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Greet' })).toBeDisabled();
  });

  it('catches a failed settings update and retries current settings', async () => {
    h.configure.mockRejectedValueOnce(new Error('runtime stopped'));
    const user = userEvent.setup();
    render(<CompanionControls connected supported />);

    expect(await screen.findByRole('alert')).toHaveTextContent(/could not save/i);
    await user.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(h.configure).toHaveBeenCalledTimes(2));
    expect(h.configure).toHaveBeenLastCalledWith('cozy', true);
    await waitFor(() => expect(screen.queryByText(/could not save/i)).not.toBeInTheDocument());
  });

  it('has no axe accessibility violations', async () => {
    const { container } = render(<CompanionControls connected supported />);
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
