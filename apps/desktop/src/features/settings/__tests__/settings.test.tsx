import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import { connectionFixture } from '../../../test/connection';
import type { AppInfoDto } from '../../../lib/ipc/types';

vi.mock('../../../lib/ipc', () => ({
  getStartupSettings: () => new Promise(() => {}),
  onStartupChanged: () => Promise.resolve(() => {}),
  getDiagnostics: () => new Promise(() => {}),
  getDeviceInternals: () => new Promise(() => {}),
  getUpdateStatus: () => new Promise(() => {}),
  onUpdateStatus: () => Promise.resolve(() => {}),
  getActivityLog: () => Promise.resolve([]),
  onActivityLog: () => Promise.resolve(() => {}),
  playMascotAction: vi.fn(),
}));

import { SettingsPage } from '../SettingsPage';

const appInfo: AppInfoDto = {
  appVersion: '1.2.3',
  supportedMajors: [0],
  protocolVersion: { major: 0, minor: 1 },
  deviceStudioEnabled: false,
};

const view = (onRunSetupAgain = vi.fn()) => (
  <TooltipProvider>
    <SettingsPage
      connection={connectionFixture()}
      appInfo={appInfo}
      onRunSetupAgain={onRunSetupAgain}
    />
  </TooltipProvider>
);

beforeEach(() => {
  localStorage.clear();
  document.documentElement.classList.remove('dark');
});

describe('SettingsPage', () => {
  it('holds the app settings once: appearance, updates, setup, diagnostics and developer mode', () => {
    render(view());
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
    expect(screen.getByRole('heading', { level: 1, name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('radiogroup', { name: 'Appearance' })).toBeInTheDocument();
    expect(screen.getByTestId('app-version')).toHaveTextContent('1.2.3');
    expect(screen.getByRole('button', { name: 'Run setup again' })).toBeInTheDocument();
    expect(screen.getAllByRole('heading', { name: 'Diagnostics' })).toHaveLength(1);
    expect(screen.getByRole('switch', { name: 'Developer mode' })).not.toBeChecked();
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });

  it('switches the theme between light, dark and system', async () => {
    const user = userEvent.setup();
    render(view());
    const group = screen.getByRole('radiogroup', { name: 'Appearance' });
    expect(screen.getByRole('radio', { name: 'System' })).toBeChecked();
    await user.click(screen.getByRole('radio', { name: 'Dark' }));
    expect(document.documentElement).toHaveClass('dark');
    expect(localStorage.getItem('kivori.theme')).toBe('dark');
    await user.click(screen.getByRole('radio', { name: 'Light' }));
    expect(document.documentElement).not.toHaveClass('dark');
    await user.click(screen.getByRole('radio', { name: 'System' }));
    expect(localStorage.getItem('kivori.theme')).toBeNull();
    expect(group).toBeInTheDocument();
  });

  it('runs setup again on request', async () => {
    const user = userEvent.setup();
    const again = vi.fn();
    render(view(again));
    await user.click(screen.getByRole('button', { name: 'Run setup again' }));
    expect(again).toHaveBeenCalledTimes(1);
  });

  it('shows the developer tools as a section once developer mode is on', async () => {
    const user = userEvent.setup();
    render(view());
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    expect(screen.getByRole('tab', { name: 'Activity' })).toBeInTheDocument();
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
  });

  it('has no axe violations', async () => {
    const { container } = render(view());
    const results = await axe.run(container, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
