import { connectionFixture } from '../../../test/connection';
import type { ConnectionStatusDto } from '../../../lib/ipc/types';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'sonner';
import { Toaster } from '@kivori/ui/components/sonner';
import { useConfig, useDeskStatus } from '../../../hooks/use-kivori';
import type { DeskActionDto, ConfigDto } from '../../../lib/ipc/types';

// The browser mock behind the real IPC wrappers; `testAction` and `testRotate` are spied on.
vi.mock('../../../lib/ipc', async (importOriginal) => {
  const real = await importOriginal<typeof import('../../../lib/ipc')>();
  return { ...real, testAction: vi.fn(real.testAction), testRotate: vi.fn(real.testRotate) };
});

import * as ipc from '../../../lib/ipc';
import { resetPageTabs } from '../../../components/kivori/page-tabs';
import { ControlsPage } from '../ControlsPage';
import { macroTemplates } from '../macro-templates';

function Harness({ connection = null }: { connection?: ConnectionStatusDto | null }): ReactElement {
  const desk = useDeskStatus();
  const config = useConfig();
  return (
    <>
      <ControlsPage desk={desk} config={config} connection={connection} />
      <Toaster />
    </>
  );
}

/** Opens a page tab (Controls, Macros or Reset & help). */
async function openTab(user: ReturnType<typeof userEvent.setup>, name: string): Promise<void> {
  await user.click(await screen.findByRole('tab', { name }));
}

