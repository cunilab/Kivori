import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { KeyboardShortcutsStatus } from './KeyboardShortcutsStatus';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@/lib/ipc', () => ({ isTauri: () => true }));

describe('keyboard shortcut permission', () => {
  it('does not request access until the user enables it', async () => {
    invoke.mockImplementation((command: string) =>
      Promise.resolve(command === 'keyboard_shortcuts_status' ? 'disabled' : undefined),
    );
    render(<KeyboardShortcutsStatus />);
    const button = await screen.findByRole('button', { name: 'Enable keyboard shortcuts' });
    expect(invoke).not.toHaveBeenCalledWith('enable_keyboard_shortcuts');
    await userEvent.click(button);
    expect(invoke).toHaveBeenCalledWith('enable_keyboard_shortcuts');
    expect(screen.getByText('Choose Allow in the system prompt to finish setup.')).toBeTruthy();
  });

  it('hides the setup reminder after access is enabled', async () => {
    invoke.mockResolvedValue('enabled');
    const { container, rerender } = render(<KeyboardShortcutsStatus />);
    await screen.findByText('Keyboard shortcuts are enabled.');
    rerender(<KeyboardShortcutsStatus hideWhenEnabled />);
    expect(container.textContent).toBe('');
  });
});
