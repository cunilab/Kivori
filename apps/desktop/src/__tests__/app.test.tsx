import * as hooks from '../hooks/use-kivori';
import * as ipc from '../lib/ipc';
import type { ConnectionStatusDto } from '../lib/ipc/types';
import { connectionFixture } from '../test/connection';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from '../App';
import { resetPageTabs } from '../components/kivori/page-tabs';
import { setDevMode } from '../lib/dev-mode';

const DEV_MODE_KEY = 'kivori.developerMode';
const navLabels = (): (string | null)[] =>
  within(screen.getByRole('navigation', { name: 'Main navigation' }))
    .getAllByRole('button')
    .map((b) => b.textContent);
const goSettings = (user: ReturnType<typeof userEvent.setup>) =>
  user.click(screen.getByRole('button', { name: 'Settings' }));
const goDeveloper = async (user: ReturnType<typeof userEvent.setup>): Promise<void> => {
  await goSettings(user);
  await user.click(screen.getByRole('tab', { name: 'Developer' }));
};

beforeEach(() => {
  resetPageTabs();
  localStorage.clear();
});
afterEach(() => vi.restoreAllMocks());

describe('App shell', () => {
  it('offers four device pages with Settings last in the same landmark', async () => {
    render(<App />);
    expect(navLabels()).toEqual(['Home', 'Controls', 'Display', 'Device', 'Settings']);
    expect(screen.getByRole('button', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Developer' })).not.toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
  });

  it('navigates between sections and marks the current one', async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole('button', { name: 'Controls' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Controls' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Controls' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    await goSettings(user);
    await waitFor(() =>
      expect(screen.getByRole('heading', { level: 1, name: 'Settings' })).toHaveFocus(),
    );
    expect(screen.getByRole('button', { name: 'Settings' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  it('keeps the whole navigation on screen as an icon rail in a narrow window', async () => {
    const width = window.innerWidth;
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 400 });
    try {
      const user = userEvent.setup();
      render(<App />);
      await screen.findByRole('heading', { level: 1, name: 'Home' });
      expect(screen.queryByRole('button', { name: 'Toggle sidebar' })).not.toBeInTheDocument();
      expect(screen.queryByRole('dialog', { name: 'Sidebar' })).not.toBeInTheDocument();
      expect(navLabels()).toEqual(['Home', 'Controls', 'Display', 'Device', 'Settings']);
      expect(screen.getByRole('button', { name: 'Home' })).toHaveAttribute('aria-current', 'page');
      await user.click(screen.getByRole('button', { name: 'Device' }));
      expect(screen.getByRole('button', { name: 'Device' })).toHaveAttribute(
        'aria-current',
        'page',
      );
      expect(screen.getByRole('button', { name: 'Home' })).not.toHaveAttribute('aria-current');
      await user.click(screen.getByRole('button', { name: 'Settings' }));
      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 1, name: 'Settings' })).toHaveFocus(),
      );
      expect(
        document.querySelector('[data-slot="sidebar"][data-collapsible="icon"]'),
      ).toBeInTheDocument();
    } finally {
      Object.defineProperty(window, 'innerWidth', { configurable: true, value: width });
    }
  });

  it('preserves configuration navigation and moves focus when a new device loses a focused live surface', async () => {
    let publish: ((status: ConnectionStatusDto) => void) | undefined;
    vi.spyOn(ipc, 'getConnectionStatus').mockResolvedValue(connectionFixture());
    vi.spyOn(ipc, 'onConnectionStatus').mockImplementation(async (handler) => {
      publish = handler;
      return () => {};
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Display' }));
    const radio = within(await screen.findByRole('radiogroup', { name: 'Show now' })).getByRole(
      'radio',
      { name: /^Clock/ },
    );
    radio.focus();
    expect(radio).toHaveFocus();
    act(() =>
      publish?.(
        connectionFixture(['physicalInputV1', 'productIdentityV1'], { connectionGeneration: 2 }),
      ),
    );
    expect(screen.queryByRole('radiogroup', { name: 'Show now' })).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1, name: 'Display' })).toHaveFocus();
    expect(screen.getByRole('radiogroup', { name: 'Default view' })).toBeInTheDocument();
    expect(navLabels()).toEqual(['Home', 'Controls', 'Display', 'Device', 'Settings']);
    act(() => publish?.(connectionFixture([], { connection: 'disconnected', device: null })));
    expect(screen.getByRole('heading', { level: 1, name: 'Display' })).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Reactions' })).not.toHaveAttribute(
      'aria-disabled',
      'true',
    );
  });

  it('shows the plug-in guidance when no Kivori is connected', async () => {
    render(<App />);
    expect(await screen.findByText('Plug in your Kivori')).toBeInTheDocument();
  });
});

