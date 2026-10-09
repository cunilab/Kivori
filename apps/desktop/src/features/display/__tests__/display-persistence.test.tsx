import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it } from 'vitest';
import { useConfig, useDeskStatus } from '../../../hooks/use-kivori';
import { getConfig, resetConfig } from '../../../lib/ipc';
import { DisplayPage } from '../DisplayPage';

// No `vi.mock`: these run against the real wrappers and the in-memory browser mock, the way the
// dev app does, so a saved change is read back through `getConfig` and the config event.
function Page(): ReactElement {
  return <DisplayPage desk={useDeskStatus()} config={useConfig()} connection={null} />;
}

beforeEach(async () => {
  localStorage.clear();
  await resetConfig();
});

describe('display and buddy settings through the browser mock', () => {
  it('persist a double-press view and a default view, and the device follows the default', async () => {
    const user = userEvent.setup();
    render(<Page />);
    const double = await screen.findByRole('group', { name: 'Double press shows' });
    await waitFor(() =>
      expect(within(double).getByRole('button', { name: 'System' })).toHaveAttribute(
        'aria-pressed',
        'true',
      ),
    );
    await user.click(within(double).getByRole('button', { name: 'Media' }));
    await waitFor(async () =>
      expect((await getConfig()).display).toEqual({
        defaultView: 'buddy',
        secondaryView: 'media',
      }),
    );

    const defaults = screen.getByRole('group', { name: 'Default view' });
    await user.click(within(defaults).getByRole('button', { name: 'Clock' }));
    await waitFor(async () => expect((await getConfig()).display.defaultView).toBe('clock'));
    expect(screen.getByTestId('on-device').closest('label')).toHaveAttribute('data-mode', 'clock');
  });

  it('persist the buddy settings across a remount', async () => {
    const user = userEvent.setup();
    const first = render(<Page />);
    await user.click(await screen.findByRole('button', { name: 'Low' }));
    await user.click(screen.getByRole('switch', { name: 'Reactions' }));
    await waitFor(async () =>
      expect((await getConfig()).buddy).toEqual({ reactions: false, intensity: 'low' }),
    );
    first.unmount();

    render(<Page />);
    await waitFor(() =>
      expect(screen.getByRole('switch', { name: 'Reactions' })).not.toBeChecked(),
    );
    expect(screen.getByRole('button', { name: 'Low' })).toHaveAttribute('aria-pressed', 'true');
  });
});
