import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from '../App';
import { setDevMode } from '../lib/dev-mode';

const DEV_MODE_KEY = 'kivori.developerMode';
const navLabels = (): (string | null)[] =>
  within(screen.getByRole('navigation', { name: 'Main navigation' }))
    .getAllByRole('button')
    .map((b) => b.textContent);

beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe('App shell', () => {
  it('offers the product sections, plus Activity and Device Studio in developer mode', async () => {
    localStorage.setItem(DEV_MODE_KEY, 'true');
    render(<App />);
    const nav = screen.getByRole('navigation', { name: 'Main navigation' });
    await waitFor(() =>
      expect(within(nav).getByRole('button', { name: 'Device Studio' })).toBeInTheDocument(),
    );
    expect(navLabels()).toEqual([
      'Home',
      'Controls',
      'Display',
      'Activity',
      'Device',
      'Device Studio',
    ]);
  });

  it('navigates between sections and marks the current one', async () => {
    localStorage.setItem(DEV_MODE_KEY, 'true');
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Controls' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Controls' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Controls' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    await user.click(screen.getByRole('button', { name: 'Activity' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Activity' })).toBeInTheDocument();
  });

  it('shows the plug-in guidance when no Kivori is connected', async () => {
    render(<App />);
    expect(await screen.findByText('Plug in your Kivori')).toBeInTheDocument();
  });
});

describe('Developer mode', () => {
  it('is off by default: no Activity, Device Studio, technical rows or social reactions', async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
    expect(navLabels()).toEqual(['Home', 'Controls', 'Display', 'Device']);

    await user.click(screen.getByRole('button', { name: 'Display' }));
    expect(screen.getByRole('button', { name: 'Playful' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Greet' })).not.toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: 'Self-play' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Device' }));
    expect(screen.getByTestId('device-firmware')).toBeInTheDocument();
    expect(screen.queryByText('Protocol')).not.toBeInTheDocument();
    expect(screen.queryByText('Device ID (short)')).not.toBeInTheDocument();
    expect(screen.queryByText('App protocol')).not.toBeInTheDocument();
    expect(screen.getByText('App version')).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Developer mode' })).not.toBeChecked();
  });

  it('turning it on reveals the developer surfaces and persists across remounts', async () => {
    const user = userEvent.setup();
    const first = render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Device' }));
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));

    await waitFor(() =>
      expect(navLabels()).toEqual([
        'Home',
        'Controls',
        'Display',
        'Activity',
        'Device',
        'Device Studio',
      ]),
    );
    expect(screen.getByText('Protocol')).toBeInTheDocument();
    expect(screen.getByText('Device ID (short)')).toBeInTheDocument();
    expect(screen.getByText('App protocol')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Display' }));
    expect(screen.getByRole('button', { name: 'Greet' })).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Self-play' })).toBeInTheDocument();

    first.unmount();
    render(<App />);
    await waitFor(() => expect(navLabels()).toContain('Activity'));
  });

  it('turning it off while on a hidden page goes Home', async () => {
    localStorage.setItem(DEV_MODE_KEY, 'true');
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Activity' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Activity' })).toBeInTheDocument();

    act(() => setDevMode(false));
    expect(await screen.findByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
    expect(navLabels()).not.toContain('Activity');
    // Turning it back on does not resurrect the hidden page.
    act(() => setDevMode(true));
    expect(screen.getByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
  });

  it('falls back to off when storage throws', async () => {
    localStorage.setItem(DEV_MODE_KEY, 'true');
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('storage blocked');
    });
    render(<App />);
    expect(await screen.findByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
    expect(navLabels()).toEqual(['Home', 'Controls', 'Display', 'Device']);
  });
});

describe('Brand art', () => {
  it('uses the keycap icon in the sidebar and the mascot on the Buddy screen', async () => {
    render(<App />);
    expect(screen.getByTestId('brand-mark')).toHaveAttribute(
      'src',
      expect.stringContaining('kivori-icon.svg'),
    );
    expect(await screen.findByTestId('empty-mascot')).toHaveAttribute(
      'src',
      expect.stringContaining('mascot.svg'),
    );
  });
});
