import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import {
  uiConnection,
  useConfig,
  useConnectionStatus,
  useDeskStatus,
  useUpdateAvailable,
} from '@/hooks/use-kivori';
import {
  mockGetStartupSettings,
  mockGrantAccessibility,
  mockResetOnboarding,
  mockSimulateInput,
} from '@/lib/ipc/mock';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { App } from '../../../App';
import { Onboarding } from '../Onboarding';
import { needsRecovery } from '../steps/PlugIn';

/** Drives the real hooks against the browser mock; `?mock=` picks the scenario. */
function Harness({ onComplete }: { onComplete: () => Promise<void> }): ReactElement {
  const connection = useConnectionStatus();
  const desk = useDeskStatus();
  const config = useConfig();
  const updateAvailable = useUpdateAvailable(connection);
  return (
    <TooltipProvider>
      <Onboarding
        connection={connection}
        desk={desk}
        config={config}
        updateAvailable={updateAvailable}
        onComplete={onComplete}
      />
    </TooltipProvider>
  );
}

const scenario = (name: string): void => history.replaceState({}, '', `/?mock=${name}&onboarding`);
const heading = (name: string): Promise<HTMLElement> =>
  screen.findByRole('heading', { level: 1, name });
const next = (user: ReturnType<typeof userEvent.setup>, label = 'Continue'): Promise<void> =>
  user.click(screen.getByRole('button', { name: label }));

beforeEach(() => {
  localStorage.clear();
  mockResetOnboarding(false);
});
afterEach(() => history.replaceState({}, '', '/'));

describe('onboarding steps follow real status', () => {
  it('stays on Plug in until Kivori is connected, unless the step is skipped', async () => {
    scenario('disconnected');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await heading('Welcome to Kivori');
    await next(user, 'Get started');
    await heading('Plug in your Kivori');
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
    expect(screen.getByText(/USB-C data cable/)).toBeInTheDocument();

    await next(user, 'Skip this step');
    await heading('Try your Kivori');
  });

  it('continues from Plug in once connected, and shows the update hint when advised', async () => {
    scenario('update');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await next(user, 'Get started');
    await heading('Plug in your Kivori');
    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());
    expect(await screen.findByTestId('onboarding-update-hint')).toBeInTheDocument();
    await next(user);
    await heading('Try your Kivori');
  });

  it('leads an incompatible device to the BOOT-button recovery', async () => {
    scenario('incompatible');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await next(user, 'Get started');
    await user.click(await screen.findByRole('button', { name: 'Restore with the BOOT button' }));
    expect(await screen.findByRole('alertdialog')).toHaveTextContent('Restore your Kivori');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
  });

  it('treats an incompatible device, or many failed reconnects, as needing recovery', () => {
    const base: ConnectionStatusDto = {
      connection: 'connecting',
      desired: 'idle',
      reported: null,
      device: null,
      incompatibleReason: null,
      retryCount: 0,
      connectionGeneration: 0,
      mascotInteraction: false,
      mascotAction: null,
      host: 'active',
    };
    expect(needsRecovery(null)).toBe(false);
    expect(needsRecovery(base)).toBe(false);
    expect(needsRecovery({ ...base, retryCount: 2 })).toBe(false);
    expect(needsRecovery({ ...base, retryCount: 3 })).toBe(true);
    expect(needsRecovery({ ...base, connection: 'incompatible' })).toBe(true);
    expect(uiConnection({ ...base, connection: 'connected' })).toBe('connected');
    expect(needsRecovery({ ...base, connection: 'connected', retryCount: 9 })).toBe(false);
  });

  it('ticks the tour off as the knob and buttons are really used', async () => {
    scenario('connected');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await next(user, 'Get started');
    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());
    await next(user);
    await heading('Try your Kivori');
    const steps = screen.getByRole('list', { name: 'Try your Kivori' });
    const done = (): string[] =>
      Array.from(steps.querySelectorAll('li[data-done="true"]')).map((li) => li.textContent ?? '');
    expect(done()).toEqual([]);
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();

    act(() => mockSimulateInput('knob'));
    await waitFor(() => expect(done()).toHaveLength(1));
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
    act(() => mockSimulateInput('press'));
    await waitFor(() => expect(done()).toHaveLength(2));
    act(() => mockSimulateInput('press'));
    await waitFor(() => expect(done()).toHaveLength(3));
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled();
    expect(screen.queryByRole('button', { name: 'Skip the tour' })).not.toBeInTheDocument();
  });

  it('lets the tour be skipped', async () => {
    scenario('disconnected');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await next(user, 'Get started');
    await next(user, 'Skip this step');
    await heading('Try your Kivori');
    expect(screen.getByText('Connect your Kivori to try these.')).toBeInTheDocument();
    await next(user, 'Skip the tour');
    await heading('Make it yours');
  });
});

