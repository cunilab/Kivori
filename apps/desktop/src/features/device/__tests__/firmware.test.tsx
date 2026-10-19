import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { FLASH_FAILURES } from '../../../lib/ipc/types';
import type { FirmwareStatusDto } from '../../../lib/ipc/types';
import { failureMessage, needsRestore } from '../firmware-copy';

const bridge = vi.hoisted(() => ({
  get: vi.fn(),
  flash: vi.fn(),
  restore: vi.fn(),
  updates: vi.fn(),
  offer: vi.fn(),
}));
vi.mock('../../../lib/ipc', () => ({
  getFirmwareStatus: bridge.get,
  flashFirmware: bridge.flash,
  restoreFirmware: bridge.restore,
  getUpdateStatus: bridge.updates,
  onUpdateStatus: () => Promise.resolve(() => {}),
  getFirmwareOffer: bridge.offer,
  onFirmwareOffer: () => Promise.resolve(() => {}),
}));

import { FirmwareUpdate } from '../FirmwareUpdate';
import { onUpdateDialogRequest } from '../../updates/dialog-request';

const ready: FirmwareStatusDto = {
  available: true,
  phase: 'idle',
  message: 'Ready to install.',
  imageSize: 1024,
  failure: null,
  bundledVersion: '0.1.0',
  advice: 'unknown',
  wrongProduct: null,
  restoreChoices: [],
};

beforeEach(() => {
  bridge.get.mockReset().mockResolvedValue(ready);
  bridge.flash.mockReset().mockResolvedValue(undefined);
  bridge.restore.mockReset().mockResolvedValue(undefined);
  bridge.offer.mockReset().mockResolvedValue({
    kind: 'none',
    product: null,
    from: null,
    to: null,
    reason: null,
    key: null,
  });
  bridge.updates.mockReset().mockResolvedValue({
    state: 'idle',
    reason: null,
    currentVersion: '0.1.0',
    lastChecked: null,
    autoCheck: true,
    update: null,
    progress: null,
  });
});

describe('firmware update', () => {
  it('only enables flashing when connected and firmware is available', async () => {
    const { rerender } = render(<FirmwareUpdate connected={false} />);
    await screen.findByText(ready.message);
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeDisabled();
    rerender(<FirmwareUpdate connected />);
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeEnabled();
  });

  it('keeps recovery enabled without enabling a connected flash for an unknown target', async () => {
    bridge.get.mockResolvedValue({ ...ready, bundledVersion: null, imageSize: 0 });
    const { rerender } = render(<FirmwareUpdate connected />);
    await screen.findByText(ready.message);
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeDisabled();
    rerender(<FirmwareUpdate connected={false} incompatible />);
    expect(screen.getByRole('button', { name: 'Flash firmware' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Restore with the BOOT button' })).toBeEnabled();
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
    expect(await screen.findByTestId('bundled-version')).toHaveTextContent('0.1.0');
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
        wrongProduct: { device: 'Device name', image: 'Image name' },
      });
      const { unmount } = render(<FirmwareUpdate connected />);
      expect(
        await screen.findByText(
          failureMessage(failure, { device: 'Device name', image: 'Image name' }),
        ),
      ).toBeVisible();
      expect(screen.queryByText('NATIVE-TEXT')).not.toBeInTheDocument();
      unmount();
    }
  });

  it('offers the BOOT-button restore only for failures that point at the device', async () => {
    const restoreName = 'Restore with the BOOT button';
    for (const failure of FLASH_FAILURES) {
      bridge.get.mockResolvedValue({ ...ready, phase: 'failed', failure });
      const { unmount } = render(<FirmwareUpdate connected />);
      await screen.findByText(failureMessage(failure));
      const expected = needsRestore({ phase: 'failed', failure });
      expect(needsRestore({ phase: 'failed', failure })).toBe(
        failure === 'noDownloadMode' || failure === 'unknown',
      );
      expect(screen.queryByRole('button', { name: restoreName }) !== null).toBe(expected);
      unmount();
    }
  });

  it('names both the device and the firmware when an image was refused for the wrong product', async () => {
    bridge.get.mockResolvedValue({
      ...ready,
      phase: 'failed',
      failure: 'wrongProduct',
      wrongProduct: { device: 'Kivori Lamp on RP2350', image: 'Kivori Pip on C3 SuperMini' },
    });
    render(<FirmwareUpdate connected />);
    const text = await screen.findByText(/nothing was changed/);
    expect(text).toHaveTextContent('Kivori Lamp on RP2350');
    expect(text).toHaveTextContent('Kivori Pip on C3 SuperMini');
  });

  it('lists the products to pick from and restores the one chosen', async () => {
    bridge.get.mockResolvedValue({
      ...ready,
      phase: 'failed',
      failure: 'chooseProduct',
      restoreChoices: [
        { id: 'pala', name: 'Kivori Pip', board: 'board-a', boardName: 'Board A' },
        { id: 'lamp', name: 'Kivori Lamp', board: 'board-b', boardName: 'Board B' },
      ],
    });
    const user = userEvent.setup();
    render(<FirmwareUpdate connected />);
    await user.click(await screen.findByRole('button', { name: 'Choose Kivori Lamp on Board B' }));
    expect(bridge.restore).not.toHaveBeenCalled();
    expect(
      await screen.findByText('Restore firmware for Kivori Lamp / Board B.'),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Restore now' }));
    expect(bridge.restore).toHaveBeenCalledWith('lamp', 'board-b');
    expect(
      screen.getByRole('button', { name: 'Choose Kivori Pip on Board A' }),
    ).toBeInTheDocument();
  });

  it('requires confirmation for each board and cancel never queues a restore', async () => {
    bridge.get.mockResolvedValue({
      ...ready,
      phase: 'failed',
      failure: 'chooseProduct',
      restoreChoices: [
        { id: 'fixture', name: 'Fixture', board: 'board-a', boardName: 'Board A' },
        { id: 'fixture', name: 'Fixture', board: 'board-b', boardName: 'Board B' },
      ],
    });
    const user = userEvent.setup();
    render(<FirmwareUpdate connected={false} />);
    await user.click(await screen.findByRole('button', { name: 'Choose Fixture on Board A' }));
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(bridge.restore).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Choose Fixture on Board B' }));
    expect(await screen.findByText('Restore firmware for Fixture / Board B.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Restore now' }));
    expect(bridge.restore).toHaveBeenCalledWith('fixture', 'board-b');
  });

  it('closes a choice confirmation when its pair disappears from the latest status', async () => {
    bridge.get.mockResolvedValue({
      ...ready,
      phase: 'failed',
      failure: 'chooseProduct',
      restoreChoices: [{ id: 'fixture', name: 'Fixture', board: 'board-a', boardName: 'Board A' }],
    });
    const user = userEvent.setup();
    render(<FirmwareUpdate connected={false} />);
    await user.click(await screen.findByRole('button', { name: 'Choose Fixture on Board A' }));
    bridge.get.mockResolvedValue(ready);
    await waitFor(
      () => expect(screen.queryByRole('button', { name: 'Restore now' })).not.toBeInTheDocument(),
      { timeout: 2500 },
    );
    expect(bridge.restore).not.toHaveBeenCalled();
  });

  it('keeps the pair confirmation open while unplugging for the BOOT instructions', async () => {
    bridge.get.mockResolvedValue({
      ...ready,
      phase: 'failed',
      failure: 'chooseProduct',
      restoreChoices: [{ id: 'fixture', name: 'Fixture', board: 'board-a', boardName: 'Board A' }],
    });
    const user = userEvent.setup();
    const view = render(<FirmwareUpdate connected={false} incompatible />);
    await user.click(await screen.findByRole('button', { name: 'Choose Fixture on Board A' }));
    view.rerender(<FirmwareUpdate connected={false} />);
    expect(screen.getByRole('button', { name: 'Restore now' })).toBeEnabled();
    expect(bridge.restore).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Restore now' }));
    expect(bridge.restore).toHaveBeenCalledWith('fixture', 'board-a');
  });

  it('closes a choice confirmation when a live device connects', async () => {
    bridge.get.mockResolvedValue({
      ...ready,
      phase: 'failed',
      failure: 'chooseProduct',
      restoreChoices: [{ id: 'fixture', name: 'Fixture', board: 'board-a', boardName: 'Board A' }],
    });
    const user = userEvent.setup();
    const view = render(<FirmwareUpdate connected={false} />);
    await user.click(await screen.findByRole('button', { name: 'Choose Fixture on Board A' }));
    view.rerender(<FirmwareUpdate connected />);
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: 'Restore now' })).not.toBeInTheDocument(),
    );
    expect(bridge.restore).not.toHaveBeenCalled();
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

