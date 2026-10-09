import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@/components/ui/sonner';
import { useConfig, useDeskStatus } from '../../../hooks/use-kivori';
import type { ConfigDto } from '../../../lib/ipc/types';

// The browser mock behind the real IPC wrappers; only `testAction` is spied on.
vi.mock('../../../lib/ipc', async (importOriginal) => {
  const real = await importOriginal<typeof import('../../../lib/ipc')>();
  return { ...real, testAction: vi.fn(real.testAction) };
});

import * as ipc from '../../../lib/ipc';
import { ControlsPage } from '../ControlsPage';

function Harness(): ReactElement {
  const desk = useDeskStatus();
  const config = useConfig();
  return (
    <>
      <ControlsPage desk={desk} config={config} />
      <Toaster />
    </>
  );
}

const row = (key: string): HTMLElement => screen.getByTestId(`gesture-${key}`);

async function openPressEditor(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  await screen.findByTestId('gesture-press');
  await user.click(within(row('press')).getByRole('button', { name: 'Edit' }));
  await screen.findByRole('dialog');
}

beforeEach(async () => {
  window.history.pushState({}, '', '/');
  await ipc.resetConfig();
  vi.mocked(ipc.testAction).mockClear();
});

afterEach(() => {
  vi.useRealTimers();
  window.history.pushState({}, '', '/');
});

describe('ControlsPage', () => {
  it('opens on the active profile and shows each control with its action', async () => {
    render(<Harness />);
    expect(await screen.findByRole('tab', { name: 'General', selected: true })).toBeVisible();
    expect(row('rotate')).toHaveTextContent('System volume');
    expect(row('press')).toHaveTextContent('Play / Pause');
    expect(row('hold')).toHaveTextContent('Mute');
    expect(row('button1')).toHaveTextContent('Previous track');
    expect(row('button1Hold')).toHaveTextContent('Not set');
    expect(screen.getByText(/not show them on screen yet/)).toBeVisible();
    expect(screen.queryByText('Custom')).not.toBeInTheDocument();
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

    await user.click(within(row('press')).getByRole('button', { name: 'Reset' }));
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
    await user.click(within(row('rotate')).getByRole('button', { name: 'Edit' }));
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
    await user.click(within(row('hold')).getByRole('button', { name: 'Test' }));
    expect(ipc.testAction).toHaveBeenCalledWith({ kind: 'systemMute' });
    expect(
      await within(row('hold')).findByText('Confirmed', { selector: '[data-result]' }),
    ).toBeVisible();
  });

  it('counts down 3 seconds before testing a shortcut, and Cancel stops it', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(await screen.findByRole('tab', { name: 'Browser' }));
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });

    await user.click(within(row('button1')).getByRole('button', { name: 'Test' }));
    expect(within(row('button1')).getByRole('status')).toHaveTextContent('Running in 3');
    act(() => vi.advanceTimersByTime(2000));
    expect(within(row('button1')).getByRole('status')).toHaveTextContent('Running in 1');
    expect(ipc.testAction).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1000));
    expect(ipc.testAction).toHaveBeenCalledWith({ kind: 'shortcut', keys: 'Alt+Left' });
    expect(within(row('button1')).queryByRole('status')).not.toBeInTheDocument();

    vi.mocked(ipc.testAction).mockClear();
    await user.click(within(row('button1')).getByRole('button', { name: 'Test' }));
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
    expect(within(row('button1Hold')).getByRole('button', { name: 'Edit' })).toBeVisible();
  });

  it('asks before resetting a profile or everything', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /Next track/ }));
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(row('press')).toHaveTextContent('Custom'));

    await user.click(screen.getByRole('button', { name: 'Reset profile' }));
    const dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Keep my settings' }));
    expect(row('press')).toHaveTextContent('Custom');

    await user.click(screen.getByRole('button', { name: 'Reset everything' }));
    await user.click(
      within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Reset' }),
    );
    await waitFor(() => expect(row('press')).not.toHaveTextContent('Custom'));
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

    await openPressEditor(user);
    await user.click(screen.getByRole('radio', { name: /^Macro/ }));
    await user.selectOptions(screen.getByLabelText('Macro', { selector: 'select' }), 'Standup');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(row('press')).toHaveTextContent('Macro: Standup'));
    expect(row('press')).toHaveTextContent('Custom');

    await user.click(
      within(screen.getByTestId('macro-standup')).getByRole('button', { name: 'Delete' }),
    );
    const toast = await screen.findByText('Couldn’t delete that macro');
    expect(toast).toBeVisible();
    expect(screen.getByText(/still bound to a control/)).toBeVisible();
    expect(screen.getByTestId('macro-standup')).toBeVisible();

    // Unbound, it can go.
    await user.click(within(row('press')).getByRole('button', { name: 'Reset' }));
    await waitFor(() => expect(row('press')).not.toHaveTextContent('Macro: Standup'));
    await user.click(
      within(screen.getByTestId('macro-standup')).getByRole('button', { name: 'Delete' }),
    );
    await waitFor(() => expect(screen.queryByTestId('macro-standup')).not.toBeInTheDocument());
  });

  it('limits a macro to 8 steps and never offers a macro as a step', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(await screen.findByRole('button', { name: 'New macro' }));
    await user.type(await screen.findByLabelText('Name'), 'Many');
    for (let i = 0; i < 8; i += 1) await addStep(user, /^Next track/);
    expect(screen.getByRole('button', { name: 'Add step' })).toBeDisabled();
    expect(screen.getByText('A macro has at most 8 steps.')).toBeVisible();
  });

  it('keeps Save off until the macro has a name and an action, and checks the wait range', async () => {
    const user = userEvent.setup();
    render(<Harness />);
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
      display: { defaultView: 'buddy', secondaryView: 'system' },
      buddy: { reactions: true, intensity: 'normal' },
    };
  }

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
