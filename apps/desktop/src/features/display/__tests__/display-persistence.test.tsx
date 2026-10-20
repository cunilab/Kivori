import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it } from 'vitest';
import { resetPageTabs } from '../../../components/kivori/page-tabs';
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
  resetPageTabs();
  await resetConfig();
});

describe('display and buddy settings through the browser mock', () => {
  it('persists a double-press view and a default view offline', async () => {
    const user = userEvent.setup();
    render(<Page />);
    const double = await screen.findByRole('radiogroup', { name: 'Double press shows' });
    await waitFor(() =>
      expect(within(double).getByRole('radio', { name: 'System' })).toHaveAttribute(
        'aria-checked',
        'true',
      ),
    );
    await user.click(within(double).getByRole('radio', { name: 'Media' }));
    await waitFor(async () =>
      expect((await getConfig()).display).toEqual({
        defaultView: 'buddy',
        secondaryView: 'media',
        brightness: 5,
      }),
    );

    const defaults = screen.getByRole('radiogroup', { name: 'Default view' });
    await user.click(within(defaults).getByRole('radio', { name: 'Clock' }));
    await waitFor(async () => expect((await getConfig()).display.defaultView).toBe('clock'));
    expect(screen.queryByTestId('on-device')).not.toBeInTheDocument();
  });

  it('persists the brightness across a remount and keeps it when a view is changed', async () => {
    const user = userEvent.setup();
    const first = render(<Page />);
    await user.click(await screen.findByRole('tab', { name: 'Brightness' }));
    const group = await screen.findByRole('radiogroup', { name: 'Brightness' });
    await user.click(within(group).getByRole('radio', { name: 'Step 2' }));
    await waitFor(async () => expect((await getConfig()).display.brightness).toBe(2));
    first.unmount();

    render(<Page />);
    // The page comes back on the tab it was left on.
    expect(await screen.findByRole('radiogroup', { name: 'Brightness' })).toBeVisible();
    await user.click(screen.getByRole('tab', { name: 'Views' }));
    const defaults = await screen.findByRole('radiogroup', { name: 'Default view' });
    await user.click(within(defaults).getByRole('radio', { name: 'Clock' }));
    await waitFor(async () => expect((await getConfig()).display.defaultView).toBe('clock'));
    expect((await getConfig()).display.brightness).toBe(2);
    await user.click(screen.getByRole('tab', { name: 'Brightness' }));
    expect(
      within(screen.getByRole('radiogroup', { name: 'Brightness' })).getByRole('radio', {
        name: 'Step 2',
      }),
    ).toHaveAttribute('aria-checked', 'true');
  });

  it('persist the buddy settings across a remount', async () => {
    const user = userEvent.setup();
    const first = render(<Page />);
    await user.click(await screen.findByRole('tab', { name: 'Buddy' }));
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
