import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@kivori/ui/components/sonner';
import { useConfig, useDeskStatus } from '../../../hooks/use-kivori';

const ICON = 'data:image/png;base64,iVBORw0KGgo=';

// The browser mock behind the real IPC wrappers, except icons: Figma has one, nothing else does.
vi.mock('../../../lib/ipc', async (importOriginal) => {
  const real = await importOriginal<typeof import('../../../lib/ipc')>();
  return {
    ...real,
    getAppIcon: vi.fn((id: string) => Promise.resolve(id === 'figma.exe' ? ICON : null)),
  };
});

import * as ipc from '../../../lib/ipc';
import * as mock from '../../../lib/ipc/mock';
import { resetPageTabs } from '../../../components/kivori/page-tabs';
import { ControlsPage } from '../ControlsPage';

function Harness(): ReactElement {
  const desk = useDeskStatus();
  const config = useConfig();
  return (
    <>
      <ControlsPage desk={desk} config={config} />
      <Toaster />
    </>
  );
}

const profile = (id: string): HTMLElement => screen.getByTestId(`profile-${id}`);

/** The profile panel follows the selected tab. */
async function show(user: ReturnType<typeof userEvent.setup>, name: string): Promise<HTMLElement> {
  await user.click(await screen.findByRole('tab', { name }));
  return screen.findByTestId(`profile-${name.toLowerCase()}`);
}

/** Expands the New profile block under the profile chips. */
async function openNew(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  await user.click(await screen.findByText('New profile', { selector: 'summary' }));
}

beforeEach(async () => {
  window.history.pushState({}, '', '/');
  resetPageTabs();
  await ipc.resetConfig();
  act(() => mock.mockSetRecentApp(null));
});