describe('Developer mode', () => {
  it('starts off and keeps technical tools away from the device and customer pages', async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole('button', { name: 'Device' }));
    expect(screen.getByTestId('device-empty')).toBeInTheDocument();
    expect(screen.queryByText('Protocol')).not.toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: 'Developer mode' })).not.toBeInTheDocument();
    await goDeveloper(user);
    expect(screen.getByRole('switch', { name: 'Developer mode' })).not.toBeChecked();
    expect(screen.queryByRole('tab', { name: 'Activity' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: 'Diagnostics' }));
    expect(await screen.findAllByRole('button', { name: 'Copy diagnostics' })).toHaveLength(1);
  });

  it('keeps tools behind its switch on Settings and persists the choice', async () => {
    const user = userEvent.setup();
    const first = render(<App />);
    await goDeveloper(user);
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(await screen.findByRole('tab', { name: 'Activity' })).toBeInTheDocument();
    expect(await screen.findByRole('tab', { name: 'Device Studio' })).toBeInTheDocument();
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
    await user.click(screen.getByRole('tab', { name: 'Social reactions' }));
    await user.click(screen.getByRole('tab', { name: 'Activity' }));
    expect(screen.getByRole('heading', { level: 2, name: 'Activity' })).toBeInTheDocument();
    first.unmount();
    render(<App />);
    await goDeveloper(user);
    expect(screen.getByRole('switch', { name: 'Developer mode' })).toBeChecked();
  });

  it('shows protocol rows and internals on Device only while developer mode is on', async () => {
    localStorage.setItem(DEV_MODE_KEY, 'true');
    vi.spyOn(ipc, 'getConnectionStatus').mockResolvedValue(connectionFixture());
    vi.spyOn(ipc, 'onConnectionStatus').mockImplementation(async (handler) => {
      handler(connectionFixture());
      return () => {};
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole('button', { name: 'Device' }));
    await user.click(screen.getByRole('tab', { name: 'Internals' }));
    expect(await screen.findByText('Protocol')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Device internals' })).toBeInTheDocument();
    act(() => setDevMode(false));
    expect(screen.queryByText('Protocol')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Device internals' })).not.toBeInTheDocument();
  });

  it('restores focus while remaining on Settings when its active tools are removed', async () => {
    localStorage.setItem(DEV_MODE_KEY, 'true');
    vi.spyOn(ipc, 'getConnectionStatus').mockResolvedValue(connectionFixture());
    vi.spyOn(ipc, 'onConnectionStatus').mockImplementation(async (handler) => {
      handler(connectionFixture());
      return () => {};
    });
    const user = userEvent.setup();
    render(<App />);
    await goDeveloper(user);
    await user.click(screen.getByRole('tab', { name: 'Social reactions' }));
    const greet = screen.getByRole('button', { name: 'Greet' });
    greet.focus();
    act(() => setDevMode(false));
    expect(screen.getByRole('heading', { level: 1, name: 'Settings' })).toHaveFocus();
    expect(screen.queryByRole('button', { name: 'Greet' })).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'Activity' })).not.toBeInTheDocument();
    act(() => setDevMode(true));
    expect(screen.getByRole('tab', { name: 'Activity' })).toHaveAttribute('aria-selected', 'true');
  });

  it('restores focus if native Studio availability removes its focused tab', async () => {
    localStorage.setItem(DEV_MODE_KEY, 'true');
    const info = {
      appVersion: '0.1.0',
      protocolVersion: { major: 0, minor: 1 },
      supportedMajors: [0],
      deviceStudioEnabled: true,
    };
    const availability = vi.spyOn(hooks, 'useAppInfo').mockReturnValue(info);
    const user = userEvent.setup();
    const page = render(<App />);
    await goDeveloper(user);
    const studio = screen.getByRole('tab', { name: 'Device Studio' });
    studio.focus();
    availability.mockReturnValue({ ...info, deviceStudioEnabled: false });
    page.rerender(<App />);
    expect(screen.queryByRole('tab', { name: 'Device Studio' })).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1, name: 'Settings' })).toHaveFocus();
  });

  it('falls back to off when storage throws', async () => {
    localStorage.setItem(DEV_MODE_KEY, 'true');
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('storage blocked');
    });
    const user = userEvent.setup();
    render(<App />);
    await goDeveloper(user);
    expect(screen.getByRole('switch', { name: 'Developer mode' })).not.toBeChecked();
  });
});

describe('Brand art', () => {
  it('uses shared brand art without assuming an unplugged device has a mascot', async () => {
    render(<App />);
    expect(screen.getByTestId('brand-mark')).toHaveAttribute(
      'src',
      expect.stringContaining('kivori-icon.svg'),
    );
    expect(await screen.findByText('Plug in your Kivori')).toBeInTheDocument();
    expect(screen.queryByTestId('empty-mascot')).not.toBeInTheDocument();
  });
});
