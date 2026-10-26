import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { VirtualDevice } from './VirtualDevice';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const mockedInvoke = vi.mocked(invoke);
const putImageData = vi.fn();

beforeEach(() => {
  vi.useFakeTimers();
  mockedInvoke.mockReset();
  putImageData.mockReset();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    putImageData,
  } as unknown as CanvasRenderingContext2D);
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('virtual device', () => {
  it('renders firmware pixels and simulated OS results, then reports a failed connection', async () => {
    const frame = Array<number>(240 * 240).fill(0);
    frame[0] = 0xf800;
    mockedInvoke.mockResolvedValueOnce({
      connected: true,
      volume: 42,
      mediaTriggers: 3,
      frame,
      error: null,
    });
    await act(async () => {
      render(<VirtualDevice />);
    });
    expect(screen.getByRole('status')).toHaveTextContent('Connected');
    expect(screen.getByText('Volume: 42%')).toBeInTheDocument();
    expect(screen.getByText('Media triggers: 3')).toBeInTheDocument();
    expect(Array.from((putImageData.mock.calls[0][0] as ImageData).data.slice(0, 4))).toEqual([
      255, 0, 0, 255,
    ]);
    mockedInvoke.mockRejectedValueOnce('Runner stopped');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(screen.getByRole('status')).toHaveTextContent('Unavailable');
    expect(screen.getByRole('alert')).toHaveTextContent('Runner stopped');
    expect(screen.getByText('Volume: —')).toBeInTheDocument();
  });

  it('allows only one pending snapshot and stops polling after unmount', async () => {
    let finish!: (value: unknown) => void;
    mockedInvoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }) as ReturnType<typeof invoke>,
    );
    const view = render(<VirtualDevice />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('alert')).toHaveTextContent('not responding');
    view.unmount();
    await act(async () => {
      finish({ connected: true, volume: 0, mediaTriggers: 0, frame: null, error: null });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
  });

  it('sends knob turns and keyboard button edges without repeated presses', async () => {
    mockedInvoke.mockImplementation(async (command) =>
      command === 'virtual_snapshot'
        ? { connected: true, volume: 0, mediaTriggers: 0, frame: null, error: null }
        : undefined,
    );
    await act(async () => {
      render(<VirtualDevice />);
    });
    fireEvent.click(screen.getByRole('button', { name: 'Turn left' }));
    fireEvent.click(screen.getByRole('button', { name: 'Turn right' }));
    const hold = screen.getByRole('button', { name: 'Hold knob' });
    fireEvent.keyDown(hold, { key: ' ' });
    fireEvent.keyDown(hold, { key: ' ', repeat: true });
    fireEvent.keyUp(hold, { key: ' ' });
    await act(async () => {});
    expect(
      mockedInvoke.mock.calls
        .filter(([command]) => command === 'virtual_input')
        .map(([, args]) => args),
    ).toEqual([
      { control: 'counterclockwise' },
      { control: 'clockwise' },
      { control: 'press' },
      { control: 'release' },
    ]);
  });

  it('reports rejected input without retaining a connected status', async () => {
    mockedInvoke.mockResolvedValueOnce({
      connected: true,
      volume: 42,
      mediaTriggers: 0,
      frame: null,
      error: null,
    });
    await act(async () => {
      render(<VirtualDevice />);
    });
    mockedInvoke.mockRejectedValueOnce('Input queue full');
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Turn right' }));
    });
    expect(screen.getByRole('alert')).toHaveTextContent('Input queue full');
    expect(screen.getByRole('status')).toHaveTextContent('Unavailable');
  });
  it('offers an accessible timed raw press without requiring pointer hold', async () => {
    mockedInvoke.mockImplementation(async (command) =>
      command === 'virtual_snapshot'
        ? { connected: true, volume: 50, mediaTriggers: 0, frame: null, error: null }
        : undefined,
    );
    await act(async () => {
      render(<VirtualDevice />);
    });
    fireEvent.click(screen.getByRole('button', { name: 'Press once' }));
    expect(mockedInvoke).toHaveBeenCalledWith('virtual_input', { control: 'tap' });
  });
});
