import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { connectionFixture } from '@/test/connection';
import { FLASH_FAILURES, type ConnectionStatusDto, type FirmwareStatusDto } from '@/lib/ipc/types';
import { failureMessage } from '@/features/device/firmware-copy';
import { PlugIn } from '../steps/PlugIn';

const bridge = vi.hoisted(() => ({ get: vi.fn(), restore: vi.fn() }));
vi.mock('@/lib/ipc', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/ipc')>()),
  getFirmwareStatus: bridge.get,
  restoreFirmware: bridge.restore,
}));

const idle: FirmwareStatusDto = {
  available: true,
  phase: 'idle',
  message: 'Ready to restore.',
  imageSize: 1024,
  bundledVersion: '0.1.0',
  failure: null,
  advice: 'unknown',
  wrongProduct: null,
  restoreChoices: [],
};
const incompatible = connectionFixture([], { connection: 'incompatible' });
const offline = connectionFixture([], { connection: 'disconnected', retryCount: 0 });
const step = (connection: ConnectionStatusDto = incompatible) => (
  <PlugIn connection={connection} updateAvailable={false} />
);
const restoreButton = () => screen.getByRole('button', { name: 'Restore with the BOOT button' });
const confirm = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(restoreButton());
  await user.click(await screen.findByRole('button', { name: 'Restore now' }));
};
const observe = (text: string) => screen.findByText(text, {}, { timeout: 2500 });

beforeEach(() => {
  bridge.get.mockReset().mockResolvedValue(idle);
  bridge.restore.mockReset().mockResolvedValue(undefined);
});

