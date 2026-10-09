import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { strings } from '../../../lib/i18n/strings';
import { FLASH_FAILURES } from '../../../lib/ipc/types';
import type { FirmwareStatusDto } from '../../../lib/ipc/types';
import { needsRestore } from '../firmware-copy';

const bridge = vi.hoisted(() => ({ get: vi.fn(), flash: vi.fn(), restore: vi.fn() }));
vi.mock('../../../lib/ipc', () => ({
  getFirmwareStatus: bridge.get,
  flashFirmware: bridge.flash,
  restoreFirmware: bridge.restore,
}));

import { FirmwareUpdate } from '../FirmwareUpdate';

const ready: FirmwareStatusDto = {
  available: true,
  phase: 'idle',
  message: 'Ready to install.',
  imageSize: 1024,
  failure: null,
  bundledVersion: '1.3.0',
  advice: 'unknown',
};

beforeEach(() => {
  bridge.get.mockReset().mockResolvedValue(ready);
  bridge.flash.mockReset().mockResolvedValue(undefined);
  bridge.restore.mockReset().mockResolvedValue(undefined);
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

  it('shows the bundled version and the update advice', async () => {
    bridge.get.mockResolvedValue({ ...ready, advice: 'updateAvailable' });
    render(<FirmwareUpdate connected />);
    expect(await screen.findByTestId('bundled-version')).toHaveTextContent('1.3.0');
    expect(screen.getByTestId('update-advice')).toHaveTextContent('Update available');
  });

  it('says nothing about updates when the advice is unknown', async () => {
    render(<FirmwareUpdate connected />);
    await screen.findByText(ready.message);
    expect(screen.queryByTestId('update-advice')).not.toBeInTheDocument();
  });

  it('words every failure token in plain language, never the native message', async () => {
    for (const failure of FLASH_FAILURES) {
      bridge.get.mockResolvedValue({
        ...ready,
        phase: 'failed',
        failure,
        message: 'NATIVE-TEXT',
      });
      const { unmount } = render(<FirmwareUpdate connected />);
      expect(await screen.findByText(strings.firmwareUpdate.failures[failure])).toBeVisible();
      expect(screen.queryByText('NATIVE-TEXT')).not.toBeInTheDocument();
      unmount();
    }
  });

  it('offers the BOOT-button restore only for failures that point at the device', async () => {
    const restoreName = 'Restore with the BOOT button';
    for (const failure of FLASH_FAILURES) {
      bridge.get.mockResolvedValue({ ...ready, phase: 'failed', failure });
      const { unmount } = render(<FirmwareUpdate connected />);
      await screen.findByText(strings.firmwareUpdate.failures[failure]);
      const expected = needsRestore({ phase: 'failed', failure });
      expect(needsRestore({ phase: 'failed', failure })).toBe(
        failure === 'noDownloadMode' || failure === 'unknown',
      );
      expect(screen.queryByRole('button', { name: restoreName }) !== null).toBe(expected);
      unmount();
    }
  });

  it('shows Restore when the connection is incompatible and keeps flashing available', async () => {
    render(<FirmwareUpdate connected={false} incompatible />);
    await screen.findByText(ready.message);
    expect(screen.getByRole('button', { name: 'Restore with the BOOT button' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeEnabled();
  });

  it('does not show Restore for a healthy idle device', async () => {
    render(<FirmwareUpdate connected />);
    await screen.findByText(ready.message);
    expect(screen.queryByRole('button', { name: 'Restore with the BOOT button' })).toBeNull();
  });

  it('walks through the BOOT steps and then requests the restore', async () => {
    const user = userEvent.setup();
    render(<FirmwareUpdate connected={false} incompatible />);
    await screen.findByText(ready.message);
    await user.click(screen.getByRole('button', { name: 'Restore with the BOOT button' }));
    expect(bridge.restore).not.toHaveBeenCalled();
    await user.click(await screen.findByRole('button', { name: 'Restore now' }));
    expect(bridge.restore).toHaveBeenCalledTimes(1);
  });
});