describe('ProfilesCard', () => {
  it('shows the selected profile with friendly app names and letter avatars where there is no icon', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('profile-list');
    const browser = await show(user, 'Browser');
    expect(browser).toHaveTextContent('Built-in');
    expect(within(browser).getByTestId('app-browser-chrome.exe')).toHaveTextContent('Chrome');
    expect(screen.getByTestId('app-avatar-chrome.exe')).toHaveTextContent('C');
    expect(await show(user, 'General')).toHaveTextContent('Every app that no other profile lists.');
  });

  it('shows six apps, then the rest behind a +N more toggle', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('profile-list');
    const code = await show(user, 'Code');
    expect(within(code).getByText('IntelliJ IDEA')).toBeVisible();
    expect(within(code).getAllByTestId(/^app-code-/)).toHaveLength(6);
    await user.click(within(code).getByRole('button', { name: '+4 more' }));
    expect(within(code).getAllByTestId(/^app-code-/)).toHaveLength(10);
    await user.click(within(code).getByRole('button', { name: 'Show fewer' }));
    expect(within(code).getAllByTestId(/^app-code-/)).toHaveLength(6);
  });

  it('shows an app once when a profile stores several ids for it', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('profile-list');
    await ipc.assignApp('browser', 'chrome');
    await ipc.assignApp('browser', 'com.google.chrome');
    const browser = await show(user, 'Browser');
    expect(within(browser).getAllByText('Chrome')).toHaveLength(1);
    // Removing the merged app sends every alias back to General.
    await user.click(within(browser).getByRole('button', { name: 'Options for Chrome' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Remove Chrome' }));
    await waitFor(() => expect(within(browser).queryByText('Chrome')).not.toBeInTheDocument());
    const stored = (await ipc.getConfig()).profiles.find((p) => p.id === 'browser');
    expect(stored?.apps.filter((id) => /chrome/.test(id))).toEqual([]);
  });

  it('hides the other OS identifiers on macOS', async () => {
    window.history.pushState({}, '', '/?mock=mac');
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('profile-list');
    await ipc.assignApp('browser', 'com.google.chrome');
    const browser = await show(user, 'Browser');
    expect(await within(browser).findByTestId('app-browser-com.google.chrome')).toBeVisible();
    expect(within(browser).queryByTestId('app-browser-chrome.exe')).toBeNull();
    expect(within(browser).getAllByText('Chrome')).toHaveLength(1);
  });

  it('disables "Add the app in front of me" with an explanation until an app has been seen', async () => {
    render(<Harness />);
    await openNew(userEvent.setup());
    const button = await screen.findByRole('button', { name: 'Add the app in front of me' });
    expect(button).toBeDisabled();
    expect(screen.getByText(/Switch to the app you want/)).toBeVisible();
    act(() => mock.mockSetRecentApp({ id: 'figma.exe', name: 'Figma' }));
    expect(await screen.findByRole('button', { name: 'Add Figma' })).toBeEnabled();
  });

  it('adds the app in front as a custom profile with its icon, copying General', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('profile-list');
    await openNew(user);
    act(() => mock.mockSetRecentApp({ id: 'figma.exe', name: 'Figma' }));
    await user.click(await screen.findByRole('button', { name: 'Add Figma' }));
    const added = await screen.findByTestId('profile-custom-1');
    expect(added).toHaveTextContent('Figma');
    expect(added).toHaveTextContent('Your profile');
    expect(await within(added).findByTestId('app-icon-figma.exe')).toHaveAttribute('src', ICON);
    // It is selected and shows General's bindings.
    expect(await screen.findByRole('tab', { name: 'Figma', selected: true })).toBeVisible();
    expect(screen.getByTestId('gesture-press')).toHaveTextContent('Play / Pause');
    expect(screen.getByText(/Later changes to General do not flow into it/)).toBeVisible();
  });

  it('moves an app between profiles and says where it came from', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await show(user, 'Browser');
    await user.click(await screen.findByRole('button', { name: 'Options for Chrome' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Move to Code' }));
    expect(await screen.findByText('Moved Chrome from Browser to Code')).toBeVisible();
    await waitFor(() =>
      expect(screen.queryByTestId('app-browser-chrome.exe')).not.toBeInTheDocument(),
    );
    await show(user, 'Code');
    await user.click(await screen.findByRole('button', { name: /more$/ }));
    expect(await screen.findByTestId('app-code-chrome.exe')).toBeVisible();
    // Remove sends it to General, which lists no apps.
    await user.click(screen.getByRole('button', { name: 'Options for Chrome' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Remove Chrome' }));
    await waitFor(() =>
      expect(screen.queryByTestId('app-code-chrome.exe')).not.toBeInTheDocument(),
    );
  });

  it('adds a typed app to a built-in profile, moving it from where it was', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('profile-list');
    await show(user, 'Code');
    await user.click(within(profile('code')).getByRole('button', { name: /Add app/ }));
    await user.type(await screen.findByLabelText('Or type its name'), 'Firefox.exe');
    await user.click(screen.getByRole('button', { name: 'Add' }));
    expect(await screen.findByText('Moved Firefox from Browser to Code')).toBeVisible();
    await user.click(await screen.findByRole('button', { name: /more$/ }));
    expect(await screen.findByTestId('app-code-firefox.exe')).toBeVisible();
  });

  it('picks a running app for a new profile', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openNew(user);
    await user.click(await screen.findByRole('button', { name: 'Pick a running app' }));
    const dialog = await screen.findByRole('dialog');
    expect(await within(dialog).findByText('In Browser')).toBeVisible();
    await user.click(await within(dialog).findByRole('button', { name: /OBS Studio/ }));
    expect(await screen.findByTestId('profile-custom-1')).toHaveTextContent('OBS Studio');
  });

  it('renames with validation and shows a refusal from the core', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await ipc.addCustomProfile('figma.exe', 'Figma');
    await user.click(await screen.findByRole('tab', { name: 'Browser' }));
    expect(
      within(profile('browser')).queryByRole('button', { name: 'Profile options for Browser' }),
    ).toBeNull();
    await user.click(await screen.findByRole('tab', { name: 'Figma' }));
    const card = await screen.findByTestId('profile-custom-1');
    await user.click(within(card).getByRole('button', { name: 'Profile options for Figma' }));
    await user.click(await screen.findByRole('menuitem', { name: /Rename/ }));
    const input = within(card).getByLabelText('Profile name');
    await user.clear(input);
    expect(within(card).getByRole('button', { name: 'Save name' })).toBeDisabled();
    await user.type(input, 'x'.repeat(33));
    expect(within(card).getByRole('button', { name: 'Save name' })).toBeDisabled();
    expect(within(card).getByText('The name can have up to 32 characters.')).toBeVisible();
    await user.clear(input);
    await user.type(input, '  Design ');
    await user.click(within(card).getByRole('button', { name: 'Save name' }));
    await waitFor(() => expect(card).toHaveTextContent('Design'));
    expect(screen.getByRole('tab', { name: 'Design' })).toBeVisible();
  });

  it('toggles the pin cycle, off by default and only for custom profiles', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await ipc.addCustomProfile('figma.exe', 'Figma');
    await user.click(await screen.findByRole('tab', { name: 'Code' }));
    expect(within(profile('code')).queryByRole('switch')).toBeNull();
    await user.click(await screen.findByRole('tab', { name: 'Figma' }));
    const card = await screen.findByTestId('profile-custom-1');
    const toggle = within(card).getByRole('switch', { name: 'In pin cycle' });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    await waitFor(() => expect(toggle).toBeChecked());
  });

  it('deletes a custom profile after asking, and its apps return', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await ipc.addCustomProfile('chrome.exe', 'Mine');
    await user.click(await screen.findByRole('tab', { name: 'Mine' }));
    const card = await screen.findByTestId('profile-custom-1');
    await user.click(within(card).getByRole('button', { name: 'Profile options for Mine' }));
    await user.click(await screen.findByRole('menuitem', { name: /Delete/ }));
    const dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Keep it' }));
    expect(screen.getByTestId('profile-custom-1')).toBeVisible();
    await user.click(within(card).getByRole('button', { name: 'Profile options for Mine' }));
    await user.click(await screen.findByRole('menuitem', { name: /Delete/ }));
    await user.click(
      within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Delete' }),
    );
    await waitFor(() => expect(screen.queryByTestId('profile-custom-1')).not.toBeInTheDocument());
    await user.click(await screen.findByRole('tab', { name: 'Browser' }));
    expect(await screen.findByTestId('app-browser-chrome.exe')).toBeVisible();
  });

  it('stops at 16 custom profiles', async () => {
    render(<Harness />);
    await openNew(userEvent.setup());
    for (let n = 0; n < 16; n += 1) await ipc.addCustomProfile(`app${n}.exe`, `App ${n}`);
    act(() => mock.mockSetRecentApp({ id: 'figma.exe', name: 'Figma' }));
    expect(await screen.findByRole('button', { name: 'Add Figma' })).toBeDisabled();
    expect(screen.getByText(/most custom profiles \(16\)/)).toBeVisible();
    await expect(ipc.addCustomProfile('one-more.exe', '')).rejects.toThrow(/at most 16/);
  });

  it('resets a custom profile to a fresh copy of General', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await ipc.addCustomProfile('figma.exe', 'Figma');
    await ipc.setBinding('custom-1', 'press', { action: { kind: 'nextTrack' } });
    await user.click(await screen.findByRole('tab', { name: 'Figma' }));
    expect(screen.getByTestId('gesture-press')).toHaveTextContent('Next track');
    await user.click(screen.getByRole('tab', { name: 'Reset & help' }));
    await user.click(screen.getByRole('button', { name: 'Reset to General' }));
    await user.click(
      within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Reset' }),
    );
    await user.click(screen.getByRole('tab', { name: 'Controls' }));
    await waitFor(() =>
      expect(screen.getByTestId('gesture-press')).toHaveTextContent('Play / Pause'),
    );
  });
});
