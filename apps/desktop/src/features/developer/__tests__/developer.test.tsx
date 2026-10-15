import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import { connectionFixture } from '../../../test/connection';
import { mockGetDiagnostics } from '../../../lib/ipc/mock';
import { setDevMode } from '../../../lib/dev-mode';
import type { AppInfoDto } from '../../../lib/ipc/types';

const bridge = vi.hoisted(() => ({
  diagnostics: vi.fn(),
  internals: vi.fn(),
  play: vi.fn(),
  activity: vi.fn(),
  stopActivity: vi.fn(),
}));
vi.mock('../../../lib/ipc', () => ({
  getDiagnostics: bridge.diagnostics,
  getDeviceInternals: bridge.internals,
  playMascotAction: bridge.play,
  getActivityLog: bridge.activity,
  onActivityLog: vi.fn(async () => bridge.stopActivity),
}));
vi.mock('../../device-studio/DeviceStudio', () => ({
  DeviceStudio: () => <h2>Studio preview</h2>,
}));
import { DeveloperPage } from '../DeveloperPage';

const appInfo: AppInfoDto = {
  appVersion: '0.1.0',
  supportedMajors: [0],
  protocolVersion: { major: 0, minor: 1 },
  deviceStudioEnabled: true,
};
const view = (info = appInfo, connection = connectionFixture()) => (
  <TooltipProvider>
    <DeveloperPage appInfo={info} connection={connection} />
  </TooltipProvider>
);
beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
  bridge.diagnostics.mockResolvedValue(mockGetDiagnostics());
  bridge.internals.mockResolvedValue({ connected: true, supported: true, snapshot: null });
  bridge.play.mockResolvedValue(undefined);
  bridge.activity.mockResolvedValue([]);
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('Developer page', () => {
  it('offers basic diagnostics while off, without requesting internals or exposing tools', async () => {
    const user = userEvent.setup();
    const copy = vi.spyOn(navigator.clipboard, 'writeText');
    render(view());
    expect(screen.getByRole('switch', { name: 'Developer mode' })).not.toBeChecked();
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
    expect(screen.queryByText('Protocol')).not.toBeInTheDocument();
    await act(async () => {});
    await user.click(screen.getByRole('button', { name: 'Copy diagnostics' }));
    expect(copy).toHaveBeenCalled();
    expect(JSON.parse(copy.mock.calls[0]![0])).not.toHaveProperty('internals');
    expect(bridge.internals).not.toHaveBeenCalled();
    expect(bridge.activity).not.toHaveBeenCalled();
  });

  it('stops internals polling on tab changes, leaving Developer, and turning the master toggle off', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const click = async (element: HTMLElement): Promise<void> => {
      await act(async () => {
        fireEvent.click(element);
      });
    };
    const page = render(view());
    await click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(bridge.internals).toHaveBeenCalledTimes(1);
    await act(async () => vi.advanceTimersByTimeAsync(1000));
    expect(bridge.internals).toHaveBeenCalledTimes(2);
    await click(screen.getByRole('tab', { name: 'Activity' }));
    await act(async () => vi.advanceTimersByTimeAsync(3000));
    expect(bridge.internals).toHaveBeenCalledTimes(2);
    expect(bridge.activity).toHaveBeenCalledTimes(1);
    await click(screen.getByRole('tab', { name: 'Diagnostics' }));
    expect(bridge.stopActivity).toHaveBeenCalledTimes(1);
    expect(bridge.internals).toHaveBeenCalledTimes(3);
    await click(screen.getByRole('switch', { name: 'Developer mode' }));
    await act(async () => vi.advanceTimersByTimeAsync(3000));
    expect(bridge.internals).toHaveBeenCalledTimes(3);
    expect(screen.queryByRole('heading', { name: 'Device internals' })).not.toBeInTheDocument();
    await click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(bridge.internals).toHaveBeenCalledTimes(4);
    page.unmount();
    await act(async () => vi.advanceTimersByTimeAsync(3000));
    expect(bridge.internals).toHaveBeenCalledTimes(4);
  });

  it('supports keyboard navigation between tool tabs', async () => {
    const user = userEvent.setup();
    render(view());
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    screen.getByRole('tab', { name: 'Diagnostics' }).focus();
    await user.keyboard('{ArrowRight}');
    expect(screen.getByRole('tab', { name: 'Activity' })).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(screen.getByRole('heading', { level: 2, name: 'Activity' })).toBeInTheDocument();
  });

  it('plays supported reactions only here, keeping settings and preview tools separate', async () => {
    const user = userEvent.setup();
    render(view());
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    await user.click(screen.getByRole('tab', { name: 'Social reactions' }));
    for (const action of ['Greet', 'Pet', 'Tickle', 'Surprise', 'Comfort']) {
      await user.click(screen.getByRole('button', { name: action }));
    }
    expect(bridge.play.mock.calls.map(([action]) => action)).toEqual([
      'greet',
      'pet',
      'tickle',
      'surprise',
      'comfort',
    ]);
    expect(screen.queryByRole('switch', { name: 'Reactions' })).not.toBeInTheDocument();
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
  });

  it('refuses reactions without current effective support', async () => {
    setDevMode(true);
    const user = userEvent.setup();
    render(view(appInfo, connectionFixture(['productIdentityV1'])));
    await user.click(screen.getByRole('tab', { name: 'Social reactions' }));
    expect(screen.queryByRole('button', { name: 'Greet' })).not.toBeInTheDocument();
    expect(bridge.play).not.toHaveBeenCalled();
  });

  it('requires both build/native support and the toggle for Studio, and resets tools when disabled', async () => {
    const user = userEvent.setup();
    const page = render(view({ ...appInfo, deviceStudioEnabled: false }));
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(screen.queryByRole('tab', { name: 'Device Studio' })).not.toBeInTheDocument();
    page.rerender(view());
    await user.click(screen.getByRole('tab', { name: 'Device Studio' }));
    expect(screen.getByRole('heading', { name: 'Studio preview' })).toBeInTheDocument();
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(screen.queryByRole('heading', { name: 'Studio preview' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(screen.getByRole('tab', { name: 'Diagnostics' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
  });

  it('has one page title and no accessibility violations in its default and enabled states', async () => {
    const page = render(view());
    await act(async () => {});
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
    act(() => setDevMode(true));
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
  });
});
