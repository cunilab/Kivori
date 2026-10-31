import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { mockGrantAccessibility, mockResetOnboarding } from '@/lib/ipc/mock';
import { AccessibilityHelp } from '../AccessibilityHelp';

const scenario = (name: string): void => history.replaceState({}, '', `/?mock=${name}`);
const focus = (): void => {
  act(() => {
    window.dispatchEvent(new Event('focus'));
  });
};
const view = (): ReturnType<typeof render> =>
  render(<AccessibilityHelp title="Needs Accessibility" body="Because shortcuts press keys." />);

beforeEach(() => mockResetOnboarding(false));
afterEach(() => history.replaceState({}, '', '/'));

describe('AccessibilityHelp', () => {
  it('shows nothing where there is no such permission', async () => {
    scenario('connected');
    view();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.queryByText('Needs Accessibility')).not.toBeInTheDocument();
  });

  it('explains the need and offers to ask when macOS says it is off', async () => {
    scenario('mac');
    view();
    expect(await screen.findByText('Needs Accessibility')).toBeInTheDocument();
    expect(screen.getByText('Because shortcuts press keys.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Allow access' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Open System Settings' })).toBeInTheDocument();
  });

  it('is quiet about stale approvals until the user asked and came back', async () => {
    scenario('mac');
    const user = userEvent.setup();
    view();
    await screen.findByText('Needs Accessibility');
    focus();
    expect(screen.queryByText('Still showing as off?')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Open System Settings' }));
    focus();
    expect(await screen.findByText('Still showing as off?')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Reset and ask again' }));
    await waitFor(() =>
      expect(screen.queryByText('Still showing as off?')).not.toBeInTheDocument(),
    );
    expect(screen.getByText('Needs Accessibility')).toBeInTheDocument();
  });

  it('goes away by itself when the window regains focus after it was switched on', async () => {
    scenario('mac');
    const user = userEvent.setup();
    view();
    await screen.findByText('Needs Accessibility');
    await user.click(screen.getByRole('button', { name: 'Open System Settings' }));
    mockGrantAccessibility();
    focus();
    await waitFor(() => expect(screen.queryByText('Needs Accessibility')).not.toBeInTheDocument());
    expect(screen.queryByText('Still showing as off?')).not.toBeInTheDocument();
  });
});
