import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from '../App';
import * as ipc from '../lib/ipc';
import type { CapabilityName, ConnectionStatusDto } from '../lib/ipc/types';
import { connectionFixture } from '../test/connection';
import { parseConnectionStatus } from '../lib/ipc/validate';

// Synthetic products enter through connection events only, never shipping catalog discovery.
function dial(capabilities: CapabilityName[], generation = 2): ConnectionStatusDto {
  const status = connectionFixture(capabilities, { connectionGeneration: generation });
  status.device!.identity!.product = 'Acceptance dial';
  status.device!.identity!.board = 'Test board';
  status.device!.presentation = { displayName: 'Acceptance dial', model: 'KV-T2', art: null };
  return status;
}

function connect(initial: ConnectionStatusDto): (next: ConnectionStatusDto) => void {
  const listeners = new Set<(status: ConnectionStatusDto) => void>();
  vi.spyOn(ipc, 'getConnectionStatus').mockResolvedValue(parseConnectionStatus(initial));
  vi.spyOn(ipc, 'onConnectionStatus').mockImplementation(async (handler) => {
    listeners.add(handler);
    return () => {
      listeners.delete(handler);
    };
  });
  return (next) =>
    act(() => {
      expect(listeners.size).toBeGreaterThan(0);
      const parsed = parseConnectionStatus(next);
      for (const handler of listeners) handler(parsed);
    });
}

async function navigate(user: ReturnType<typeof userEvent.setup>, name: string): Promise<void> {
  await user.click(
    within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', {
      name,
    }),
  );
  expect(screen.getByRole('heading', { level: 1, name })).toBeInTheDocument();
}

const gesture = (name: string) => within(screen.getByTestId(`gesture-${name}`));

/** Runs "Test" from a control row's overflow menu. */
async function testRow(user: ReturnType<typeof userEvent.setup>, name: string): Promise<void> {
  await user.click(gesture(name).getByRole('button', { name: /^More actions for/ }));
  await user.click(await screen.findByRole('menuitem', { name: 'Test' }));
}

beforeEach(async () => {
  localStorage.clear();
  history.replaceState({}, '', '/');
  await ipc.resetConfig();
});
afterEach(() => vi.restoreAllMocks());