describe('a device newer than this app', () => {
  const newer: FirmwareStatusDto = { ...ready, advice: 'deviceNewer' };

  it('says calmly to update the app, with no button while no app update is offered', async () => {
    bridge.get.mockResolvedValue(newer);
    render(<FirmwareUpdate connected />);
    expect(await screen.findByTestId('update-advice')).toHaveTextContent(
      'This Kivori has newer firmware than this app — update the Kivori app.',
    );
    expect(screen.queryByRole('button', { name: 'View update' })).toBeNull();
  });

  it('also covers an incompatible device with a newer protocol major', async () => {
    bridge.offer.mockResolvedValue({
      kind: 'updateApp',
      product: null,
      from: null,
      to: null,
      reason: null,
      key: null,
    });
    render(<FirmwareUpdate connected={false} incompatible />);
    expect(await screen.findByTestId('update-advice')).toHaveTextContent(
      'This Kivori has newer firmware than this app — update the Kivori app.',
    );
  });

  it('still lets a developer reflash it by hand, only after confirming', async () => {
    // A board flashed before the version reset speaks protocol major 1, newer than this app's 0.
    bridge.offer.mockResolvedValue({
      kind: 'updateApp',
      product: null,
      from: null,
      to: null,
      reason: null,
      key: null,
    });
    const user = userEvent.setup();
    render(<FirmwareUpdate connected={false} incompatible />);
    await screen.findByTestId('update-advice');
    expect(screen.queryByRole('alertdialog')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Flash firmware' }));
    expect(await screen.findByRole('alertdialog')).toHaveTextContent('Flash Kivori firmware?');
    expect(bridge.flash).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Flash now' }));
    expect(bridge.flash).toHaveBeenCalledTimes(1);
  });

  it('opens the app update dialog when an app update is available', async () => {
    const user = userEvent.setup();
    bridge.get.mockResolvedValue(newer);
    bridge.updates.mockResolvedValue({
      state: 'available',
      reason: null,
      currentVersion: '0.1.0',
      lastChecked: null,
      autoCheck: true,
      update: { version: '0.2.0', notes: '', date: null },
      progress: null,
    });
    const opened = vi.fn();
    const stop = onUpdateDialogRequest(opened);
    render(<FirmwareUpdate connected />);
    await user.click(await screen.findByRole('button', { name: 'View update' }));
    expect(opened).toHaveBeenCalledTimes(1);
    stop();
  });
});
