import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { VirtualDevice } from './VirtualDevice';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const mockedInvoke = vi.mocked(invoke);
const putImageData = vi.fn();
const clearRect = vi.fn();

beforeEach(() => {
  vi.useFakeTimers();
  mockedInvoke.mockReset();
  putImageData.mockReset();
  clearRect.mockReset();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    putImageData,
    clearRect,
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
      generation: 1,
      volume: 42,
      muted: true,
      mediaTriggers: 3,
      frame,
      error: null,
    });
    await act(async () => {
      render(<VirtualDevice />);
    });
    expect(screen.getByRole('status')).toHaveTextContent('Connected');
    expect(screen.getByText('Volume: 42%')).toBeInTheDocument();
    expect(screen.getByText('Output mute: muted')).toBeInTheDocument();
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
      finish({
        connected: true,
        generation: 1,
        volume: 0,
        mediaTriggers: 0,
        frame: null,
        error: null,
      });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
  });

  it('sends knob turns and keyboard button edges without repeated presses', async () => {
    mockedInvoke.mockImplementation(async (command) =>
      command === 'virtual_snapshot'
        ? { connected: true, generation: 1, volume: 0, mediaTriggers: 0, frame: null, error: null }
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
      { control: 'counterclockwise', generation: 1 },
      { control: 'clockwise', generation: 1 },
      { control: 'press', generation: 1 },
      { control: 'release', generation: 1 },
    ]);
  });

  it('reports rejected input without retaining a connected status', async () => {
    mockedInvoke.mockResolvedValueOnce({
      connected: true,
      generation: 1,
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
        ? { connected: true, generation: 1, volume: 50, mediaTriggers: 0, frame: null, error: null }
        : undefined,
    );
    await act(async () => {
      render(<VirtualDevice />);
    });
    fireEvent.click(screen.getByRole('button', { name: 'Press once' }));
    expect(mockedInvoke).toHaveBeenCalledWith('virtual_input', { control: 'tap', generation: 1 });
  });
});

const connectedSnapshot = {
  connected: true,
  connectionState: 'connected',
  generation: 1,
  mode: 'normal',
  volume: 50,
  mediaTriggers: 0,
  frame: null,
  error: null,
};

describe('virtual connection controls', () => {
  it('clears the live screen and disables raw inputs after intentional disconnect', async () => {
    let current = {
      ...connectedSnapshot,
      frame: Array<number>(240 * 240).fill(0xffff) as number[] | null,
    };
    mockedInvoke.mockImplementation(async (command) =>
      command === 'virtual_snapshot' ? current : undefined,
    );
    await act(async () => {
      render(<VirtualDevice />);
    });
    expect(putImageData).toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }));
    });
    expect(mockedInvoke).toHaveBeenCalledWith('virtual_connection', { action: 'disconnect' });
    current = { ...current, connected: false, connectionState: 'disconnected', frame: null };
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(screen.getByRole('status')).toHaveTextContent('Disconnected');
    expect(screen.getByRole('button', { name: 'Turn right' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Hold knob' })).toBeDisabled();
    expect(clearRect).toHaveBeenLastCalledWith(0, 0, 240, 240);
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Reconnect' }));
    });
    expect(mockedInvoke).toHaveBeenCalledWith('virtual_connection', { action: 'reconnect' });
  });

  it('resets a held knob on a new generation without releasing into the new session', async () => {
    let current = connectedSnapshot;
    mockedInvoke.mockImplementation(async (command) =>
      command === 'virtual_snapshot' ? current : undefined,
    );
    await act(async () => {
      render(<VirtualDevice />);
    });
    const hold = screen.getByRole('button', { name: 'Hold knob' });
    fireEvent.keyDown(hold, { key: ' ' });
    current = { ...current, generation: 2, mode: 'fragmented' };
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    fireEvent.keyUp(hold, { key: ' ' });
    expect(mockedInvoke.mock.calls.filter(([command]) => command === 'virtual_input')).toEqual([
      ['virtual_input', { control: 'press', generation: 1 }],
    ]);
    expect(screen.getByText('Generation: 2 · Mode: fragmented')).toBeInTheDocument();
    fireEvent.keyDown(hold, { key: ' ' });
    fireEvent.blur(window);
    expect(mockedInvoke).toHaveBeenLastCalledWith('virtual_input', {
      control: 'release',
      generation: 2,
    });
  });

  it('offers only fixed fault arguments and retains command errors across successful polls', async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === 'virtual_snapshot') return connectedSnapshot;
      if (command === 'virtual_connection') throw new Error('Runner start failed');
      return undefined;
    });
    await act(async () => {
      render(<VirtualDevice />);
    });
    expect(
      screen.getAllByRole('option').map((option) => (option as HTMLOptionElement).value),
    ).toEqual(['fragmented', 'silent', 'exit', 'wrong-major', 'unknown-product', 'unknown-board']);
    fireEvent.change(screen.getByLabelText('Fault mode'), { target: { value: 'wrong-major' } });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Apply fault' }));
    });
    expect(mockedInvoke).toHaveBeenCalledWith('virtual_connection', { action: 'wrong-major' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(screen.getByRole('alert')).toHaveTextContent('Runner start failed');
    expect(screen.getByRole('status')).toHaveTextContent('Unavailable');
    expect(screen.getByRole('button', { name: 'Turn right' })).toBeDisabled();
  });

  it('keeps only one connection operation in flight even while snapshots keep responding', async () => {
    let finish!: () => void;
    mockedInvoke.mockImplementation(async (command) => {
      if (command === 'virtual_snapshot') return connectedSnapshot;
      if (command === 'virtual_connection')
        await new Promise<void>((resolve) => {
          finish = resolve;
        });
      return undefined;
    });
    await act(async () => {
      render(<VirtualDevice />);
    });
    fireEvent.click(screen.getByRole('button', { name: 'Reconnect' }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply fault' }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    expect(
      mockedInvoke.mock.calls.filter(([command]) => command === 'virtual_connection'),
    ).toHaveLength(1);
    expect(screen.getByRole('status')).toHaveTextContent('Connecting');
    expect(screen.getByRole('button', { name: 'Reconnect' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Turn right' })).toBeDisabled();
    await act(async () => {
      finish();
    });
    expect(screen.getByRole('button', { name: 'Reconnect' })).toBeEnabled();
  });
  it('ignores an old snapshot completing after a connection operation', async () => {
    let finish!: (value: unknown) => void;
    mockedInvoke.mockImplementation(async (command) => {
      if (command === 'virtual_snapshot')
        return new Promise((resolve) => {
          finish = resolve;
        });
      return undefined;
    });
    render(<VirtualDevice />);
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }));
    });
    await act(async () => {
      finish(connectedSnapshot);
    });
    expect(screen.getByRole('status')).toHaveTextContent('Connecting');
    expect(screen.getByRole('button', { name: 'Turn right' })).toBeDisabled();
  });
  it('tags unmount release with the generation that originally pressed the knob', async () => {
    mockedInvoke.mockImplementation(async (command) =>
      command === 'virtual_snapshot' ? { ...connectedSnapshot, generation: 7 } : undefined,
    );
    let view!: ReturnType<typeof render>;
    await act(async () => {
      view = render(<VirtualDevice />);
    });
    fireEvent.keyDown(screen.getByRole('button', { name: 'Hold knob' }), { key: ' ' });
    view.unmount();
    expect(mockedInvoke.mock.calls.filter(([command]) => command === 'virtual_input')).toEqual([
      ['virtual_input', { control: 'press', generation: 7 }],
      ['virtual_input', { control: 'release', generation: 7 }],
    ]);
  });
});
