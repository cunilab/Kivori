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
import { DeveloperTools } from '../DeveloperTools';

const appInfo: AppInfoDto = {
  appVersion: '0.1.0',
  supportedMajors: [0],
  protocolVersion: { major: 0, minor: 1 },
  deviceStudioEnabled: true,
};
const view = (info = appInfo, connection = connectionFixture()) => (
  <TooltipProvider>
    <DeveloperTools appInfo={info} connection={connection} />
  </TooltipProvider>
);
beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
  bridge.diagnostics.mockResolvedValue({
    ...mockGetDiagnostics(),
    connection: { ...mockGetDiagnostics().connection, state: 'connected', generation: 1 },
  });
  bridge.internals.mockResolvedValue({
    connected: true,
    supported: true,
    connectionGeneration: 1,
    snapshotAgeMs: null,
    snapshot: null,
  });
  bridge.play.mockResolvedValue(undefined);
  bridge.activity.mockResolvedValue([]);
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('Developer tools', () => {
  it('stays hidden while off, without requesting internals or activity', async () => {
    render(view());
    expect(screen.getByRole('switch', { name: 'Developer mode' })).not.toBeChecked();
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
    await act(async () => {});
    expect(bridge.internals).not.toHaveBeenCalled();
    expect(bridge.activity).not.toHaveBeenCalled();
  });

  it('stops activity streaming on tab changes, turning the toggle off, and leaving the page', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const click = async (element: HTMLElement): Promise<void> => {
      await act(async () => {
        fireEvent.click(element);
      });
    };
    const page = render(view());
    await click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(bridge.activity).toHaveBeenCalledTimes(1);
    await click(screen.getByRole('tab', { name: 'Social reactions' }));
    expect(bridge.stopActivity).toHaveBeenCalledTimes(1);
    await click(screen.getByRole('tab', { name: 'Activity' }));
    expect(bridge.activity).toHaveBeenCalledTimes(2);
    await click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(bridge.stopActivity).toHaveBeenCalledTimes(2);
    await click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(bridge.activity).toHaveBeenCalledTimes(3);
    page.unmount();
    expect(bridge.stopActivity).toHaveBeenCalledTimes(3);
    expect(bridge.internals).not.toHaveBeenCalled();
  });

  it('supports keyboard navigation between tool tabs', async () => {
    const user = userEvent.setup();
    render(view());
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    screen.getByRole('tab', { name: 'Activity' }).focus();
    await user.keyboard('{ArrowRight}');
    expect(screen.getByRole('tab', { name: 'Social reactions' })).toHaveFocus();
    await user.keyboard('{ArrowLeft}{Enter}');
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
    expect(screen.queryByRole('heading', { level: 1 })).not.toBeInTheDocument();
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
    expect(screen.getByRole('tab', { name: 'Activity' })).toHaveAttribute('aria-selected', 'true');
  });

  it('has no accessibility violations in its default and enabled states', async () => {
    const page = render(view());
    await act(async () => {});
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
    act(() => setDevMode(true));
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
  });
});
