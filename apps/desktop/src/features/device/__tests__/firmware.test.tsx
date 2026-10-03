import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FirmwareStatusDto } from '../../../lib/ipc/types';

const bridge = vi.hoisted(() => ({ get: vi.fn(), flash: vi.fn() }));
vi.mock('../../../lib/ipc', () => ({
  getFirmwareStatus: bridge.get,
  flashFirmware: bridge.flash,
}));

import { FirmwareUpdate } from '../FirmwareUpdate';

const ready: FirmwareStatusDto = {
  available: true,
  phase: 'idle',
  message: 'Ready to install.',
  imageSize: 1024,
};

beforeEach(() => {
  bridge.get.mockReset().mockResolvedValue(ready);
  bridge.flash.mockReset().mockResolvedValue(undefined);
});

describe('firmware update', () => {
  it('only enables flashing when connected and firmware is available', async () => {
    const { rerender } = render(<FirmwareUpdate connected={false} />);
    await screen.findByText(ready.message);
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeDisabled();
    rerender(<FirmwareUpdate connected />);
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeEnabled();
  });

  it('explains an unavailable firmware package without enabling the action', async () => {
    bridge.get.mockResolvedValue({
      ...ready,
      available: false,
      message: 'No firmware is bundled.',
    });
    render(<FirmwareUpdate connected />);
    await screen.findByText('No firmware is bundled.');
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeDisabled();
  });

  it('starts one native request and stays disabled while flashing', async () => {
    let resolve!: () => void;
    bridge.flash.mockImplementation(
      () =>
        new Promise<void>((done) => {
          resolve = done;
        }),
    );
    const user = userEvent.setup();
    render(<FirmwareUpdate connected />);
    await screen.findByText(ready.message);
    await user.click(screen.getByRole('button', { name: 'Flash firmware' }));
    expect(bridge.flash).not.toHaveBeenCalled();
    await user.dblClick(await screen.findByRole('button', { name: 'Flash now' }));
    expect(bridge.flash).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: 'Updating firmware…' })).toBeDisabled();
    bridge.get.mockResolvedValue({ ...ready, phase: 'flashing', message: 'Writing firmware.' });
    await act(async () => resolve());
    await screen.findByText('Writing firmware.');
    expect(screen.getByRole('button', { name: 'Updating firmware…' })).toBeDisabled();
  });

  it('asks for confirmation and does nothing when cancelled', async () => {
    const user = userEvent.setup();
    render(<FirmwareUpdate connected />);
    await screen.findByText(ready.message);
    await user.click(screen.getByRole('button', { name: 'Flash firmware' }));
    expect(await screen.findByRole('alertdialog')).toHaveTextContent('Keep USB connected');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(bridge.flash).not.toHaveBeenCalled();
  });

  it('restores progress after navigation and waits for native reconnect success', async () => {
    bridge.get.mockResolvedValue({
      ...ready,
      phase: 'reconnecting',
      message: 'Waiting for device.',
    });
    render(<FirmwareUpdate connected={false} />);
    await screen.findByText('Waiting for device.');
    expect(screen.getByRole('button', { name: 'Updating firmware…' })).toBeDisabled();
    bridge.get.mockResolvedValue({ ...ready, phase: 'succeeded', message: 'Firmware installed.' });
    await waitFor(() => expect(screen.getByText('Firmware installed.')).toBeInTheDocument(), {
      timeout: 2000,
    });
  });

  it('shows a rejected request and allows retry', async () => {
    bridge.flash.mockRejectedValue('The device disconnected.');
    const user = userEvent.setup();
    render(<FirmwareUpdate connected />);
    await screen.findByText(ready.message);
    await user.click(screen.getByRole('button', { name: 'Flash firmware' }));
    await user.click(await screen.findByRole('button', { name: 'Flash now' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('The device disconnected.');
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeEnabled();
  });

  it('handles status lookup failure', async () => {
    bridge.get.mockRejectedValue(new Error('Native runtime unavailable.'));
    render(<FirmwareUpdate connected />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Native runtime unavailable.');
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeDisabled();
  });
});
