import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FirmwareOfferDto, FirmwareStatusDto, UpdateStatusDto } from '../../../lib/ipc/types';

const bridge = vi.hoisted(() => ({
  getOffer: vi.fn(),
  onOffer: vi.fn(),
  onShowOffer: vi.fn(),
  accept: vi.fn(),
  dismiss: vi.fn(),
  firmware: vi.fn(),
  getUpdates: vi.fn(),
  onUpdates: vi.fn(),
  onShowUpdate: vi.fn(),
  install: vi.fn(),
  snooze: vi.fn(),
  skip: vi.fn(),
}));
vi.mock('../../../lib/ipc', () => ({
  getFirmwareOffer: bridge.getOffer,
  onFirmwareOffer: bridge.onOffer,
  onFirmwareOfferRequested: bridge.onShowOffer,
  acceptFirmwareOffer: bridge.accept,
  dismissFirmwareOffer: bridge.dismiss,
  getFirmwareStatus: bridge.firmware,
  getUpdateStatus: bridge.getUpdates,
  onUpdateStatus: bridge.onUpdates,
  onUpdateDialogRequested: bridge.onShowUpdate,
  installUpdate: bridge.install,
  snoozeUpdate: bridge.snooze,
  skipUpdateVersion: bridge.skip,
}));

import { FirmwareOfferDialog, resetFirmwareOfferDialog } from '../FirmwareOfferDialog';
import { resetUpdateDialog, UpdateDialog } from '../UpdateDialog';

const none: FirmwareOfferDto = {
  kind: 'none',
  product: null,
  from: null,
  to: null,
  reason: null,
  key: null,
};
const optional: FirmwareOfferDto = {
  kind: 'offerOptional',
  product: 'Kivori Pip',
  from: '0.1.0',
  to: '0.2.0',
  reason: null,
  key: '1:abcd1234:0.2.0',
};
const required: FirmwareOfferDto = {
  kind: 'required',
  product: null,
  from: null,
  to: '0.2.0',
  reason: 'protocolTooOld',
  key: '1::0.2.0',
};
const deferred: FirmwareOfferDto = { ...none, kind: 'deferredForAppUpdate' };

const idleApp: UpdateStatusDto = {
  state: 'idle',
  reason: null,
  currentVersion: '0.1.0',
  lastChecked: null,
  autoCheck: true,
  update: null,
  progress: null,
};
const appAvailable: UpdateStatusDto = {
  ...idleApp,
  state: 'available',
  update: { version: '0.2.0', notes: 'Fixes.', date: null },
};

const flashStatus = (phase: FirmwareStatusDto['phase']): FirmwareStatusDto => ({
  available: true,
  phase,
  message: '',
  imageSize: 1024,
  failure: phase === 'failed' ? 'portBusy' : null,
  bundledVersion: '0.2.0',
  advice: 'updateAvailable',
  wrongProduct: null,
  restoreChoices: [],
});

let emitOffer: (offer: FirmwareOfferDto) => void = () => {};
let emitUpdates: (status: UpdateStatusDto) => void = () => {};
let showOfferFromTray: () => void = () => {};

beforeEach(() => {
  resetFirmwareOfferDialog();
  resetUpdateDialog();
  bridge.getOffer.mockReset().mockResolvedValue(none);
  bridge.onOffer.mockReset().mockImplementation((handler: typeof emitOffer) => {
    emitOffer = handler;
    return Promise.resolve(() => {});
  });
  bridge.onShowOffer.mockReset().mockImplementation((handler: () => void) => {
    showOfferFromTray = handler;
    return Promise.resolve(() => {});
  });
  bridge.accept.mockReset().mockResolvedValue(undefined);
  bridge.dismiss.mockReset().mockResolvedValue(none);
  bridge.firmware.mockReset().mockResolvedValue(flashStatus('succeeded'));
  bridge.getUpdates.mockReset().mockResolvedValue(idleApp);
  bridge.onUpdates.mockReset().mockImplementation((handler: typeof emitUpdates) => {
    emitUpdates = handler;
    return Promise.resolve(() => {});
  });
  bridge.onShowUpdate.mockReset().mockResolvedValue(() => {});
  for (const command of [bridge.install, bridge.snooze, bridge.skip]) {
    command.mockReset().mockResolvedValue(idleApp);
  }
});

