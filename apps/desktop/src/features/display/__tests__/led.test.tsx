import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@kivori/ui/components/sonner';
import { connectionFixture } from '../../../test/connection';
import { resetPageTabs } from '../../../components/kivori/page-tabs';
import { CAPABILITY_NAMES, DEFAULT_LED_SETTINGS } from '../../../lib/ipc/types';
import type { ConfigDto, LedSettingsDto } from '../../../lib/ipc/types';

const h = vi.hoisted(() => ({
  setLed: vi.fn<(led: unknown) => Promise<unknown>>(() => Promise.resolve({})),
}));

vi.mock('../../../lib/ipc', () => ({
  setLedSettings: h.setLed,
  setDisplayMode: vi.fn(() => Promise.resolve()),
  setDisplaySettings: vi.fn(() => Promise.resolve()),
  setBrightness: vi.fn(() => Promise.resolve()),
  setBuddySettings: vi.fn(() => Promise.resolve()),
  playMascotAction: vi.fn(() => Promise.resolve()),
}));

import { LedControls } from '../LedControls';
import { DisplayPage } from '../DisplayPage';

const profile = (id: string, name: string): ConfigDto['profiles'][number] =>
  ({
    id,
    name,
    custom: false,
    inPinCycle: false,
    apps: [],
  }) as unknown as ConfigDto['profiles'][number];

function configWith(led: Partial<LedSettingsDto> = {}, revision = 1): ConfigDto {
  return {
    version: 2,
    revision,
    notice: null,
    profiles: [profile('general', 'General'), profile('zoom', 'Zoom')],
    macros: [],
    display: { defaultView: 'buddy', secondaryView: 'system', brightness: 5 },
    buddy: { reactions: true, intensity: 'normal' },
    led: { ...DEFAULT_LED_SETTINGS, ...led },
  };
}

const lastSaved = (): LedSettingsDto => h.setLed.mock.lastCall?.[0] as LedSettingsDto;

beforeEach(() => {
  h.setLed.mockReset().mockResolvedValue({});
  resetPageTabs();
});