describe('permission step', () => {
  it('is skipped where there is no such permission (Windows)', async () => {
    scenario('connected');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await next(user, 'Get started');
    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());
    await next(user);
    await heading('Try your Kivori');
    expect(screen.getByText('Step 3 of 5')).toBeInTheDocument();
  });

  it('on macOS waits for Accessibility and turns green once it is granted', async () => {
    scenario('mac');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await next(user, 'Get started');
    await next(user, 'Skip this step');
    await heading('Let Kivori press keys for you');
    expect(screen.getByText('Step 3 of 6')).toBeInTheDocument();
    expect(await screen.findByText('Accessibility access is not on yet.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Open System Settings' })).toBeInTheDocument();

    mockGrantAccessibility();
    expect(
      await screen.findByText('Accessibility access is on.', {}, { timeout: 4000 }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled();
  });

  it('can be skipped on macOS', async () => {
    scenario('mac');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await next(user, 'Get started');
    await next(user, 'Skip this step');
    await heading('Let Kivori press keys for you');
    await screen.findByText('Accessibility access is not on yet.');
    await next(user, 'Skip this step');
    await heading('Try your Kivori');
  });
});

describe('finishing', () => {
  it('Finish applies the launch-at-login choice (on by default) and completes', async () => {
    scenario('connected');
    const onComplete = vi.fn(() => Promise.resolve());
    const user = userEvent.setup();
    render(<Harness onComplete={onComplete} />);
    await next(user, 'Get started');
    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());
    await next(user);
    await next(user, 'Skip the tour');
    await heading('Make it yours');
    const launch = await screen.findByRole('switch', { name: 'Start with Windows' });
    expect(launch).toBeChecked();
    expect(mockGetStartupSettings().launchAtLogin).toBe(false);
    await next(user);
    await heading('You are all set');
    expect(onComplete).not.toHaveBeenCalled();
    expect(mockGetStartupSettings().launchAtLogin).toBe(false);
    await next(user, 'Finish');
    await waitFor(() => expect(onComplete).toHaveBeenCalledTimes(1));
    expect(mockGetStartupSettings().launchAtLogin).toBe(true);
  });

  it('keeps launch at login off when the switch is turned off', async () => {
    scenario('disconnected');
    const onComplete = vi.fn(() => Promise.resolve());
    const user = userEvent.setup();
    render(<Harness onComplete={onComplete} />);
    await next(user, 'Get started');
    await next(user, 'Skip this step');
    await next(user, 'Skip the tour');
    await user.click(await screen.findByRole('switch', { name: 'Start with Windows' }));
    await next(user);
    await next(user, 'Finish');
    await waitFor(() => expect(onComplete).toHaveBeenCalledTimes(1));
    expect(mockGetStartupSettings().launchAtLogin).toBe(false);
  });

  it('stays on the last step with an error when setup cannot be saved', async () => {
    scenario('disconnected');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.reject(new Error('disk'))} />);
    await next(user, 'Get started');
    await next(user, 'Skip this step');
    await next(user, 'Skip the tour');
    await next(user);
    await next(user, 'Finish');
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not finish setup');
    expect(screen.getByRole('button', { name: 'Finish' })).toBeEnabled();
  });

  it('saves the screen choice as soon as it is picked', async () => {
    scenario('disconnected');
    const user = userEvent.setup();
    render(<Harness onComplete={() => Promise.resolve()} />);
    await next(user, 'Get started');
    await next(user, 'Skip this step');
    await next(user, 'Skip the tour');
    await heading('Make it yours');
    const clock = await screen.findByRole('button', { name: 'Clock' });
    await waitFor(() => expect(clock).toBeEnabled());
    await user.click(clock);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Clock' })).toHaveAttribute('aria-pressed', 'true'),
    );
    await user.click(screen.getByRole('button', { name: 'High' }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'High' })).toHaveAttribute('aria-pressed', 'true'),
    );
  });
});

describe('in the app', () => {
  it('shows once: Skip setup completes it and a relaunch goes straight to Home', async () => {
    scenario('disconnected');
    const user = userEvent.setup();
    const first = render(<App />);
    await heading('Welcome to Kivori');
    expect(screen.queryByRole('navigation', { name: 'Main navigation' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Skip setup' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
    // Skipping applies nothing: launch at login was never touched.
    expect(mockGetStartupSettings().launchAtLogin).toBe(false);
    first.unmount();

    render(<App />);
    expect(await screen.findByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Welcome to Kivori' })).not.toBeInTheDocument();
  });

  it('runs again from the Device page', async () => {
    scenario('disconnected');
    mockResetOnboarding(true);
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Device' }));
    await user.click(await screen.findByRole('button', { name: 'Run setup again' }));
    await heading('Welcome to Kivori');
  });
});

describe('accessibility', () => {
  it('has no axe violations on the first and the permission steps', async () => {
    scenario('mac');
    const user = userEvent.setup();
    const { container } = render(<Harness onComplete={() => Promise.resolve()} />);
    await heading('Welcome to Kivori');
    const run = (): Promise<axe.AxeResults> =>
      axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect((await run()).violations).toEqual([]);
    await next(user, 'Get started');
    await heading('Plug in your Kivori');
    expect((await run()).violations).toEqual([]);
    await next(user, 'Skip this step');
    await heading('Let Kivori press keys for you');
    await screen.findByText('Accessibility access is not on yet.');
    expect((await run()).violations).toEqual([]);
  });
});
