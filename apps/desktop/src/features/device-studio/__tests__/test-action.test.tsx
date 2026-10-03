import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const h = vi.hoisted(() => ({
  run: vi.fn<(request: unknown) => Promise<void>>(() => Promise.resolve()),
}));

vi.mock('../../../lib/ipc', () => ({ runTestAction: h.run }));

import { TestAction } from '../TestAction';

beforeEach(() => {
  h.run.mockReset();
  h.run.mockResolvedValue(undefined);
});

describe('TestAction', () => {
  it('runs play/pause and mute', async () => {
    const user = userEvent.setup();
    render(<TestAction />);
    await user.click(screen.getByRole('button', { name: 'Play/Pause' }));
    await user.click(screen.getByRole('button', { name: 'Mute' }));
    expect(h.run.mock.calls.map(([request]) => request)).toEqual([
      { action: 'playPause' },
      { action: 'mute' },
    ]);
  });

  it('submits a shortcut and shows the native rejection string', async () => {
    h.run.mockRejectedValueOnce('unknown key: Banana');
    const user = userEvent.setup();
    render(<TestAction />);
    await user.type(screen.getByLabelText('Shortcut'), 'Ctrl+Banana');
    await user.click(screen.getByRole('button', { name: 'Run shortcut' }));
    expect(h.run).toHaveBeenCalledWith({ action: 'shortcut', shortcut: 'Ctrl+Banana' });
    expect(await screen.findByRole('alert')).toHaveTextContent('unknown key: Banana');
  });

  it('submits a launch target on Enter', async () => {
    const user = userEvent.setup();
    render(<TestAction />);
    await user.type(screen.getByLabelText('Application'), 'Notes{Enter}');
    expect(h.run).toHaveBeenCalledWith({ action: 'launch', target: 'Notes' });
  });

  it('has no axe violations', async () => {
    const { container } = render(<TestAction />);
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
