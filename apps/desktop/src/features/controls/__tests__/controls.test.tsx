import { connectionFixture } from '../../../test/connection';
import type { ConnectionStatusDto } from '../../../lib/ipc/types';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@kivori/ui/components/sonner';
import { useConfig, useDeskStatus } from '../../../hooks/use-kivori';
import type { ConfigDto } from '../../../lib/ipc/types';

// The browser mock behind the real IPC wrappers; `testAction` and `testRotate` are spied on.
vi.mock('../../../lib/ipc', async (importOriginal) => {
  const real = await importOriginal<typeof import('../../../lib/ipc')>();
  return { ...real, testAction: vi.fn(real.testAction), testRotate: vi.fn(real.testRotate) };
});

import * as ipc from '../../../lib/ipc';
import { resetPageTabs } from '../../../components/kivori/page-tabs';
import { ControlsPage } from '../ControlsPage';

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

const row = (key: string): HTMLElement => screen.getByTestId(`gesture-${key}`);

/** Runs an item of a row's overflow menu (Test or Reset). */
async function rowAction(
  user: ReturnType<typeof userEvent.setup>,
  key: string,
  item: string,
): Promise<void> {
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
    await user.click(within(row('press')).getByRole('button', { name: /^More actions for/ }));
    expect(await screen.findByRole('menuitem', { name: 'Test' })).not.toHaveAttribute(
      'aria-disabled',
      'true',
    );
    await user.keyboard('{Escape}');
    page.rerender(<Harness connection={null} />);
    expect(row('press')).toHaveTextContent('Keyboard shortcut: Ctrl+Alt+K');
    expect(await ipc.getConfig()).toEqual(saved);
  });

  it('opens on the active profile and shows each control with its action', async () => {
    render(<Harness />);
    expect(await screen.findByRole('tab', { name: 'General', selected: true })).toBeVisible();
    expect(row('rotate')).toHaveTextContent('System volume');
    expect(row('press')).toHaveTextContent('Play / Pause');
    expect(row('hold')).toHaveTextContent('Mute');
    expect(row('button1')).toHaveTextContent('Previous track');
    expect(row('button1Hold')).toHaveTextContent('Not set');
    // The disclaimers live under Reset & help, not above the controls.
    expect(screen.queryByTestId('controls-notes')).not.toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole('tab', { name: 'Reset & help' }));
    expect(screen.getByText(/not show them on screen yet/)).toBeVisible();
    expect(screen.getByTestId('mic-scope-note')).toBeVisible();
    expect(screen.queryByText('Custom')).not.toBeInTheDocument();
  });

  it('groups the rows under Knob and Buttons and keeps Test and Reset in an overflow menu', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    const knob = screen.getByRole('list', { name: 'Knob' });
    const buttons = screen.getByRole('list', { name: 'Buttons' });
    expect(within(knob).getAllByRole('listitem')).toHaveLength(4);
    expect(within(buttons).getAllByRole('listitem')).toHaveLength(6);
    // Test and Reset are not on the row itself.
    expect(within(row('press')).queryByRole('button', { name: 'Test' })).toBeNull();
    expect(within(row('press')).queryByRole('button', { name: 'Reset' })).toBeNull();
    await user.click(within(row('press')).getByRole('button', { name: 'More actions for Press' }));
    expect(await screen.findByRole('menuitem', { name: 'Test' })).toBeVisible();
    expect(screen.getByRole('menuitem', { name: 'Reset' })).toHaveAttribute(
      'aria-disabled',
      'true',
    );
  });

  it('opens the editor when the row text is clicked', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByTestId('gesture-press');
    await user.click(within(row('press')).getByText('Play / Pause'));
    expect(await screen.findByRole('dialog')).toBeVisible();
  });

  it('shows a friendly status mark instead of a Confirmed / Result unknown badge', async () => {
    render(<Harness />);
    await screen.findByTestId('gesture-hold');
    const mark = within(row('hold')).getByLabelText('Kivori can confirm this worked');
    expect(mark).toHaveAttribute('data-verification', 'confirmed');
    expect(
      within(row('press')).getByLabelText('Kivori can’t tell whether this worked'),
    ).toHaveAttribute('data-verification', 'unverified');
    expect(within(row('hold')).queryByText('Result unknown')).toBeNull();
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
    expect(row('press')).toHaveTextContent('Play / Pause');
  });

  it('shows a native rejection in the sheet and leaves the row unchanged', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /Keyboard shortcut/ }));
    await user.type(screen.getByLabelText('Shortcut'), 'Ctrl+');
    await user.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('not a valid shortcut');
    expect(row('press')).toHaveTextContent('Play / Pause');
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
    await screen.findByTestId('gesture-hold');
    await rowAction(user, 'hold', 'Test');
    expect(ipc.testAction).toHaveBeenCalledWith({ kind: 'systemMute' });
    expect(
      await within(row('hold')).findByText('Confirmed', { selector: '[data-result]' }),
    ).toBeVisible();
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

  it('counts down 3 seconds before testing a shortcut, and Cancel stops it', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(await screen.findByRole('tab', { name: 'Browser' }));

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

  it('keeps the middle button Hold locked as Pin profile', async () => {
    render(<Harness />);
    await screen.findByTestId('gesture-button2Hold');
    expect(row('button2Hold')).toHaveTextContent('Pin profile');
    expect(within(row('button2Hold')).queryByRole('button')).not.toBeInTheDocument();
    expect(within(row('button1Hold')).getByRole('button', { name: /^Edit/ })).toBeVisible();
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
    expect(await screen.findByRole('tablist', { name: 'Profile' })).toBeVisible();
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
    await user.click(await screen.findByRole('tab', { name: 'Browser' }));
    expect(await screen.findByRole('tab', { name: 'Browser', selected: true })).toBeVisible();
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
    await user.click(
      within(screen.getByTestId('macro-standup')).getByRole('button', { name: 'Delete' }),
    );
    const toast = await screen.findByText('Couldn’t delete that macro');
    expect(toast).toBeVisible();
    expect(screen.getByText(/still bound to a control/)).toBeVisible();
    expect(screen.getByTestId('macro-standup')).toBeVisible();

    // Unbound, it can go.
    await openTab(user, 'Controls');
    await rowAction(user, 'press', 'Reset');
    await waitFor(() => expect(row('press')).not.toHaveTextContent('Macro: Standup'));
    await openTab(user, 'Macros');
    await user.click(
      within(screen.getByTestId('macro-standup')).getByRole('button', { name: 'Delete' }),
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
