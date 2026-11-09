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
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Kivori needs permission to use the USB device',
    );
    expect(screen.getByRole('button', { name: 'Allow USB access' })).toBeEnabled();
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
    await userEvent.click(await screen.findByRole('button', { name: 'Allow USB access' }));
    expect(bridge.fix).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: 'Waiting for your password…' })).toBeDisabled();
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
    await userEvent.click(await screen.findByRole('button', { name: 'Allow USB access' }));
    expect(await screen.findByTestId('usb-access-error')).toHaveTextContent(
      "Permission wasn't granted",
    );
    expect(screen.getByRole('button', { name: 'Allow USB access' })).toBeEnabled();
  });

  it('falls back to a generic line when the failure has no text', async () => {
    bridge.get.mockResolvedValue(denied);
    bridge.fix.mockRejectedValueOnce(undefined);
    render(<UsbAccess />);
    await userEvent.click(await screen.findByRole('button', { name: 'Allow USB access' }));
    expect(await screen.findByTestId('usb-access-error')).toHaveTextContent(
      'USB access was not set up.',
    );
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
    expect(screen.queryByRole('button', { name: 'Allow USB access' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Copy command' }));
    expect(writeText).toHaveBeenCalledWith(manual.manualCommand);
    expect(await screen.findByRole('button', { name: 'Copied' })).toBeInTheDocument();
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