async function selectProfile(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
): Promise<void> {
  const nav = await screen.findByRole('navigation', { name: 'Profile' });
  await user.click(
    within(nav).getByRole('button', {
      name: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`),
    }),
  );
}

const row = (key: string): HTMLElement => screen.getByTestId(`gesture-${key}`);

/** The open control tab. Inactive panels stay mounted, so list queries must start here. */
function selectedPanel(): HTMLElement {
  const tabs = screen.getByRole('tablist', { name: 'Choose a control' });
  const tab = within(tabs).getByRole('tab', { selected: true });
  const panel = document.getElementById(tab.getAttribute('aria-controls') ?? '');
  if (!panel) throw new Error('missing tab panel');
  return panel;
}

/** Runs Test from its visible button, or Reset from the row's overflow menu. */
async function rowAction(
  user: ReturnType<typeof userEvent.setup>,
  key: string,
  item: string,
): Promise<void> {
  if (item === 'Test') {
    await user.click(within(row(key)).getByRole('button', { name: /^Test / }));
    return;
  }
  await user.click(within(row(key)).getByRole('button', { name: /^More actions for/ }));
  await user.click(await screen.findByRole('menuitem', { name: item }));
}

/**
 * Like `rowAction`, for tests that fake the interval timers: the menu opens on real timers (the
 * queries poll), then the click that starts the countdown runs on fake ones.
 */
async function rowActionFake(
  user: ReturnType<typeof userEvent.setup>,
  key: string,
  item: string,
): Promise<void> {
  vi.useRealTimers();
  if (item === 'Test') {
    const button = within(row(key)).getByRole('button', { name: /^Test / });
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    await user.click(button);
    return;
  }
  await user.click(within(row(key)).getByRole('button', { name: /^More actions for/ }));
  const entry = await screen.findByRole('menuitem', { name: item });
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
  await user.click(entry);
}

async function openPressEditor(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  await screen.findByTestId('gesture-press');
  await user.click(within(row('press')).getByRole('button', { name: /^Edit/ }));
  await screen.findByRole('dialog');
}

beforeEach(async () => {
  window.history.pushState({}, '', '/');
  resetPageTabs();
  await ipc.resetConfig();
  vi.mocked(ipc.testAction).mockClear();
  vi.mocked(ipc.testRotate).mockClear();
});

afterEach(() => {
  vi.useRealTimers();
  window.history.pushState({}, '', '/');
});

describe('ControlsPage', () => {
  it('shows Hold + rotate as the device’s fixed profile picker, with nothing to bind', async () => {
    render(<Harness connection={connectionFixture()} />);
    await screen.findByTestId('gesture-heldRotate');
    expect(row('heldRotate')).toHaveTextContent('Fixed: picks a profile on the device');
    expect(
      within(row('heldRotate')).queryByRole('button', { name: 'Edit Hold + rotate' }),
    ).not.toBeInTheDocument();
    expect(row('rotate')).toHaveTextContent('System volume');
    expect(row('hold')).toHaveTextContent('Fixed: Hold + turn switches profile.');
    expect(within(row('hold')).queryByRole('button')).not.toBeInTheDocument();
  });

  it('explains that firmware without the profile picker cannot use Hold + rotate', async () => {
    render(
      <Harness
        connection={connectionFixture(['physicalInputV1', 'buttonInputV1', 'productIdentityV1'])}
      />,
    );
    await screen.findByTestId('gesture-heldRotate');
    expect(within(row('heldRotate')).getByText('Saved only')).toBeVisible();
    expect(row('heldRotate')).toHaveTextContent('Update the device firmware');
    expect(within(row('rotate')).queryByText('Saved only')).not.toBeInTheDocument();
  });

  it('selects a physical button for editing without running its action', async () => {
    const user = userEvent.setup();
    render(<Harness connection={connectionFixture()} />);
    await screen.findByTestId('gesture-press');
    const left = screen.getByRole('button', { name: 'Select Left button' });
    expect(left).toHaveAttribute('aria-pressed', 'false');
    await user.click(left);
    expect(left).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('tab', { name: 'Left button', selected: true })).toBeVisible();
    expect(row('button1')).toHaveTextContent('Previous track');
    expect(ipc.testAction).not.toHaveBeenCalled();
    expect(ipc.testRotate).not.toHaveBeenCalled();
    await user.click(within(row('button1')).getByRole('button', { name: 'Edit Left button' }));
    expect(await screen.findByRole('dialog', { name: 'Edit Left button' })).toBeVisible();
  });

  it('keeps a text-free mascot screen and reacts to selection, hover and keyboard focus', async () => {
    const user = userEvent.setup();
    render(<Harness connection={connectionFixture()} />);
    await screen.findByTestId('gesture-press');
    const deviceScreen = screen.getByTestId('device-screen');
    expect(within(deviceScreen).getByTestId('device-screen-mascot')).toBeInTheDocument();
    expect(deviceScreen.textContent).toBe('');
    expect(deviceScreen).toHaveAttribute('data-reaction', 'knob');

    const left = screen.getByRole('button', { name: 'Select Left button' });
    await user.hover(left);
    expect(deviceScreen).toHaveAttribute('data-reaction', 'button1');
    expect(screen.getByRole('button', { name: 'Select Knob' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await user.unhover(left);
    expect(deviceScreen).toHaveAttribute('data-reaction', 'knob');
    await user.click(left);
    await user.unhover(left);
    expect(deviceScreen).toHaveAttribute('data-reaction', 'button1');
    expect(left).toHaveAttribute('aria-pressed', 'true');
    await user.tab();
    expect(deviceScreen).toHaveAttribute('data-reaction', 'button2');
    expect(deviceScreen.textContent).toBe('');
    expect(ipc.testAction).not.toHaveBeenCalled();
    expect(ipc.testRotate).not.toHaveBeenCalled();
  });

  it('retains edited bindings and host tests when switching to a device without their controls', async () => {
    const user = userEvent.setup();
    const page = render(<Harness connection={connectionFixture()} />);
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /Keyboard shortcut/ }));
    await user.type(screen.getByLabelText('Shortcut'), 'Ctrl+Alt+K');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(row('press')).toHaveTextContent('Custom'));
    const saved = await ipc.getConfig();
    page.rerender(
      <Harness connection={connectionFixture(['physicalInputV1', 'productIdentityV1'])} />,
    );
    expect(row('press')).toHaveTextContent('Saved only');
    expect(row('rotate')).not.toHaveTextContent('Saved only');
    expect(within(row('press')).getByRole('button', { name: 'Test Press' })).toBeEnabled();
    page.rerender(<Harness connection={null} />);
    expect(row('press')).toHaveTextContent('Keyboard shortcut: Ctrl+Alt+K');
    expect(await ipc.getConfig()).toEqual(saved);
  });

  it('opens on the active profile and shows each control with its action', async () => {
    render(<Harness />);
    expect(
      within(await screen.findByRole('navigation', { name: 'Profile' })).getByRole('button', {
        pressed: true,
      }),
    ).toHaveTextContent('General');
    expect(row('rotate')).toHaveTextContent('System volume');
    expect(row('press')).toHaveTextContent('Mute');
    expect(row('press')).toHaveTextContent('Mute');
    await userEvent.setup().click(screen.getByRole('tab', { name: 'Left button' }));
    expect(row('button1')).toHaveTextContent('Previous track');
    expect(row('button1Hold')).toHaveTextContent('Add action');
    // The disclaimers live under Reset & help, not above the controls.
    expect(screen.queryByTestId('controls-notes')).not.toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole('tab', { name: 'Reset & help' }));
    expect(screen.getByText(/Middle button hold mutes every microphone/)).toBeVisible();
    expect(screen.getByTestId('mic-scope-note')).toBeVisible();
    expect(screen.queryByText('Custom')).not.toBeInTheDocument();
  });

  it('groups editable rows under each control and keeps Reset in the overflow menu', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    const knob = screen.getByRole('list', { name: 'Knob' });
    expect(within(knob).getAllByRole('listitem')).toHaveLength(2);
    expect(within(knob).queryByTestId('gesture-heldRotate')).not.toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: 'Left button' }));
    expect(
      within(screen.getByRole('list', { name: 'Left button' })).getAllByRole('listitem'),
    ).toHaveLength(2);
    await user.click(screen.getByRole('tab', { name: 'Knob' }));
    expect(within(row('press')).getByRole('button', { name: 'Test Press' })).toBeVisible();
    expect(within(row('press')).queryByRole('button', { name: 'Reset' })).toBeNull();
    await user.click(within(row('press')).getByRole('button', { name: 'More actions for Press' }));
    const reset = await screen.findByRole('menuitem', { name: 'Reset' });
    expect(screen.queryByRole('menuitem', { name: 'Test' })).not.toBeInTheDocument();
    expect(reset).toHaveAttribute('aria-disabled', 'true');
  });

  it('opens the editor when the row text is clicked', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    await user.click(within(row('press')).getByText('Mute'));
    expect(await screen.findByRole('dialog')).toBeVisible();
  });

  it('shows a friendly status mark instead of a Confirmed / Result unknown badge', async () => {
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    const mark = within(row('press')).getByLabelText('Kivori can confirm this worked');
    expect(mark).toHaveAttribute('data-verification', 'confirmed');
    await userEvent.setup().click(screen.getByRole('tab', { name: 'Left button' }));
    expect(
      within(row('button1')).getByLabelText('Kivori can’t tell whether this worked'),
    ).toHaveAttribute('data-verification', 'unverified');
    expect(within(row('press')).queryByText('Result unknown')).toBeNull();
  });

  it('marks an edited Press as Custom and Reset brings the built-in back', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /Keyboard shortcut/ }));
    await user.type(screen.getByLabelText('Shortcut'), 'Ctrl+Alt+K');
    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(row('press')).toHaveTextContent('Custom'));
    expect(row('press')).toHaveTextContent('Keyboard shortcut: Ctrl+Alt+K');

    await rowAction(user, 'press', 'Reset');
    await waitFor(() => expect(row('press')).not.toHaveTextContent('Custom'));
    expect(row('press')).toHaveTextContent('Mute');
  });

  it('shows a native rejection in the sheet and leaves the row unchanged', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /Keyboard shortcut/ }));
    await user.type(screen.getByLabelText('Shortcut'), 'Ctrl+');
    await user.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('not a valid shortcut');
    expect(row('press')).toHaveTextContent('Mute');
    expect(row('press')).not.toHaveTextContent('Custom');
  });

  it('checks the label length and warns when it may fade', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /Keyboard shortcut/ }));
    await user.type(screen.getByLabelText('Shortcut'), 'F5');
    await user.type(screen.getByLabelText('Label on the device'), 'Reload everything');
    expect(screen.getByText(/may fade on the device/)).toBeVisible();
    await user.clear(screen.getByLabelText('Label on the device'));
    await user.type(screen.getByLabelText('Label on the device'), 'x'.repeat(33));
    expect(screen.getByText('A label is at most 32 characters.')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  });

  it('offers only knob actions for Rotate, and App volume is disabled on macOS', async () => {
    window.history.pushState({}, '', '/?mock=mac');
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-rotate');
    await user.click(within(row('rotate')).getByRole('button', { name: /^Edit/ }));
    await screen.findByRole('dialog');
    expect(screen.getByRole('radio', { name: /System volume/ })).toBeEnabled();
    expect(screen.getByRole('radio', { name: /Two shortcuts/ })).toBeEnabled();
    expect(screen.queryByRole('radio', { name: /Play \/ Pause/ })).not.toBeInTheDocument();
    const appVolume = screen.getByRole('radio', { name: /App volume/ });
    expect(appVolume).toHaveAttribute('aria-disabled', 'true');
    expect(screen.getByText("Per-app volume isn't available on macOS")).toBeVisible();
  });

  it('runs a non-keyboard Test at once and shows its result', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    await rowAction(user, 'press', 'Test');
    expect(ipc.testAction).toHaveBeenCalledWith({ kind: 'systemMute' });
    expect(
      await within(row('press')).findByText('Confirmed', { selector: '[data-result]' }),
    ).toBeVisible();
  });

  it('keeps pending test feedback independent of old and unrelated desk actions', async () => {
    const user = userEvent.setup();
    await ipc.testRotate({ kind: 'systemVolume' });
    let finish!: (outcome: DeskActionDto) => void;
    vi.mocked(ipc.testAction).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    await rowAction(user, 'press', 'Test');
    expect(row('press').querySelector('[data-result]')).toHaveAttribute(
      'data-result',
      'processing',
    );
    await act(async () => {
      await ipc.testRotate({ kind: 'systemVolume' });
    });
    expect(row('press').querySelector('[data-result]')).toHaveAttribute(
      'data-result',
      'processing',
    );
    await act(async () =>
      finish({ action: 'mute', result: 'unverified', permissionRequired: false }),
    );
    expect(row('press').querySelector('[data-result]')).toHaveAttribute(
      'data-result',
      'unverified',
    );
    await act(async () => {
      await ipc.testRotate({ kind: 'systemVolume' });
    });
    expect(row('press').querySelector('[data-result]')).toHaveAttribute(
      'data-result',
      'unverified',
    );
  });

  it('replaces a prior success when a later test rejects', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    await rowAction(user, 'press', 'Test');
    await waitFor(() =>
      expect(row('press').querySelector('[data-result]')).toHaveAttribute(
        'data-result',
        'stateConfirmed',
      ),
    );
    vi.mocked(ipc.testAction).mockRejectedValueOnce(new Error('failed test'));
    await rowAction(user, 'press', 'Test');
    await waitFor(() =>
      expect(row('press').querySelector('[data-result]')).toHaveAttribute('data-result', 'error'),
    );
    expect(await screen.findByText('failed test')).toBeVisible();
  });

  it('ignores superseded replies and clears old feedback when a countdown starts', async () => {
    const user = userEvent.setup();
    let finish!: (outcome: DeskActionDto) => void;
    vi.mocked(ipc.testAction).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    await rowAction(user, 'press', 'Test');
    await rowAction(user, 'rotate', 'Test');
    await waitFor(() =>
      expect(row('rotate').querySelector('[data-result]')).toHaveAttribute(
        'data-result',
        'stateConfirmed',
      ),
    );
    await act(async () => finish({ action: 'mute', result: 'error', permissionRequired: false }));
    expect(row('press').querySelector('[data-result]')).toBeNull();
    expect(row('rotate').querySelector('[data-result]')).toHaveAttribute(
      'data-result',
      'stateConfirmed',
    );
    await ipc.setBinding('general', 'press', { action: { kind: 'shortcut', keys: 'Ctrl+K' } });
    await waitFor(() => expect(row('press')).toHaveTextContent('Ctrl+K'));
    await rowActionFake(user, 'press', 'Test');
    expect(row('rotate').querySelector('[data-result]')).toBeNull();
    expect(within(row('press')).getByRole('status')).toHaveTextContent('Running in 3');
    await user.click(within(row('press')).getByRole('button', { name: 'Cancel' }));
    act(() => vi.advanceTimersByTime(5000));
    vi.useRealTimers();
  });

  it('invalidates a pending reply when the saved binding changes', async () => {
    const user = userEvent.setup();
    let finish!: (outcome: DeskActionDto) => void;
    vi.mocked(ipc.testAction).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    await rowAction(user, 'press', 'Test');
    await act(async () => {
      await ipc.setBinding('general', 'press', { action: { kind: 'playPause' } });
    });
    await act(async () =>
      finish({ action: 'mute', result: 'stateConfirmed', permissionRequired: false }),
    );
    expect(row('press').querySelector('[data-result]')).toBeNull();
  });

  it.each(['profile', 'unmount'])('ignores a late rejection after %s changes', async (change) => {
    const user = userEvent.setup();
    let reject!: (error: Error) => void;
    vi.mocked(ipc.testAction).mockImplementationOnce(
      () =>
        new Promise((_, fail) => {
          reject = fail;
        }),
    );
    const page = render(<Harness />);
    await screen.findByTestId('gesture-press');
    await rowAction(user, 'press', 'Test');
    if (change === 'unmount') page.unmount();
    else await selectProfile(user, 'Browser');
    const errorToast = vi.spyOn(toast, 'error');
    await act(async () => reject(new Error('obsolete test')));
    expect(errorToast).not.toHaveBeenCalled();
    errorToast.mockRestore();
    if (change === 'profile') expect(row('press').querySelector('[data-result]')).toBeNull();
  });

  it('tests a volume knob at once, as a step up and back, and shows the result', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-rotate');
    await openTab(user, 'Reset & help');
    expect(screen.getByTestId('controls-notes')).toHaveTextContent('up and back down');
    await openTab(user, 'Controls');
    await rowAction(user, 'rotate', 'Test');
    expect(ipc.testRotate).toHaveBeenCalledWith({ kind: 'systemVolume' });
    expect(ipc.testAction).not.toHaveBeenCalled();
    expect(
      await within(row('rotate')).findByText('Confirmed', { selector: '[data-result]' }),
    ).toBeVisible();
  });

  it.each([
    ['Browser', 'Use tab switching', 'Tabs', 'Ctrl+Tab', 'Ctrl+Shift+Tab'],
    ['Premiere Pro', 'Use frame stepping', 'Frame', 'Right', 'Left'],
    ['Claude Code', 'Use prompt history', 'History', 'Down', 'Up'],
    ['Codex', 'Use prompt history', 'History', 'Down', 'Up'],
    ['OpenCode', 'Use navigation', 'Navigate', 'Down', 'Up'],
  ])(
    'offers an optional rotate preset for %s without changing the default',
    async (name, preset, label, cw, ccw) => {
      const user = userEvent.setup();
      render(<Harness />);
      await selectProfile(user, name);
      expect(row('rotate')).toHaveTextContent('System volume');
      const saved = await ipc.getConfig();
      await user.click(within(row('rotate')).getByRole('button', { name: /^Edit/ }));
      await user.click(await screen.findByRole('button', { name: preset }));
      expect(screen.getByLabelText('Clockwise shortcut')).toHaveValue(cw);
      expect(screen.getByLabelText('Counter-clockwise shortcut')).toHaveValue(ccw);
      expect(await ipc.getConfig()).toEqual(saved);
      await user.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() => expect(row('rotate')).toHaveTextContent(label));
      expect((await ipc.getConfig()).profiles.find((p) => p.name === name)?.rotate.spec).toEqual({
        kind: 'shortcuts',
        cw,
        ccw,
        label,
      });
    },
  );

  it('counts down before testing a knob shortcut pair, then sends it as one test', async () => {
    const user = userEvent.setup();
    const pair = {
      kind: 'shortcuts',
      cw: 'Ctrl+Tab',
      ccw: 'Ctrl+Shift+Tab',
      label: 'Tabs',
    } as const;
    await ipc.setRotate('general', pair);
    render(<Harness />);
    await screen.findByTestId('gesture-rotate');
    await rowActionFake(user, 'rotate', 'Test');
    expect(within(row('rotate')).getByRole('status')).toHaveTextContent('Running in 3');
    expect(ipc.testRotate).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(3000));
    expect(ipc.testRotate).toHaveBeenCalledWith(pair);
  });

  it('cancels a pending shortcut test when selecting another control', async () => {
    const user = userEvent.setup();
    await ipc.setRotate('general', {
      kind: 'shortcuts',
      cw: 'Ctrl+Tab',
      ccw: 'Ctrl+Shift+Tab',
      label: 'Tabs',
    });
    render(<Harness />);
    await screen.findByTestId('gesture-rotate');
    await rowActionFake(user, 'rotate', 'Test');
    await user.click(screen.getByRole('tab', { name: 'Left button' }));
    act(() => vi.advanceTimersByTime(3000));
    expect(ipc.testRotate).not.toHaveBeenCalled();
    expect(row('button1')).toBeVisible();
  });

  it('counts down 3 seconds before testing a shortcut, and Cancel stops it', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await selectProfile(user, 'Browser');
    await user.click(screen.getByRole('tab', { name: 'Left button' }));

    await rowActionFake(user, 'button1', 'Test');
    expect(within(row('button1')).getByRole('status')).toHaveTextContent('Running in 3');
    act(() => vi.advanceTimersByTime(2000));
    expect(within(row('button1')).getByRole('status')).toHaveTextContent('Running in 1');
    expect(ipc.testAction).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1000));
    expect(ipc.testAction).toHaveBeenCalledWith({ kind: 'shortcut', keys: 'Alt+Left' });
    expect(within(row('button1')).queryByRole('status')).not.toBeInTheDocument();

    vi.mocked(ipc.testAction).mockClear();
    await rowActionFake(user, 'button1', 'Test');
    await user.click(within(row('button1')).getByRole('button', { name: 'Cancel' }));
    act(() => vi.advanceTimersByTime(5000));
    expect(ipc.testAction).not.toHaveBeenCalled();
    expect(within(row('button1')).queryByRole('status')).not.toBeInTheDocument();
  });

  it('keeps the middle button Hold locked as mic mute', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(await screen.findByRole('tab', { name: 'Middle button' }));
    await screen.findByTestId('gesture-button2Hold');
    expect(row('button2Hold')).toHaveTextContent('Mic mute (every capture device)');
    expect(within(row('button2Hold')).queryByRole('button')).not.toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: 'Left button' }));
    expect(
      within(row('button1Hold')).getAllByRole('button', { name: /^Edit/ }).length,
    ).toBeGreaterThan(0);
  });

  it('asks before resetting a profile or everything', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /Next track/ }));
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(row('press')).toHaveTextContent('Custom'));

    await openTab(user, 'Reset & help');
    await user.click(screen.getByRole('button', { name: 'Reset profile' }));
    const dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Keep my settings' }));
    await openTab(user, 'Controls');
    expect(row('press')).toHaveTextContent('Custom');

    await openTab(user, 'Reset & help');
    await user.click(screen.getByRole('button', { name: 'Reset everything' }));
    await user.click(
      within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Reset' }),
    );
    await openTab(user, 'Controls');
    await waitFor(() => expect(row('press')).not.toHaveTextContent('Custom'));
  });

  it('selects a profile from the rail and keeps the active mark on the profile in use', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const nav = await screen.findByRole('navigation', { name: 'Profile' });
    const general = within(nav).getByTestId('profile-chip-general');
    expect(general).toHaveAttribute('aria-pressed', 'true');
    expect(general).toHaveTextContent('Active now');
    await user.click(within(nav).getByTestId('profile-chip-browser'));
    expect(within(nav).getByTestId('profile-chip-browser')).toHaveAttribute('aria-pressed', 'true');
    expect(general).toHaveAttribute('aria-pressed', 'false');
    expect(general).toHaveTextContent('Active now');
    expect(within(nav).getByTestId('profile-chip-browser')).not.toHaveTextContent('Active now');
    expect(screen.getByTestId('profile-browser')).toBeVisible();
  });

  it('starts a test from the visible Test button', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    await user.click(within(row('press')).getByRole('button', { name: 'Test Press' }));
    expect(ipc.testAction).toHaveBeenCalledWith({ kind: 'systemMute' });
  });

  it('shows Add action on an unbound row and opens the picker', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(await screen.findByRole('tab', { name: 'Left button' }));
    const hold = row('button1Hold');
    expect(hold).toHaveTextContent('Add action');
    expect(hold).not.toHaveTextContent('Not set');
    expect(within(hold).queryByRole('button', { name: /^Test / })).not.toBeInTheDocument();
    await user.click(within(hold).getByText('Add action'));
    expect(await screen.findByRole('dialog', { name: 'Edit Left button hold' })).toBeVisible();
  });

  it('renders fixed rows in the built-in gestures group', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-rotate');
    const builtIn = within(selectedPanel()).getByRole('list', { name: 'Built-in gestures' });
    expect(within(builtIn).getByTestId('gesture-heldRotate')).toBeVisible();
    expect(within(builtIn).getByTestId('gesture-doublePress')).toBeVisible();
    expect(within(builtIn).getByTestId('gesture-hold')).toBeVisible();
    expect(within(builtIn).getByTestId('gesture-hold').querySelector('button')).toBeNull();
    await user.click(screen.getByRole('tab', { name: 'Middle button' }));
    const middle = within(selectedPanel()).getByRole('list', { name: 'Built-in gestures' });
    expect(within(middle).getByTestId('gesture-button2Hold')).toHaveTextContent(
      'Mic mute (every capture device)',
    );
    expect(within(middle).getByTestId('gesture-button2Hold').querySelector('button')).toBeNull();
    await user.click(screen.getByRole('tab', { name: 'Left button' }));
    expect(
      within(selectedPanel()).queryByRole('list', { name: 'Built-in gestures' }),
    ).not.toBeInTheDocument();
  });
});

describe('ControlsPage tabs', () => {
  it('splits the page into Controls, Macros and Reset & help, and tells them apart from the profile chips', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const tabs = await screen.findByRole('tablist', { name: 'Controls sections' });
    expect(
      within(tabs)
        .getAllByRole('tab')
        .map((tab) => tab.textContent),
    ).toEqual(['Controls', 'Macros', 'Reset & help']);
    // The first screen is controls: the profile chips and the rows, no explanations or resets.
    // The profile chips wait for the config, which loads after the page frame.
    expect(await screen.findByRole('navigation', { name: 'Profile' })).toBeVisible();
    expect(await screen.findByTestId('gesture-press')).toBeVisible();
    expect(screen.queryByTestId('macros')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Reset everything' })).not.toBeInTheDocument();

    await user.click(within(tabs).getByRole('tab', { name: 'Macros' }));
    expect(await screen.findByTestId('macros')).toBeVisible();
    await waitFor(() => expect(screen.queryByTestId('gesture-press')).not.toBeInTheDocument());

    await user.click(within(tabs).getByRole('tab', { name: 'Reset & help' }));
    expect(await screen.findByRole('button', { name: 'Reset everything' })).toBeVisible();
    expect(screen.getByText('How Kivori reports results')).toBeVisible();
    expect(screen.getByText('Restart Kivori')).toBeVisible();
  });

  it('still switches profile from the chips', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await selectProfile(user, 'Browser');
    expect(screen.getByTestId('profile-chip-browser')).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByTestId('profile-browser')).toBeVisible();
  });
});

describe('ControlsPage macros', () => {
  const stepTexts = (): string[] =>
    within(screen.getByRole('list', { name: 'Steps' }))
      .getAllByRole('listitem')
      .map((li) => li.textContent ?? '');

  async function addStep(user: ReturnType<typeof userEvent.setup>, choice: RegExp): Promise<void> {
    await user.click(screen.getByRole('button', { name: 'Add step' }));
    const picker = await screen.findByRole('dialog', { name: 'Add a step' });
    await user.click(within(picker).getByRole('radio', { name: choice }));
    await user.click(within(picker).getByRole('button', { name: 'Add step' }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Add a step' })).not.toBeInTheDocument(),
    );
  }

  it('creates a macro, reorders its steps, binds it, and refuses to delete it while bound', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-press');

    await openTab(user, 'Macros');
    await user.click(screen.getByRole('button', { name: 'New macro' }));
    await user.type(await screen.findByLabelText('Name'), 'Standup');
    await addStep(user, /^Mute/);
    await addStep(user, /^Wait/);
    await addStep(user, /^Play \/ Pause/);
    expect(stepTexts().map((s) => s.replace(/^\d/, ''))).toEqual([
      'Mute',
      'Wait 500 ms',
      'Play / Pause',
    ]);
    // Mute is confirmed, Play / Pause is not: the macro is as certain as the weaker one.
    expect(screen.getByTestId('macro-least')).toHaveTextContent(
      'Least certain step: Result unknown',
    );

    await user.click(screen.getByRole('button', { name: 'Move step 3 up' }));
    expect(stepTexts().map((s) => s.replace(/^\d/, ''))).toEqual([
      'Mute',
      'Play / Pause',
      'Wait 500 ms',
    ]);
    await user.click(screen.getByRole('button', { name: 'Save macro' }));

    const saved = await screen.findByTestId('macro-standup');
    expect(saved).toHaveTextContent('Standup');
    expect(saved).toHaveTextContent('Mute → Play / Pause → Wait 500 ms');

    await openTab(user, 'Controls');
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /^Macro/ }));
    await user.selectOptions(screen.getByLabelText('Macro', { selector: 'select' }), 'Standup');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(row('press')).toHaveTextContent('Macro: Standup'));
    expect(row('press')).toHaveTextContent('Custom');

    await openTab(user, 'Macros');
    const bound = screen.getByTestId('macro-standup');
    const boundDelete = within(bound).getByRole('button', { name: 'Delete' });
    expect(bound).toHaveTextContent('Used by: General · Press');
    expect(boundDelete).toBeDisabled();
    expect(boundDelete).toHaveAttribute('aria-describedby', 'macro-standup-delete-reason');
    expect(screen.getByText('Unbind it from the controls above first.')).toBeVisible();

    // Unbound, it can go, and only after the confirm.
    await openTab(user, 'Controls');
    await rowAction(user, 'press', 'Reset');
    await waitFor(() => expect(row('press')).not.toHaveTextContent('Macro: Standup'));
    await openTab(user, 'Macros');
    const freed = screen.getByTestId('macro-standup');
    expect(freed).toHaveTextContent('Not used by any control yet');
    await user.click(within(freed).getByRole('button', { name: 'Delete' }));
    const dialog = await screen.findByRole('alertdialog', { name: /Delete Standup/ });
    expect(screen.getByTestId('macro-standup')).toBeVisible();
    await user.click(within(dialog).getByRole('button', { name: 'Keep it' }));
    expect(screen.getByTestId('macro-standup')).toBeVisible();
    await user.click(
      within(screen.getByTestId('macro-standup')).getByRole('button', { name: 'Delete' }),
    );
    await user.click(
      within(await screen.findByRole('alertdialog', { name: /Delete Standup/ })).getByRole(
        'button',
        { name: 'Delete' },
      ),
    );
    await waitFor(() => expect(screen.queryByTestId('macro-standup')).not.toBeInTheDocument());
  });

  it('limits a macro to 8 steps and never offers a macro as a step', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openTab(user, 'Macros');
    await user.click(await screen.findByRole('button', { name: 'New macro' }));
    await user.type(await screen.findByLabelText('Name'), 'Many');
    for (let i = 0; i < 8; i += 1) await addStep(user, /^Next track/);
    expect(screen.getByRole('button', { name: 'Add step' })).toBeDisabled();
    expect(screen.getByText('A macro has at most 8 steps.')).toBeVisible();
  });

  it('keeps Save off until the macro has a name and an action, and checks the wait range', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openTab(user, 'Macros');
    await user.click(await screen.findByRole('button', { name: 'New macro' }));
    await user.type(await screen.findByLabelText('Name'), 'Pause only');
    expect(screen.getByRole('button', { name: 'Save macro' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Add step' }));
    const picker = await screen.findByRole('dialog', { name: 'Add a step' });
    expect(within(picker).queryByRole('radio', { name: /^Macro/ })).not.toBeInTheDocument();
    await user.click(within(picker).getByRole('radio', { name: /^Wait/ }));
    await user.clear(within(picker).getByLabelText('Wait (milliseconds)'));
    await user.type(within(picker).getByLabelText('Wait (milliseconds)'), '49');
    expect(within(picker).getByText('Wait between 50 and 2000 milliseconds.')).toBeVisible();
    expect(within(picker).getByRole('button', { name: 'Add step' })).toBeDisabled();
  });

  const clickTemplate = async (
    user: ReturnType<typeof userEvent.setup>,
    name: RegExp,
  ): Promise<void> => {
    const button = await screen.findByRole('button', { name });
    await waitFor(() => expect(button).toBeEnabled());
    await user.click(button);
  };

  it('opens a template prefilled and saves it with saveMacro', async () => {
    const user = userEvent.setup();
    const save = vi.spyOn(ipc, 'saveMacro');
    render(<Harness />);
    await openTab(user, 'Macros');
    const card = await screen.findByTestId('macros');
    expect(card.querySelector('img')).toHaveAttribute('alt', '');
    expect(card.querySelector('img')).toHaveClass('size-12');
    expect(screen.getByText('No macros yet.')).toBeVisible();
    expect(screen.getByText('Start from a template below, or create a new macro.')).toBeVisible();

    await clickTemplate(user, /^Copy, then paste/);
    expect(await screen.findByLabelText('Name')).toHaveValue('Copy, then paste');
    expect(stepTexts().map((text) => text.replace(/^\d/, ''))).toEqual([
      'Keyboard shortcut: Ctrl+C',
      'Wait 150 ms',
      'Keyboard shortcut: Ctrl+V',
    ]);
    await user.click(screen.getByRole('button', { name: 'Save macro' }));
    const steps = macroTemplates('windows').find((template) => template.key === 'copyPaste')?.steps;
    expect(save).toHaveBeenCalledWith({ id: 'copy-then-paste', name: 'Copy, then paste', steps });
    expect(await screen.findByTestId('macro-copy-then-paste')).toBeVisible();
    save.mockRestore();
  });

  it('tests a macro from the list, counting down when it sends keys', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openTab(user, 'Macros');
    await clickTemplate(user, /^Quiet mode/);
    await user.click(screen.getByRole('button', { name: 'Save macro' }));
    const quiet = await screen.findByTestId('macro-quiet-mode');
    await user.click(within(quiet).getByRole('button', { name: 'Test Quiet mode' }));
    expect(ipc.testAction).toHaveBeenCalledWith({ kind: 'macro', id: 'quiet-mode' });
    expect(within(quiet).queryByRole('status')).not.toBeInTheDocument();
    await waitFor(() =>
      expect(quiet.querySelector('[data-result]')).toHaveAttribute('data-result', 'unverified'),
    );

    vi.mocked(ipc.testAction).mockClear();
    await clickTemplate(user, /^Select all and copy/);
    await user.click(screen.getByRole('button', { name: 'Save macro' }));
    const row = await screen.findByTestId('macro-select-all-and-copy');
    const test = within(row).getByRole('button', { name: 'Test Select all and copy' });
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    await user.click(test);
    expect(within(row).getByRole('status')).toHaveTextContent('Running in 3');
    expect(within(row).getByRole('button', { name: 'Cancel' })).toBeVisible();
    expect(ipc.testAction).not.toHaveBeenCalled();
    await user.click(within(row).getByRole('button', { name: 'Cancel' }));
    act(() => vi.advanceTimersByTime(5000));
    expect(ipc.testAction).not.toHaveBeenCalled();

    vi.useRealTimers();
    const again = within(row).getByRole('button', { name: 'Test Select all and copy' });
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    await user.click(again);
    act(() => vi.advanceTimersByTime(3000));
    expect(ipc.testAction).toHaveBeenCalledWith({ kind: 'macro', id: 'select-all-and-copy' });
    await waitFor(() =>
      expect(row.querySelector('[data-result]')).toHaveAttribute('data-result', 'unverified'),
    );
    vi.useRealTimers();
  });

  it('replaces a step in place and leaves a delay without Edit', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openTab(user, 'Macros');
    await user.click(await screen.findByRole('button', { name: 'New macro' }));
    await user.type(await screen.findByLabelText('Name'), 'Swap');
    await addStep(user, /^Mute/);
    await addStep(user, /^Play \/ Pause/);
    await addStep(user, /^Wait/);
    expect(screen.queryByRole('button', { name: 'Edit step 3' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Edit step 1' }));
    const picker = await screen.findByRole('dialog', { name: 'Add a step' });
    expect(within(picker).getByRole('radio', { name: /^Mute/ })).toBeChecked();
    await user.click(within(picker).getByRole('radio', { name: /^Next track/ }));
    await user.click(within(picker).getByRole('button', { name: 'Add step' }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Add a step' })).not.toBeInTheDocument(),
    );
    expect(stepTexts().map((text) => text.replace(/^\d/, ''))).toEqual([
      'Next track',
      'Play / Pause',
      'Wait 500 ms',
    ]);
  });

  it('selects a macro created from the open picker', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /^Macro/ }));
    await user.click(screen.getByRole('button', { name: 'New macro…' }));
    const editor = await screen.findByRole('dialog', { name: 'New macro' });
    await user.type(within(editor).getByLabelText('Name'), 'From picker');
    await user.click(within(editor).getByRole('button', { name: 'Add step' }));
    const stepPicker = await screen.findByRole('dialog', { name: 'Add a step' });
    await user.click(within(stepPicker).getByRole('radio', { name: /^Mute/ }));
    await user.click(within(stepPicker).getByRole('button', { name: 'Add step' }));
    await user.click(within(editor).getByRole('button', { name: 'Save macro' }));
    await waitFor(() =>
      expect(screen.getByLabelText('Macro', { selector: 'select' })).toHaveValue('from-picker'),
    );
    expect(screen.getByRole('dialog', { name: /Edit Press/ })).toBeVisible();
    expect(screen.queryByRole('dialog', { name: 'New macro' })).not.toBeInTheDocument();
  });
});

describe('ControlsPage notice', () => {
  const withNotice = (notice: ConfigDto['notice']): ConfigDto => ({
    ...ipc_config(),
    notice,
  });

  function ipc_config(): ConfigDto {
    return {
      version: 1,
      revision: 1,
      notice: null,
      profiles: [],
      macros: [],
      display: { defaultView: 'buddy', secondaryView: 'system', brightness: 5 },
      buddy: { reactions: true, intensity: 'normal' },
    };
  }

  it('explains a failed backup without claiming recovery succeeded', () => {
    render(<ControlsPage desk={null} config={withNotice('backupFailed')} />);
    const notice = screen.getByTestId('config-notice');
    expect(notice).toHaveTextContent('Your original settings need a backup');
    expect(notice).toHaveTextContent('They have not been replaced');
    expect(notice).toHaveTextContent('will retry the backup first');
    expect(notice).not.toHaveTextContent('kept a backup');
  });

  it('explains a recovered corrupt file in plain language', () => {
    render(<ControlsPage desk={null} config={withNotice('recoveredCorrupt')} />);
    expect(screen.getByTestId('config-notice')).toHaveTextContent('Your settings were damaged');
    expect(screen.getByTestId('config-notice')).toHaveTextContent('kept a backup');
  });

  it('explains a newer-version file and a migration, and shows nothing otherwise', () => {
    const { rerender } = render(
      <ControlsPage desk={null} config={withNotice('recoveredNewerVersion')} />,
    );
    expect(screen.getByTestId('config-notice')).toHaveTextContent('newer Kivori');
    rerender(<ControlsPage desk={null} config={withNotice('migrated')} />);
    expect(screen.getByTestId('config-notice')).toHaveTextContent('Nothing was lost');
    rerender(<ControlsPage desk={null} config={withNotice(null)} />);
    expect(screen.queryByTestId('config-notice')).not.toBeInTheDocument();
  });
});