describe('firmware offer dialog', () => {
  it('asks with the product, both versions, how long it takes, and Update now / Later', async () => {
    bridge.getOffer.mockResolvedValue(optional);
    render(<FirmwareOfferDialog />);
    expect(
      await screen.findByRole('alertdialog', { name: 'Update your Kivori?' }),
    ).toBeInTheDocument();
    expect(screen.getByTestId('offer-product')).toHaveTextContent('Kivori Pip');
    expect(screen.getByTestId('offer-versions')).toHaveTextContent('Firmware 0.1.0 → 0.2.0');
    expect(screen.getByText('About 30 seconds. Keep it plugged in.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Update now' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Later' })).toBeEnabled();
  });

  it.each([none, deferred, { ...none, kind: 'updateApp' as const }])(
    'stays closed for $kind',
    async (offer) => {
      bridge.getOffer.mockResolvedValue(offer);
      render(<FirmwareOfferDialog />);
      await waitFor(() => expect(bridge.onOffer).toHaveBeenCalled());
      expect(screen.queryByRole('alertdialog')).toBeNull();
    },
  );

  it('opens by itself when the offer arrives, once per key', async () => {
    const user = userEvent.setup();
    render(<FirmwareOfferDialog />);
    await waitFor(() => expect(bridge.onOffer).toHaveBeenCalled());
    act(() => emitOffer(optional));
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    act(() => emitOffer({ ...optional }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    // The tray item reopens it.
    act(() => showOfferFromTray());
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
  });

  it('Later calls the command and closes; the next connect asks again', async () => {
    const user = userEvent.setup();
    bridge.getOffer.mockResolvedValue(optional);
    render(<FirmwareOfferDialog />);
    await user.click(await screen.findByRole('button', { name: 'Later' }));
    expect(bridge.dismiss).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    act(() => emitOffer(none));
    // A new connection is a new key.
    act(() => emitOffer({ ...optional, key: '2:abcd1234:0.2.0' }));
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
  });

  it('Update now runs the flash and follows it to success', async () => {
    const user = userEvent.setup();
    bridge.getOffer.mockResolvedValue(optional);
    bridge.firmware
      .mockResolvedValueOnce(flashStatus('flashing'))
      .mockResolvedValue(flashStatus('succeeded'));
    render(<FirmwareOfferDialog />);
    await user.click(await screen.findByRole('button', { name: 'Update now' }));
    expect(bridge.accept).toHaveBeenCalledTimes(1);
    expect(
      await screen.findByText('Updating your Kivori… Keep it plugged in.'),
    ).toBeInTheDocument();
    // The device reconnecting up to date withdraws the offer; the dialog keeps the result.
    act(() => emitOffer(none));
    expect(
      await screen.findByText('Your Kivori is up to date.', {}, { timeout: 3000 }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Close' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
  });

  it('reports a failed flash as failed', async () => {
    const user = userEvent.setup();
    bridge.getOffer.mockResolvedValue(optional);
    bridge.firmware.mockResolvedValue(flashStatus('failed'));
    render(<FirmwareOfferDialog />);
    await user.click(await screen.findByRole('button', { name: 'Update now' }));
    expect(await screen.findByText(/Another program is using your Kivori/)).toBeInTheDocument();
    expect(screen.queryByText('Your Kivori is up to date.')).toBeNull();
  });

  it('says so when the update cannot start, and stays on the question', async () => {
    const user = userEvent.setup();
    bridge.getOffer.mockResolvedValue(optional);
    bridge.accept.mockRejectedValue(new Error('A firmware update is already in progress.'));
    render(<FirmwareOfferDialog />);
    await user.click(await screen.findByRole('button', { name: 'Update now' }));
    expect(
      await screen.findByText('The update could not start. Nothing was changed.'),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Later' })).toBeEnabled();
  });

  it('a required update has Update now and Not now, which only closes it', async () => {
    const user = userEvent.setup();
    bridge.getOffer.mockResolvedValue(required);
    render(<FirmwareOfferDialog />);
    expect(
      await screen.findByRole('alertdialog', { name: 'Your Kivori needs a firmware update' }),
    ).toBeInTheDocument();
    expect(screen.getByText('Its firmware is too old to work with this app.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Later' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Not now' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    expect(bridge.dismiss).not.toHaveBeenCalled();
    act(() => showOfferFromTray());
    await user.click(await screen.findByRole('button', { name: 'Update now' }));
    expect(bridge.accept).toHaveBeenCalledTimes(1);
  });

  it('closes when the offer is withdrawn while it asks', async () => {
    bridge.getOffer.mockResolvedValue(optional);
    render(<FirmwareOfferDialog />);
    await screen.findByRole('alertdialog');
    act(() => emitOffer(none));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
  });
});

describe('app update first', () => {
  it('shows only the app update while it is offered, then the firmware offer', async () => {
    const user = userEvent.setup();
    bridge.getUpdates.mockResolvedValue(appAvailable);
    bridge.getOffer.mockResolvedValue(deferred);
    render(
      <>
        <UpdateDialog />
        <FirmwareOfferDialog />
      </>,
    );
    expect(
      await screen.findByRole('alertdialog', { name: 'Kivori 0.2.0 is ready' }),
    ).toBeInTheDocument();
    expect(screen.queryByRole('alertdialog', { name: 'Update your Kivori?' })).toBeNull();
    // "Later" on the app update: the native side then releases the firmware offer.
    await user.click(screen.getByRole('button', { name: 'Later' }));
    act(() => emitUpdates(idleApp));
    act(() => emitOffer(optional));
    expect(
      await screen.findByRole('alertdialog', { name: 'Update your Kivori?' }),
    ).toBeInTheDocument();
    expect(screen.queryByRole('alertdialog', { name: 'Kivori 0.2.0 is ready' })).toBeNull();
  });

  it('never opens the firmware offer by itself while an app update is offered', async () => {
    bridge.getUpdates.mockResolvedValue(appAvailable);
    bridge.getOffer.mockResolvedValue(optional);
    render(<FirmwareOfferDialog />);
    await waitFor(() => expect(bridge.onUpdates).toHaveBeenCalled());
    await waitFor(() => expect(bridge.getOffer).toHaveBeenCalled());
    expect(screen.queryByRole('alertdialog')).toBeNull();
    act(() => emitUpdates(idleApp));
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
  });

  it('an app update found while the offer asks takes over; the offer comes back after', async () => {
    bridge.getOffer.mockResolvedValue(optional);
    render(<FirmwareOfferDialog />);
    await screen.findByRole('alertdialog');
    act(() => emitOffer(deferred));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    act(() => emitOffer(optional));
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
  });
});
