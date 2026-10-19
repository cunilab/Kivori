import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { connectionFixture } from '@/test/connection';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { mockGetConfig } from '@/lib/ipc/mock';
import { Onboarding } from '../Onboarding';

vi.mock('@/lib/ipc', async (importOriginal) => {
  const real = await importOriginal<typeof import('@/lib/ipc')>();
  return {
    ...real,
    setDisplaySettings: vi.fn(real.setDisplaySettings),
    setBuddySettings: vi.fn(real.setBuddySettings),
  };
});
import * as ipc from '@/lib/ipc';

function setup(connection: ConnectionStatusDto | null) {
  return (
    <Onboarding
      connection={connection}
      config={mockGetConfig()}
      updateAvailable
      onComplete={() => Promise.resolve()}
    />
  );
}
const next = async (user: ReturnType<typeof userEvent.setup>, name = 'Continue') => {
  await user.click(screen.getByRole('button', { name }));
};

beforeEach(async () => {
  history.replaceState({}, '', '/');
  await ipc.resetConfig();
  vi.clearAllMocks();
});

describe('capability-aware setup', () => {
  it('keeps Plug-in gated for pending/unknown/recovery identities and accepts usable legacy sessions', async () => {
    const user = userEvent.setup();
    const pending = connectionFixture();
    pending.device!.identity = null;
    const page = render(setup(pending));
    await next(user, 'Get started');
    expect(screen.getByText(/Reading your Kivori’s identity/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
    expect(screen.queryByTestId('onboarding-update-hint')).not.toBeInTheDocument();
    const unknown = connectionFixture();
    unknown.device!.identity!.known = false;
    page.rerender(setup(unknown));
    expect(screen.getByText(/Update the app, or skip this step/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
    page.rerender(setup(connectionFixture(['productIdentityV1'])));
    expect(screen.getByText(/firmware needs attention/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Skip this step' })).toBeEnabled();
    const legacy = connectionFixture([]);
    legacy.device!.identity!.source = 'legacy';
    page.rerender(setup(legacy));
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled();
    page.rerender(setup({ ...legacy, connection: 'disconnected' }));
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Skip this step' })).toBeEnabled();
  });

  it('updates Tour guidance across products/generations without moving or blocking the step', async () => {
    const user = userEvent.setup();
    const page = render(setup(connectionFixture()));
    await next(user, 'Get started');
    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());
    await next(user);
    expect(screen.getAllByRole('listitem')).toHaveLength(3);
    const progress = screen.getByTestId('onboarding-progress').textContent;
    const dial = connectionFixture(['productIdentityV1', 'physicalInputV1'], {
      connectionGeneration: 2,
    });
    dial.device!.identity!.product = 'Test dial';
    dial.device!.presentation = { displayName: 'Test dial', model: 'KV-T2', art: null };
    page.rerender(setup(dial));
    expect(screen.getByText('Turn the rotary control')).toBeInTheDocument();
    expect(screen.queryByText('Press the push-switch')).not.toBeInTheDocument();
    expect(screen.queryByText('Press a context button')).not.toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'Test dial · KV-T2' })).toBeInTheDocument();
    expect(screen.getByTestId('onboarding-progress')).toHaveTextContent(progress!);
    page.rerender(setup(connectionFixture(['physicalInputV1', 'contextButtonsV1'])));
    expect(screen.getByText('Press a context button')).toBeInTheDocument();
    expect(screen.queryByText('Press the push-switch')).not.toBeInTheDocument();
    page.rerender(setup(connectionFixture(['deskStatusV1'])));
    expect(screen.getByText(/no controls to try in this guide/)).toBeInTheDocument();
    page.rerender(setup(connectionFixture(['productIdentityV1'])));
    expect(screen.getByText(/No device controls are available/)).toBeInTheDocument();
    page.rerender(setup(null));
    expect(screen.queryByRole('list', { name: 'Try your Kivori' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled();
    expect(screen.getByRole('heading', { level: 1, name: 'Try your Kivori' })).toBeInTheDocument();
    expect(screen.getByTestId('onboarding-progress')).toHaveTextContent(progress!);
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
  });

  it('changes Customize guidance without writes and keeps absent-feature settings editable offline', async () => {
    const user = userEvent.setup();
    const before = await ipc.getConfig();
    const page = render(setup(connectionFixture()));
    await next(user, 'Get started');
    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());
    await next(user);
    await next(user);
    expect(screen.getByRole('heading', { level: 1, name: 'Make it yours' })).toBeInTheDocument();
    expect(screen.getByText(/Hold the middle context button/)).toBeInTheDocument();
    expect(screen.queryByText('Saved only')).not.toBeInTheDocument();
    const otherProduct = connectionFixture(['physicalInputV1'], { connectionGeneration: 9 });
    otherProduct.device!.identity!.product = 'Test dial';
    otherProduct.device!.presentation = { displayName: 'Test dial', model: 'KV-T2', art: null };
    page.rerender(setup(otherProduct));
    expect(screen.queryByText(/Hold the middle context button/)).not.toBeInTheDocument();
    expect(screen.getAllByText('Saved only')).toHaveLength(2);
    page.rerender(setup(connectionFixture(['deskStatusV1', 'mascotInteraction'])));
    expect(screen.queryByText('Saved only')).not.toBeInTheDocument();
    page.rerender(setup(connectionFixture(['idleLifeV1'])));
    expect(screen.getAllByText('Saved only')).toHaveLength(1);
    const pending = connectionFixture();
    pending.device!.identity = null;
    page.rerender(setup(pending));
    expect(screen.getAllByText('Saved only')).toHaveLength(2);
    const unknown = connectionFixture();
    unknown.device!.identity!.known = false;
    page.rerender(setup(unknown));
    expect(screen.getAllByText('Saved only')).toHaveLength(2);
    page.rerender(setup(null));
    expect(screen.getByRole('button', { name: 'Clock' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'High' })).toBeEnabled();
    expect(ipc.setDisplaySettings).not.toHaveBeenCalled();
    expect(ipc.setBuddySettings).not.toHaveBeenCalled();
    expect(await ipc.getConfig()).toEqual(before);
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
    await user.click(screen.getByRole('button', { name: 'Clock' }));
    await user.click(screen.getByRole('button', { name: 'High' }));
    expect(ipc.setDisplaySettings).toHaveBeenCalledTimes(1);
    expect(ipc.setBuddySettings).toHaveBeenCalledTimes(1);
    expect((await ipc.getConfig()).profiles).toEqual(before.profiles);
  });
});
