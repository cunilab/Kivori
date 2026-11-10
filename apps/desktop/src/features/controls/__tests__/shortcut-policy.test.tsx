import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getGnomeShortcutPolicy } from '@/lib/ipc/gnome';
import { mockListActionCatalog } from '@/lib/ipc/mock';
import { ActionPicker } from '../ActionPicker';
import { strings } from '@/lib/i18n/strings';
import type { ActionSpec, RotateSpec } from '@/lib/ipc/types';

vi.mock('@/lib/ipc/gnome', () => ({ getGnomeShortcutPolicy: vi.fn() }));
const denied = 'This shortcut isn’t allowed. Try a different shortcut, such as Ctrl+Shift+T.';
beforeEach(() => {
  vi.mocked(getGnomeShortcutPolicy).mockImplementation(async (keys) =>
    ['Alt+F2', 'Ctrl+Shift+V'].includes(keys) ? denied : null,
  );
});
function picker(spec: ActionSpec | RotateSpec, control: 'press' | 'rotate' | 'step' = 'press') {
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <ActionPicker
      target={{ profile: null, control, name: control, spec, label: null }}
      catalog={mockListActionCatalog()}
      onClose={vi.fn()}
      onSaveSlot={save}
      onSaveRotate={save}
      onPickStep={save}
    />,
  );
  return save;
}

describe('GNOME shortcut restrictions in the action picker', () => {
  it('shows the native not-allowed reason and prevents saving a blocked chord', async () => {
    const save = picker({ kind: 'shortcut', keys: 'Alt+F2' });
    expect(await screen.findByRole('alert')).toHaveTextContent(denied);
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(save).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('Shortcut'), { target: { value: 'Ctrl+Tab' } });
    await waitFor(() => expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled());
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
  it('checks both knob directions and macro shortcut steps', async () => {
    picker({ kind: 'shortcuts', cw: 'Ctrl+Tab', ccw: 'Ctrl+Shift+V', label: 'Tabs' }, 'rotate');
    expect(await screen.findByRole('alert')).toHaveTextContent(denied);
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(getGnomeShortcutPolicy).toHaveBeenCalledWith('Ctrl+Tab');
    expect(getGnomeShortcutPolicy).toHaveBeenCalledWith('Ctrl+Shift+V');
  });
  it('fails closed if the native policy query fails', async () => {
    vi.mocked(getGnomeShortcutPolicy).mockRejectedValue(new Error('disconnected'));
    picker({ kind: 'shortcut', keys: 'Ctrl+A' }, 'step');
    expect(await screen.findByRole('alert')).toHaveTextContent(
      strings.controls.picker.shortcutPolicyUnavailable,
    );
    expect(screen.getByRole('button', { name: 'Add step' })).toBeDisabled();
  });
});
