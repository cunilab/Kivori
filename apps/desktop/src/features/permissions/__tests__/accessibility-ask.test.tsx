import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import * as ipc from '@/lib/ipc';
import { mockGrantAccessibility, mockResetOnboarding } from '@/lib/ipc/mock';
import { App } from '../../../App';

const scenario = (name: string): void => history.replaceState({}, '', `/?mock=${name}`);
const focus = (): void => {
  act(() => {
    window.dispatchEvent(new Event('focus'));
  });
};
const card = (): HTMLElement | null => screen.queryByRole('alertdialog');
const home = (): Promise<HTMLElement> => screen.findByRole('heading', { level: 1, name: 'Home' });
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 60));

// An existing install: setup finished long ago, and Kivori has never asked for Accessibility.
beforeEach(() => mockResetOnboarding(true, false));
afterEach(() => history.replaceState({}, '', '/'));

describe('the one automatic Accessibility ask (setup already finished)', () => {
  it('asks once when it is off and Kivori never asked, and records that it asked', async () => {
    scenario('mac');
    render(<App />);
    await home();
    expect(await screen.findByRole('alertdialog')).toHaveTextContent(
      'Let Kivori press keys for you?',
    );
    expect(screen.getByRole('button', { name: 'Allow' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Not now' })).toBeInTheDocument();
    await waitFor(async () => expect((await ipc.getOnboarding()).accessibilityAsked).toBe(true));
  });

  it('never comes back after Not now and a restart', async () => {
    scenario('mac');
    const user = userEvent.setup();
    const first = render(<App />);
    await home();
    await user.click(await screen.findByRole('button', { name: 'Not now' }));
    await waitFor(() => expect(card()).not.toBeInTheDocument());
    first.unmount();

    render(<App />);
    await home();
    await settle();
    expect(card()).not.toBeInTheDocument();
  });

  it('never comes back after Allow and a restart, even if macOS still says off', async () => {
    scenario('mac');
    const user = userEvent.setup();
    const first = render(<App />);
    await home();
    await user.click(await screen.findByRole('button', { name: 'Allow' }));
    expect(
      await screen.findByText(/Switch Kivori on in the Accessibility list/),
    ).toBeInTheDocument();
    first.unmount();

    render(<App />);
    await home();
    await settle();
    expect(card()).not.toBeInTheDocument();
  });

  it('shows nothing when Accessibility is already on, and does not record an ask', async () => {
    scenario('mac');
    mockGrantAccessibility();
    render(<App />);
    await home();
    await settle();
    expect(card()).not.toBeInTheDocument();
    expect((await ipc.getOnboarding()).accessibilityAsked).toBe(false);
  });

  it('shows nothing on a system without the permission (Windows)', async () => {
    scenario('connected');
    render(<App />);
    await home();
    await settle();
    expect(card()).not.toBeInTheDocument();
    expect((await ipc.getOnboarding()).accessibilityAsked).toBe(false);
  });

  it('shows nothing when Kivori already asked on this install', async () => {
    scenario('mac');
    mockResetOnboarding(true, true);
    render(<App />);
    await home();
    await settle();
    expect(card()).not.toBeInTheDocument();
  });

  it('goes away by itself when it is switched on and the window regains focus', async () => {
    scenario('mac');
    const user = userEvent.setup();
    render(<App />);
    await home();
    await user.click(await screen.findByRole('button', { name: 'Allow' }));
    await screen.findByText(/Switch Kivori on in the Accessibility list/);
    mockGrantAccessibility();
    focus();
    await waitFor(() => expect(card()).not.toBeInTheDocument());
  });

  it('is not shown after setup, which already asked in its own step', async () => {
    scenario('mac');
    mockResetOnboarding(false);
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole('heading', { level: 1, name: 'Welcome to Kivori' });
    await user.click(screen.getByRole('button', { name: 'Get started' }));
    await user.click(await screen.findByRole('button', { name: 'Skip this step' }));
    await screen.findByRole('heading', { level: 1, name: 'Let Kivori press keys for you' });
    await screen.findByText('Accessibility access is not on yet.');
    await user.click(screen.getByRole('button', { name: 'Not now' }));
    await user.click(await screen.findByRole('button', { name: 'Continue' }));
    await user.click(await screen.findByRole('button', { name: 'Continue' }));
    await user.click(await screen.findByRole('button', { name: 'Finish' }));
    await home();
    await settle();
    expect(card()).not.toBeInTheDocument();
  });
});
