import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StartupSettingsDto } from '../../../lib/ipc/types';

const bridge = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn() }));
vi.mock('../../../lib/ipc', () => ({
  getStartupSettings: bridge.get,
  setLaunchAtLogin: bridge.set,
}));

import { StartupCard } from '../StartupCard';

const off: StartupSettingsDto = { launchAtLogin: false, platform: 'windows' };

beforeEach(() => {
  bridge.get.mockReset().mockResolvedValue(off);
  bridge.set
    .mockReset()
    .mockImplementation((enabled: boolean) => Promise.resolve({ ...off, launchAtLogin: enabled }));
});

describe('start at login switch', () => {
  it('names the OS and reflects the OS entry', async () => {
    render(<StartupCard />);
    const toggle = await screen.findByRole('switch', { name: 'Start with Windows' });
    expect(toggle).not.toBeChecked();
  });

  it('uses macOS wording on macOS', async () => {
    bridge.get.mockResolvedValue({ launchAtLogin: true, platform: 'macos' });
    render(<StartupCard />);
    expect(await screen.findByRole('switch', { name: 'Start with macOS' })).toBeChecked();
  });

  it('turns on through the native command and shows the result it returns', async () => {
    render(<StartupCard />);
    await userEvent.click(await screen.findByRole('switch'));
    expect(bridge.set).toHaveBeenCalledWith(true);
    await waitFor(() => expect(screen.getByRole('switch')).toBeChecked());
  });

  it('keeps the old state and says so when the OS refuses', async () => {
    bridge.set.mockRejectedValue('denied');
    render(<StartupCard />);
    await userEvent.click(await screen.findByRole('switch'));
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not change');
    expect(screen.getByRole('switch')).not.toBeChecked();
  });

  it('renders nothing until the setting is known', () => {
    bridge.get.mockReturnValue(new Promise(() => {}));
    const { container } = render(<StartupCard />);
    expect(container).toBeEmptyDOMElement();
  });
});
