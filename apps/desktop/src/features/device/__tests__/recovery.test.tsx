import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { RecoveryDialog } from '../RecoveryDialog';

describe('recovery dialog', () => {
  it('lists the three BOOT steps in order', async () => {
    render(<RecoveryDialog open onOpenChange={() => {}} onRestore={() => {}} />);
    const dialog = await screen.findByRole('alertdialog');
    const steps = within(dialog).getAllByRole('listitem');
    expect(steps).toHaveLength(3);
    expect(steps[0]).toHaveTextContent('Unplug');
    expect(steps[1]).toHaveTextContent('Hold down the BOOT button');
    expect(steps[2]).toHaveTextContent('Let go of the BOOT button');
  });

  it('only calls restore when the user confirms', async () => {
    const onRestore = vi.fn();
    const user = userEvent.setup();
    render(<RecoveryDialog open onOpenChange={() => {}} onRestore={onRestore} />);
    await user.click(await screen.findByRole('button', { name: 'Cancel' }));
    expect(onRestore).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Restore now' }));
    expect(onRestore).toHaveBeenCalledTimes(1);
  });
});
