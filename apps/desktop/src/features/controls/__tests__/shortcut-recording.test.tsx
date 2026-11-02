import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { mockListActionCatalog } from '@/lib/ipc/mock';
import { ActionPicker } from '../ActionPicker';
import type { PickerTarget } from '../ActionPicker';

function picker(target: PickerTarget) {
  const onSaveSlot = vi.fn().mockResolvedValue(undefined);
  const onSaveRotate = vi.fn().mockResolvedValue(undefined);
  const onPickStep = vi.fn();
  const onClose = vi.fn();
  render(
    <ActionPicker
      target={target}
      catalog={mockListActionCatalog()}
      onClose={onClose}
      onSaveSlot={onSaveSlot}
      onSaveRotate={onSaveRotate}
      onPickStep={onPickStep}
    />,
  );
  return { onSaveSlot, onSaveRotate, onPickStep, onClose };
}

describe('shortcut recording', () => {
  it('captures a supported combination, ignores modifiers, and saves only on confirmation', async () => {
    const user = userEvent.setup();
    const target: PickerTarget = {
      profile: null,
      control: 'press',
      name: 'Press',
      spec: { kind: 'shortcut', keys: 'Ctrl+A' },
      label: null,
    };
    const { onSaveSlot } = picker(target);
    const input = screen.getByLabelText('Shortcut');
    await user.click(screen.getByRole('button', { name: 'Record keys for Shortcut' }));
    fireEvent.keyDown(input, { key: 'Control', ctrlKey: true });
    expect(input).toHaveValue('Ctrl+A');
    fireEvent.keyDown(input, { key: 'ArrowLeft', ctrlKey: true, shiftKey: true });
    expect(input).toHaveValue('Ctrl+Shift+Left');
    expect(onSaveSlot).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Save' }));
    expect(onSaveSlot).toHaveBeenCalledWith(target, {
      action: { kind: 'shortcut', keys: 'Ctrl+Shift+Left' },
      label: null,
    });
  });

  it('rejects unsupported keys, allows cancel and retry without losing the prior value', async () => {
    const user = userEvent.setup();
    const { onClose } = picker({
      profile: null,
      control: 'press',
      name: 'Press',
      spec: { kind: 'shortcut', keys: 'Ctrl+A' },
      label: null,
    });
    const input = screen.getByLabelText('Shortcut');
    await user.click(screen.getByRole('button', { name: 'Record keys for Shortcut' }));
    fireEvent.keyDown(input, { key: ';' });
    expect(screen.getByRole('status')).toHaveTextContent('cannot be used');
    expect(input).toHaveValue('Ctrl+A');
    fireEvent.keyDown(input, { key: '+', code: 'NumpadAdd' });
    expect(screen.getByRole('status')).toHaveTextContent('cannot be used');
    expect(input).toHaveValue('Ctrl+A');
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onClose).not.toHaveBeenCalled();
    expect(input).toHaveValue('Ctrl+A');
    await user.click(screen.getByRole('button', { name: 'Record keys for Shortcut' }));
    fireEvent.keyDown(input, { key: 'Enter', metaKey: true });
    expect(input).toHaveValue('Meta+Enter');
  });

  it('records both knob directions and shortcut macro steps', async () => {
    const user = userEvent.setup();
    const target: PickerTarget = {
      profile: null,
      control: 'rotate',
      name: 'Rotate',
      spec: { kind: 'shortcuts', cw: 'Ctrl+A', ccw: 'Ctrl+B', label: 'Tabs' },
      label: null,
    };
    const { onSaveRotate } = picker(target);
    const dialog = screen.getByRole('dialog');
    for (const [label, key] of [
      ['Clockwise shortcut', 'ArrowUp'],
      ['Counter-clockwise shortcut', 'ArrowDown'],
    ] as const) {
      await user.click(within(dialog).getByRole('button', { name: `Record keys for ${label}` }));
      fireEvent.keyDown(within(dialog).getByLabelText(label), { key, altKey: true });
    }
    await user.click(within(dialog).getByRole('button', { name: 'Save' }));
    expect(onSaveRotate).toHaveBeenCalledWith(target, {
      kind: 'shortcuts',
      cw: 'Alt+Up',
      ccw: 'Alt+Down',
      label: 'Tabs',
    });
  });

  it('records a shortcut for a macro step', async () => {
    const user = userEvent.setup();
    const { onPickStep } = picker({
      profile: null,
      control: 'step',
      name: 'Step',
      spec: { kind: 'shortcut', keys: 'Ctrl+A' },
      label: null,
    });
    await user.click(screen.getByRole('button', { name: 'Record keys for Shortcut' }));
    fireEvent.keyDown(screen.getByLabelText('Shortcut'), { key: 'Tab', ctrlKey: true });
    await user.click(screen.getByRole('button', { name: 'Add step' }));
    expect(onPickStep).toHaveBeenCalledWith({
      kind: 'action',
      action: { kind: 'shortcut', keys: 'Ctrl+Tab' },
    });
  });

  it('records shifted digits using the supported base key', async () => {
    const user = userEvent.setup();
    picker({
      profile: null,
      control: 'step',
      name: 'Step',
      spec: { kind: 'shortcut', keys: 'Ctrl+A' },
      label: null,
    });
    await user.click(screen.getByRole('button', { name: 'Record keys for Shortcut' }));
    fireEvent.keyDown(screen.getByLabelText('Shortcut'), {
      key: '!',
      code: 'Digit1',
      ctrlKey: true,
      shiftKey: true,
    });
    expect(screen.getByLabelText('Shortcut')).toHaveValue('Ctrl+Shift+1');
  });
});