describe('onboarding firmware recovery', () => {
  it('observes accepted restore progress, reconnect and a later recovery attempt', async () => {
    const user = userEvent.setup();
    bridge.restore.mockImplementation(async () => {
      bridge.get.mockResolvedValue({ ...idle, phase: 'preparing', message: 'Preparing restore.' });
    });
    const view = render(step());
    await waitFor(() => expect(restoreButton()).toBeEnabled());
    await confirm(user);
    await observe('Preparing restore.');
    expect(restoreButton()).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Flash firmware' })).not.toBeInTheDocument();
    view.rerender(step(offline));
    for (const phase of ['flashing', 'reconnecting'] as const) {
      bridge.get.mockResolvedValue({ ...idle, phase, message: `Restore ${phase}.` });
      await observe(`Restore ${phase}.`);
    }
    bridge.get.mockResolvedValue({ ...idle, phase: 'succeeded', message: 'Restored.' });
    view.rerender(step(connectionFixture()));
    await waitFor(
      () =>
        expect(screen.queryByRole('heading', { name: 'Firmware update' })).not.toBeInTheDocument(),
      { timeout: 2500 },
    );
    expect(screen.getByText('Your Kivori is connected.')).toBeInTheDocument();
    view.rerender(step());
    await waitFor(() => expect(restoreButton()).toBeEnabled());
  });

  it('shows asynchronous failure and restores retry even after the USB connection disappears', async () => {
    const user = userEvent.setup();
    bridge.restore.mockImplementation(async () => {
      bridge.get.mockResolvedValue({ ...idle, phase: 'flashing', message: 'Writing restore.' });
    });
    const view = render(step());
    await waitFor(() => expect(restoreButton()).toBeEnabled());
    await confirm(user);
    await observe('Writing restore.');
    view.rerender(step(offline));
    bridge.get.mockResolvedValue({ ...idle, phase: 'failed', failure: 'portBusy' });
    await observe(failureMessage('portBusy'));
    expect(restoreButton()).toBeEnabled();
    await confirm(user);
    expect(bridge.restore).toHaveBeenCalledTimes(2);
  });

  it.each(FLASH_FAILURES.filter((failure) => failure !== 'chooseProduct'))(
    'allows retry after an accepted restore ends in %s',
    async (failure) => {
      const user = userEvent.setup();
      bridge.restore.mockImplementation(async () => {
        bridge.get.mockResolvedValue({ ...idle, phase: 'failed', failure });
      });
      const view = render(step());
      await waitFor(() => expect(restoreButton()).toBeEnabled());
      await confirm(user);
      view.rerender(step(offline));
      await observe(failureMessage(failure));
      expect(restoreButton()).toBeEnabled();
      expect(bridge.restore).toHaveBeenCalledTimes(1);
    },
  );

  it('requires product/board selection and BOOT confirmation after accepted enqueue', async () => {
    const user = userEvent.setup();
    bridge.restore.mockImplementation(async () => {
      bridge.get.mockResolvedValue({
        ...idle,
        phase: 'failed',
        failure: 'chooseProduct',
        restoreChoices: [
          { id: 'fixture', name: 'Fixture', board: 'a', boardName: 'Board A' },
          { id: 'fixture', name: 'Fixture', board: 'b', boardName: 'Board B' },
        ],
      });
    });
    const view = render(step());
    await waitFor(() => expect(restoreButton()).toBeEnabled());
    await confirm(user);
    await user.click(await screen.findByRole('button', { name: 'Choose Fixture on Board A' }));
    expect(screen.getByRole('alertdialog')).toHaveTextContent('Fixture / Board A');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(bridge.restore).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole('button', { name: 'Choose Fixture on Board B' }));
    view.rerender(step(offline));
    expect(screen.getByRole('alertdialog')).toHaveTextContent('Fixture / Board B');
    await user.click(screen.getByRole('button', { name: 'Restore now' }));
    expect(bridge.restore).toHaveBeenLastCalledWith('fixture', 'b');
    expect(bridge.restore).toHaveBeenCalledTimes(2);
  });

  it('drops a board confirmation when the native choices change', async () => {
    bridge.get.mockResolvedValue({
      ...idle,
      phase: 'failed',
      failure: 'chooseProduct',
      restoreChoices: [{ id: 'fixture', name: 'Fixture', board: 'a', boardName: 'Board A' }],
    });
    const user = userEvent.setup();
    render(step(offline));
    await user.click(await screen.findByRole('button', { name: 'Choose Fixture on Board A' }));
    bridge.get.mockResolvedValue(idle);
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument(), {
      timeout: 2500,
    });
    expect(bridge.restore).not.toHaveBeenCalled();
  });

  it('recovers native progress after leaving and reopening setup', async () => {
    bridge.get.mockResolvedValue({
      ...idle,
      phase: 'reconnecting',
      message: 'Waiting for restore.',
    });
    render(step(offline));
    await observe('Waiting for restore.');
    expect(restoreButton()).toBeDisabled();
  });

  it('restores retry after a disconnected request rejection and clears it on usable reconnect', async () => {
    let reject!: (error: Error) => void;
    bridge.restore.mockImplementation(
      () =>
        new Promise<void>((_, fail) => {
          reject = fail;
        }),
    );
    const user = userEvent.setup();
    const view = render(step());
    await waitFor(() => expect(restoreButton()).toBeEnabled());
    await user.click(restoreButton());
    await user.dblClick(await screen.findByRole('button', { name: 'Restore now' }));
    expect(bridge.restore).toHaveBeenCalledTimes(1);
    expect(restoreButton()).toBeDisabled();
    view.rerender(step(offline));
    await act(async () => reject(new Error('Restore request refused.')));
    expect(await screen.findByRole('alert')).toHaveTextContent('Restore request refused.');
    expect(restoreButton()).toBeEnabled();
    view.rerender(step(connectionFixture()));
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
    expect(screen.queryByRole('heading', { name: 'Firmware update' })).not.toBeInTheDocument();
  });

  it('keeps a usable reconnect clear when an older restore request rejects afterwards', async () => {
    let reject!: (error: Error) => void;
    bridge.restore.mockImplementation(
      () =>
        new Promise<void>((_, fail) => {
          reject = fail;
        }),
    );
    const user = userEvent.setup();
    const view = render(step());
    await waitFor(() => expect(restoreButton()).toBeEnabled());
    await confirm(user);
    view.rerender(step(connectionFixture()));
    await act(async () => reject(new Error('Late restore rejection.')));
    expect(screen.getByText('Your Kivori is connected.')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Firmware update' })).not.toBeInTheDocument();
  });

  it('disables restore while status is unavailable, then retries polling', async () => {
    bridge.get.mockRejectedValue(new Error('Firmware status unavailable.'));
    render(step());
    expect(await screen.findByRole('alert')).toHaveTextContent('Firmware status unavailable.');
    expect(restoreButton()).toBeDisabled();
    bridge.get.mockResolvedValue(idle);
    await waitFor(() => expect(restoreButton()).toBeEnabled(), { timeout: 2500 });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('keeps ordinary setup quiet when no recovery is needed', async () => {
    const view = render(step(offline));
    await waitFor(() => expect(bridge.get).toHaveBeenCalled());
    expect(screen.queryByRole('heading', { name: 'Firmware update' })).not.toBeInTheDocument();
    view.rerender(step(connectionFixture()));
    expect(screen.getByText('Your Kivori is connected.')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Restore with the BOOT button' }),
    ).not.toBeInTheDocument();
  });
});
