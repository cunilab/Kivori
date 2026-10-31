import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { mockGrantAccessibility, mockResetOnboarding } from '@/lib/ipc/mock';
import { AccessibilityStatus } from '../AccessibilityStatus';

const scenario = (name: string): void => history.replaceState({}, '', `/?mock=${name}`);
const focus = (): void => {
  act(() => {
    window.dispatchEvent(new Event('focus'));
  });
};

beforeEach(() => mockResetOnboarding(true));
afterEach(() => history.replaceState({}, '', '/'));

describe('Settings Accessibility row', () => {
  it('is absent where there is no such permission (Windows)', async () => {
    scenario('connected');
    const { container } = render(<AccessibilityStatus />);
    await new Promise((resolve) => setTimeout(resolve, 40));
    expect(container).toBeEmptyDOMElement();
  });

  it('says Not allowed with the ask, and the recovery once the user came back and it still reads off', async () => {
    scenario('mac');
    const user = userEvent.setup();
    render(<AccessibilityStatus />);
    expect(await screen.findByText('Not allowed')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Allow access' })).toBeInTheDocument();
    expect(screen.queryByText('Still showing as off?')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Open System Settings' }));
    focus();
    expect(await screen.findByText('Still showing as off?')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Reset and ask again' }));
    await waitFor(() =>
      expect(screen.queryByText('Still showing as off?')).not.toBeInTheDocument(),
    );
    expect(screen.getByText('Not allowed')).toBeInTheDocument();
  });

  it('says Allowed, without any button, once macOS reports it on', async () => {
    scenario('mac');
    render(<AccessibilityStatus />);
    await screen.findByText('Not allowed');
    mockGrantAccessibility();
    focus();
    expect(await screen.findByText('Allowed')).toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
    mockGrantAccessibility(false);
    focus();
    expect(await screen.findByText('Not allowed')).toBeInTheDocument();
  });

  it('has no axe violations in either state', async () => {
    scenario('mac');
    const { container } = render(<AccessibilityStatus />);
    await screen.findByText('Not allowed');
    const run = (): Promise<axe.AxeResults> =>
      axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect((await run()).violations).toEqual([]);
    mockGrantAccessibility();
    focus();
    await screen.findByText('Allowed');
    expect((await run()).violations).toEqual([]);
  });
});
