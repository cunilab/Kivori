import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { DeviceStudio } from '../DeviceStudio';
import { useStudioStore } from '../store';
import type { ConnectionStatusDto } from '../../../lib/ipc/types';

const bridge = vi.hoisted(() => ({
  update: vi.fn(),
  close: vi.fn(),
  open: vi.fn(),
  snapshot: vi.fn(),
  statusHandler: null as null | ((status: ConnectionStatusDto) => void),
}));
vi.mock('../../../lib/ipc', () => ({
  openPreviewStream: bridge.open,
  mirrorState: vi.fn(),
  getConnectionStatus: bridge.snapshot,
  onConnectionStatus: vi.fn((handler) => {
    bridge.statusHandler = handler;
    return Promise.resolve(() => {
      bridge.statusHandler = null;
    });
  }),
}));
vi.mock('../../../lib/canvas/DevicePreview', () => ({
  DevicePreview: (props: { frame?: Uint8ClampedArray; streaming?: boolean }) => (
    <div
      data-testid="preview"
      data-has-frame={Boolean(props.frame)}
      data-streaming={props.streaming}
    />
  ),
}));
vi.mock('../Controls', () => ({ Controls: () => null }));

beforeEach(() => {
  vi.useFakeTimers();
  useStudioStore.getState().reset();
  bridge.update.mockReset().mockResolvedValue(undefined);
  bridge.close.mockReset().mockResolvedValue(undefined);
  bridge.open.mockReset().mockImplementation((_state, _fps, onFrame) => {
    onFrame(new Uint8ClampedArray(4));
    return Promise.resolve({ update: bridge.update, close: bridge.close });
  });
  bridge.snapshot.mockReset().mockResolvedValue(
    status({
      connectionGeneration: 7,
      mascotAction: { action: 'pet', personality: 'cozy', seed: 16, appliedAtMs: 80_000 },
    }),
  );
  bridge.statusHandler = null;
});
afterEach(() => vi.useRealTimers());

function status(overrides: Partial<ConnectionStatusDto> = {}): ConnectionStatusDto {
  return {
    connection: 'connected',
    desired: 'idle',
    reported: 'idle',
    device: null,
    incompatibleReason: null,
    retryCount: 0,
    connectionGeneration: 1,
    mascotInteraction: true,
    mascotAction: null,
    host: 'active',
    ...overrides,
  };
}

it('keeps one live stream when the expression changes and sends event history', async () => {
  useStudioStore.getState().play();
  render(<DeviceStudio />);
  await act(async () => {
    await Promise.resolve();
  });
  act(() => useStudioStore.getState().setState('happy'));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(40);
  });
  expect(bridge.open).toHaveBeenCalledTimes(1);
  expect(bridge.update.mock.calls.at(-1)?.[0].events).toEqual([{ atMs: 0, state: 'happy' }]);
});

it('drops stale frames and enables exact-frame fallback when an update fails', async () => {
  useStudioStore.getState().play();
  render(<DeviceStudio />);
  await act(async () => {
    await Promise.resolve();
  });
  expect(screen.getByTestId('preview')).toHaveAttribute('data-has-frame', 'true');
  bridge.update.mockRejectedValue(new Error('preview unavailable'));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(40);
  });
  expect(screen.getByRole('alert')).toHaveTextContent('preview unavailable');
  expect(screen.getByTestId('preview')).toHaveAttribute('data-has-frame', 'false');
  expect(screen.getByTestId('preview')).toHaveAttribute('data-streaming', 'false');
});

it('records only live connected acknowledgments and rebases after device uptime resets', async () => {
  useStudioStore.getState().seek(2_000);
  useStudioStore.getState().play();
  render(<DeviceStudio />);
  await act(async () => {
    await Promise.resolve();
  });
  expect(useStudioStore.getState().actionEvents).toEqual([]);
  expect(bridge.snapshot).not.toHaveBeenCalled();

  act(() => {
    bridge.statusHandler?.(
      status({
        connection: 'connected',
        connectionGeneration: 8,
        mascotAction: { action: 'greet', personality: 'playful', seed: 17, appliedAtMs: 90_000 },
      }),
    );
  });
  act(() => {
    bridge.statusHandler?.(
      status({
        connection: 'disconnected',
        connectionGeneration: 8,
        mascotAction: { action: 'greet', personality: 'playful', seed: 17, appliedAtMs: 90_000 },
      }),
    );
  });
  act(() => {
    bridge.statusHandler?.(
      status({
        connection: 'connected',
        connectionGeneration: 9,
        mascotAction: { action: 'comfort', personality: 'calm', seed: 18, appliedAtMs: 15 },
      }),
    );
  });

  expect(useStudioStore.getState().actionEvents).toEqual([
    {
      action: 'greet',
      personality: 'playful',
      seed: 17,
      atMs: 2_000,
      deviceAppliedAtMs: 90_000,
      connectionGeneration: 8,
    },
    {
      action: 'comfort',
      personality: 'calm',
      seed: 18,
      atMs: 2_000,
      deviceAppliedAtMs: 15,
      connectionGeneration: 9,
    },
  ]);
  expect(useStudioStore.getState().playing).toBe(true);
});