describe('LedControls', () => {
  it('starts on the saved mode and offers all six modes', () => {
    render(<LedControls config={configWith()} />);
    const modes = screen.getByRole('radiogroup', { name: 'Mode' });
    expect(within(modes).getAllByRole('radio')).toHaveLength(6);
    expect(within(modes).getByRole('radio', { name: /Mascot mood/ })).toBeChecked();
    expect(screen.getByTestId('led-summary')).toHaveTextContent('Mascot mood · 60%');
    // Mood picks its own colors: no color picker, no speed.
    expect(screen.queryByRole('radiogroup', { name: 'Color' })).not.toBeInTheDocument();
    expect(screen.getByText('This mode chooses its own colors.')).toBeInTheDocument();
    expect(screen.queryByRole('radiogroup', { name: 'Speed' })).not.toBeInTheDocument();
  });

  it('saves a mode change with every other setting kept', async () => {
    const user = userEvent.setup();
    render(<LedControls config={configWith()} />);
    await user.click(screen.getByRole('radio', { name: /Breathing/ }));
    expect(lastSaved()).toEqual({ ...DEFAULT_LED_SETTINGS, mode: 'breathe' });
    // Breathing uses the color and a speed.
    expect(screen.getByRole('radiogroup', { name: 'Color' })).toBeInTheDocument();
    expect(screen.getByRole('radiogroup', { name: 'Speed' })).toBeInTheDocument();
  });

  it.each([
    ['off', false, false],
    ['solid', true, false],
    ['breathe', true, true],
    ['cycle', false, true],
    ['status', true, false],
    ['mood', false, false],
  ] as const)('%s shows color %s and speed %s', (mode, color, speed) => {
    render(<LedControls config={configWith({ mode })} />);
    expect(!!screen.queryByRole('radiogroup', { name: 'Color' })).toBe(color);
    expect(!!screen.queryByRole('radiogroup', { name: 'Speed' })).toBe(speed);
    // Brightness matters for every mode but Off.
    expect(!!document.querySelector('input[type="range"][aria-label="Brightness"]')).toBe(
      mode !== 'off',
    );
  });

  it('saves a preset color and a speed', async () => {
    const user = userEvent.setup();
    render(<LedControls config={configWith({ mode: 'breathe' })} />);
    await user.click(screen.getByRole('radio', { name: 'Color #FF3B30' }));
    expect(lastSaved().color).toBe('#FF3B30');
    await user.click(screen.getByRole('radio', { name: 'Fast' }));
    expect(lastSaved()).toMatchObject({ color: '#FF3B30', speed: 'fast' });
  });

  it('saves a custom color only when the picker commits, not on every drag step', () => {
    render(<LedControls config={configWith({ mode: 'solid' })} />);
    const picker = screen.getByLabelText('Custom color');
    fireEvent.input(picker, { target: { value: '#123456' } });
    fireEvent.input(picker, { target: { value: '#abcdef' } });
    expect(h.setLed).not.toHaveBeenCalled();
    fireEvent.change(picker, { target: { value: '#abcdef' } });
    expect(h.setLed).toHaveBeenCalledTimes(1);
    expect(lastSaved().color).toBe('#ABCDEF');
  });

  it('shows the signals and a color per app in Status mode, and can clear one', async () => {
    const user = userEvent.setup();
    render(
      <LedControls config={configWith({ mode: 'status', profileColors: { zoom: '#34C759' } })} />,
    );
    const signals = screen.getByRole('heading', { name: 'Signals' }).parentElement!;
    for (const cue of ['Microphone muted', 'Not connected', 'Knob or key pressed']) {
      expect(within(signals).getByText(cue)).toBeInTheDocument();
    }
    const general = screen.getByLabelText('Color for General');
    fireEvent.change(general, { target: { value: '#ff8a00' } });
    expect(lastSaved().profileColors).toEqual({ zoom: '#34C759', general: '#FF8A00' });
    await user.click(screen.getByRole('button', { name: 'Use the default color for Zoom' }));
    expect(lastSaved().profileColors).toEqual({ general: '#FF8A00' });
    // Zoom is back on the default color, so it has nothing left to clear.
    expect(
      screen.queryByRole('button', { name: 'Use the default color for Zoom' }),
    ).not.toBeInTheDocument();
  });

  it.each([
    ['Custom color', { mode: 'solid' }],
    ['Color for Zoom', { mode: 'status' }],
  ] as const)('keeps what was dragged in the %s picker and saves it on commit', (label, led) => {
    render(<LedControls config={configWith(led)} />);
    const picker = screen.getByLabelText(label);
    // A real picker sends input events while dragging, then one change with the same value.
    fireEvent.input(picker, { target: { value: '#ff8a00' } });
    expect(picker).toHaveValue('#ff8a00');
    expect(h.setLed).not.toHaveBeenCalled();
    fireEvent.change(picker);
    expect(h.setLed).toHaveBeenCalledTimes(1);
    const saved = lastSaved();
    expect(label === 'Custom color' ? saved.color : saved.profileColors.zoom).toBe('#FF8A00');
  });

  it('reverts a failed save and says so', async () => {
    const user = userEvent.setup();
    h.setLed.mockRejectedValueOnce(new Error('the LED colour must be #RRGGBB'));
    render(
      <>
        <LedControls config={configWith()} />
        <Toaster />
      </>,
    );
    await user.click(screen.getByRole('radio', { name: /Static/ }));
    expect(await screen.findByText('Couldn’t save the LED settings')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /Mascot mood/ })).toBeChecked();
  });

  it('follows the saved config once it arrives', () => {
    const { rerender } = render(<LedControls config={configWith()} />);
    rerender(<LedControls config={configWith({ mode: 'cycle', brightness: 25 }, 2)} />);
    expect(screen.getByRole('radio', { name: /Color cycle/ })).toBeChecked();
    expect(screen.getByTestId('led-summary')).toHaveTextContent('Color cycle · 25%');
  });

  it('waits for the config before offering anything', () => {
    render(<LedControls config={null} />);
    expect(screen.queryByRole('radiogroup', { name: 'Mode' })).not.toBeInTheDocument();
  });

  it('has no accessibility violations', async () => {
    const { container } = render(
      <LedControls config={configWith({ mode: 'status', profileColors: { zoom: '#34C759' } })} />,
    );
    const results = await act(() => axe.run(container));
    expect(results.violations).toEqual([]);
  });
});

describe('DisplayPage LED tab', () => {
  const openLed = (): boolean => !!screen.queryByRole('tab', { name: 'LED' });

  it('is there for a board with an LED and with no device yet', () => {
    const { unmount } = render(
      <DisplayPage desk={null} config={configWith()} connection={connectionFixture()} />,
    );
    expect(openLed()).toBe(true);
    unmount();
    render(<DisplayPage desk={null} config={configWith()} connection={null} />);
    expect(openLed()).toBe(true);
  });

  it('is hidden for a board without an LED', () => {
    const noLed = CAPABILITY_NAMES.filter((name) => name !== 'rgbLedV1');
    render(
      <DisplayPage desk={null} config={configWith()} connection={connectionFixture([...noLed])} />,
    );
    expect(openLed()).toBe(false);
  });
});