describe('product acceptance through the app shell', () => {
  it('keeps Pip and minimal-product actions usable while preserving saved bindings across navigation and disconnect', async () => {
    const publish = connect(connectionFixture());
    const testRotate = vi.spyOn(ipc, 'testRotate');
    const testAction = vi.spyOn(ipc, 'testAction');
    const setMode = vi.spyOn(ipc, 'setDisplayMode');
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole('img', { name: 'Kivori Pip · KV-P1' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Change view' }));
    await user.click(
      within(await screen.findByRole('radiogroup', { name: 'Show now' })).getByRole('radio', {
        name: /^Clock/,
      }),
    );
    expect(setMode).toHaveBeenCalledWith('clock');
    await waitFor(() =>
      expect(screen.getByTestId('on-device').closest('label')).toHaveAttribute(
        'data-mode',
        'clock',
      ),
    );
    await navigate(user, 'Controls');
    await screen.findByTestId('gesture-press');
    await user.click(gesture('press').getByRole('button', { name: /^Edit/ }));
    await user.click(await screen.findByRole('radio', { name: /^Mute/ }));
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(screen.getByTestId('gesture-press')).toHaveTextContent('Custom'));
    const saved = await ipc.getConfig();
    await testRow(user, 'press');
    expect(testAction).toHaveBeenCalledWith({ kind: 'systemMute' });

    publish(dial(['productIdentityV1', 'physicalInputV1']));
    expect(screen.getByTestId('gesture-press')).toHaveTextContent('Saved only');
    expect(screen.getByTestId('gesture-rotate')).not.toHaveTextContent('Saved only');
    await testRow(user, 'rotate');
    expect(testRotate).toHaveBeenCalledWith({ kind: 'systemVolume' });
    await navigate(user, 'Home');
    expect(screen.getByRole('img', { name: 'Acceptance dial · KV-T2' })).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent(/Kivori Pip|KV-P1/);
    expect(screen.queryByRole('button', { name: 'Change view' })).not.toBeInTheDocument();
    expect(screen.queryByTestId('buddy-mascot')).not.toBeInTheDocument();
    await navigate(user, 'Device');
    expect(screen.getByTestId('device-product')).toHaveTextContent('Acceptance dial');
    expect(screen.getByTestId('device-board')).toHaveTextContent('Test board');
    expect(document.body).not.toHaveTextContent(/Kivori Pip|KV-P1|C3 SuperMini/);
    await navigate(user, 'Display');
    expect(screen.queryByRole('radiogroup', { name: 'Show now' })).not.toBeInTheDocument();
    expect(await ipc.getConfig()).toEqual(saved);

    publish(
      connectionFixture([], { connection: 'disconnected', device: null, connectionGeneration: 3 }),
    );
    expect(screen.queryByRole('radiogroup', { name: 'Show now' })).not.toBeInTheDocument();
    await user.click(
      within(screen.getByRole('radiogroup', { name: 'Default view' })).getByRole('radio', {
        name: 'Clock',
      }),
    );
    await waitFor(async () => expect((await ipc.getConfig()).display.defaultView).toBe('clock'));
    expect((await ipc.getConfig()).profiles).toEqual(saved.profiles);
    expect(setMode).toHaveBeenCalledTimes(1);
    publish(connectionFixture(undefined, { connectionGeneration: 4 }));
    expect(screen.getByRole('radiogroup', { name: 'Show now' })).toBeInTheDocument();
    await navigate(user, 'Controls');
    expect(await screen.findByTestId('gesture-press')).toHaveTextContent('Custom');
    expect(screen.getByTestId('gesture-press')).not.toHaveTextContent('Saved only');
    expect((await ipc.getConfig()).profiles).toEqual(saved.profiles);
  }, 15000);

  it('keeps mixed-capability features independent and handles missing art without borrowing Pip art', async () => {
    const mixed = dial([
      'productIdentityV1',
      'physicalInputV1',
      'contextButtonsV1',
      'deskStatusV1',
    ]);
    mixed.device!.presentation!.art = 'acceptance-missing-art';
    const pip = connectionFixture();
    pip.device!.presentation!.art = 'acceptance-pip-art';
    const publish = connect(pip);
    const pipImage =
      'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a4v8AAAAASUVORK5CYII=';
    const art = vi
      .spyOn(ipc, 'getProductArt')
      .mockImplementation(async (key) => (key === 'acceptance-pip-art' ? pipImage : null));
    const setMode = vi.spyOn(ipc, 'setDisplayMode');
    const testAction = vi.spyOn(ipc, 'testAction');
    const play = vi.spyOn(ipc, 'playMascotAction');
    const user = userEvent.setup();
    render(<App />);
    const pipFigure = await screen.findByRole('img', { name: 'Kivori Pip · KV-P1' });
    await waitFor(() => expect(pipFigure.querySelector('img')).toHaveAttribute('src', pipImage));
    publish(mixed);
    const figure = screen.getByRole('img', { name: 'Acceptance dial · KV-T2' });
    await waitFor(() => expect(art).toHaveBeenCalledWith('acceptance-missing-art'));
    expect(within(figure).getByTestId('product-art-neutral')).toBeInTheDocument();
    expect(figure.querySelector('img')).toBeNull();
    expect(document.body).not.toHaveTextContent(/Kivori Pip|KV-P1/);
    await user.click(screen.getByRole('button', { name: 'Change view' }));
    await user.click(
      within(await screen.findByRole('radiogroup', { name: 'Show now' })).getByRole('radio', {
        name: /^Media/,
      }),
    );
    expect(setMode).toHaveBeenCalledWith('media');
    await navigate(user, 'Controls');
    expect(await screen.findByTestId('gesture-press')).toHaveTextContent('Saved only');
    expect(screen.getByTestId('gesture-button1')).not.toHaveTextContent('Saved only');
    await testRow(user, 'button1');
    expect(testAction).toHaveBeenCalledWith({ kind: 'previousTrack' });
    await navigate(user, 'Developer');
    await user.click(screen.getByRole('switch', { name: 'Developer mode' }));
    await user.click(screen.getByRole('tab', { name: 'Social reactions' }));
    expect(screen.queryByRole('button', { name: 'Greet' })).not.toBeInTheDocument();
    publish(connectionFixture(undefined, { connectionGeneration: 3 }));
    await user.click(screen.getByRole('button', { name: 'Greet' }));
    expect(play).toHaveBeenCalledWith('greet');
    publish({ ...mixed, connectionGeneration: 4 });
    expect(screen.queryByRole('button', { name: 'Greet' })).not.toBeInTheDocument();
    expect(play).toHaveBeenCalledTimes(1);
  }, 15000);

  it('drops live features for pending, unknown and identity-only sessions, then reaches legacy ready state', async () => {
    const publish = connect(connectionFixture());
    const setMode = vi.spyOn(ipc, 'setDisplayMode');
    const saved = await ipc.getConfig();
    const setBinding = vi.spyOn(ipc, 'setBinding');
    const setDisplay = vi.spyOn(ipc, 'setDisplaySettings');
    const setBuddy = vi.spyOn(ipc, 'setBuddySettings');
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole('button', { name: 'Change view' });
    const pending = connectionFixture(undefined, { connectionGeneration: 2 });
    pending.device!.identity = null;
    const unknown = connectionFixture(undefined, { connectionGeneration: 3 });
    unknown.device!.identity!.known = false;
    const identityOnly = dial(['productIdentityV1'], 4);
    for (const [status, heading] of [
      [pending, 'Reading device identity…'],
      [unknown, 'This device is not supported by this app'],
      [identityOnly, 'Device firmware needs attention'],
    ] as const) {
      publish(status);
      expect(screen.getByRole('heading', { name: heading })).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Change view' })).not.toBeInTheDocument();
      expect(
        screen.getByRole('img', {
          name: status === identityOnly ? 'Acceptance dial · KV-T2' : 'Kivori device',
        }),
      ).toBeInTheDocument();
      await navigate(user, 'Display');
      expect(screen.queryByRole('radiogroup', { name: 'Show now' })).not.toBeInTheDocument();
      await navigate(user, 'Home');
      expect(await ipc.getConfig()).toEqual(saved);
    }
    const legacy = connectionFixture([], { connectionGeneration: 5 });
    legacy.device!.identity!.source = 'legacy';
    legacy.device!.firmwareVersion = '0.0.1';
    publish(legacy);
    expect(screen.getByRole('heading', { name: 'Kivori is ready' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Change view' })).not.toBeInTheDocument();
    await navigate(user, 'Device');
    expect(screen.getByTestId('device-firmware')).toHaveTextContent('0.0.1');
    expect(screen.getByTestId('device-identity-source')).toHaveTextContent(
      'Not reported (original firmware)',
    );
    expect(setMode).not.toHaveBeenCalled();
    expect(setBinding).not.toHaveBeenCalled();
    expect(setDisplay).not.toHaveBeenCalled();
    expect(setBuddy).not.toHaveBeenCalled();
    expect(await ipc.getConfig()).toEqual(saved);
  }, 15000);
});
