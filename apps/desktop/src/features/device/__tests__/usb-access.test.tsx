import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SerialAccessDto } from '../../../lib/ipc/types';

const bridge = vi.hoisted(() => ({ get: vi.fn(), on: vi.fn(), fix: vi.fn() }));
vi.mock('../../../lib/ipc', () => ({
  getSerialAccess: bridge.get,
  onSerialAccess: bridge.on,
  fixSerialAccess: bridge.fix,
}));

import { UsbAccess } from '../UsbAccess';
import { strings } from '@/lib/i18n/strings';

const t = strings.device.usbAccess;

const ok: SerialAccessDto = { denied: false, canFix: true, manualCommand: null };
const denied: SerialAccessDto = { denied: true, canFix: true, manualCommand: null };
const manual: SerialAccessDto = {
  denied: true,
  canFix: false,
  manualCommand: "printf '%s\\n' 'x' | sudo tee /etc/udev/rules.d/60-kivori.rules",
};

beforeEach(() => {
  bridge.on.mockReset().mockResolvedValue(() => {});
  bridge.get.mockReset().mockResolvedValue(ok);
  bridge.fix.mockReset().mockResolvedValue(ok);
});

describe('USB access card', () => {
  it('shows nothing while the device can be opened', async () => {
    const { container } = render(<UsbAccess />);
    await waitFor(() => expect(bridge.get).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it('says what is wrong in plain words and offers one button', async () => {
    bridge.get.mockResolvedValue(denied);
    render(<UsbAccess />);
    expect(await screen.findByRole('alert')).toHaveTextContent(t.title);
    expect(screen.getByRole('button', { name: t.fix })).toBeEnabled();
    expect(screen.queryByTestId('usb-access-command')).toBeNull();
  });

  it('runs the fix on click, waits for the password and then goes away on the event', async () => {
    bridge.get.mockResolvedValue(denied);
    let emit: (access: SerialAccessDto) => void = () => {};
    bridge.on.mockImplementation((handler: typeof emit) => {
      emit = handler;
      return Promise.resolve(() => {});
    });
    let finish: (access: SerialAccessDto) => void = () => {};
    bridge.fix.mockReturnValue(new Promise((resolve) => (finish = resolve)));
    render(<UsbAccess />);
    await userEvent.click(await screen.findByRole('button', { name: t.fix }));
    expect(bridge.fix).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: t.fixing })).toBeDisabled();
    await act(async () => {
      finish(denied);
      emit(ok);
    });
    await waitFor(() => expect(screen.queryByTestId('usb-access')).toBeNull());
  });

  it('shows the native reason when the prompt is closed, and lets the user try again', async () => {
    bridge.get.mockResolvedValue(denied);
    bridge.fix.mockRejectedValueOnce("Permission wasn't granted");
    render(<UsbAccess />);
    await userEvent.click(await screen.findByRole('button', { name: t.fix }));
    expect(await screen.findByRole('status')).toHaveTextContent(t.fixFailed);
    const details = screen.getByText(t.errorDetails).closest('details');
    expect(details).not.toHaveAttribute('open');
    expect(screen.getByText("Permission wasn't granted")).not.toBeVisible();
    await userEvent.click(screen.getByText(t.errorDetails));
    expect(screen.getByText("Permission wasn't granted")).toBeVisible();
    expect(screen.getByRole('button', { name: t.fix })).toBeEnabled();
  });

  it('falls back to a generic line when the failure has no text', async () => {
    bridge.get.mockResolvedValue(denied);
    bridge.fix.mockRejectedValueOnce(undefined);
    render(<UsbAccess />);
    await userEvent.click(await screen.findByRole('button', { name: t.fix }));
    expect(await screen.findByTestId('usb-access-error')).toHaveTextContent(t.fixFailed);
  });

  it('without pkexec hides the button and shows the command to copy instead', async () => {
    bridge.get.mockResolvedValue(manual);
    const writeText = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(<UsbAccess />);
    expect(await screen.findByTestId('usb-access-command')).toHaveTextContent(
      'sudo tee /etc/udev/rules.d/60-kivori.rules',
    );
    const summary = screen.getByText(t.manualTitle);
    expect(summary.closest('details')).not.toHaveAttribute('open');
    expect(screen.getByTestId('usb-access-command')).not.toBeVisible();
    await user.click(summary);
    expect(screen.getByTestId('usb-access-command')).toBeVisible();
    expect(screen.queryByRole('button', { name: t.fix })).toBeNull();
    await user.click(screen.getByRole('button', { name: t.copy }));
    expect(writeText).toHaveBeenCalledWith(manual.manualCommand);
    expect(await screen.findByRole('button', { name: t.copied })).toBeInTheDocument();
  });

  it('appears when a later event reports the permission was refused', async () => {
    let emit: (access: SerialAccessDto) => void = () => {};
    bridge.on.mockImplementation((handler: typeof emit) => {
      emit = handler;
      return Promise.resolve(() => {});
    });
    render(<UsbAccess />);
    await waitFor(() => expect(bridge.on).toHaveBeenCalled());
    expect(screen.queryByTestId('usb-access')).toBeNull();
    act(() => emit(denied));
    expect(await screen.findByTestId('usb-access')).toBeInTheDocument();
  });
});
