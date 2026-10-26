import { useEffect, useRef, useState, type ReactElement } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { blitRgba } from '../../lib/canvas/blit';
import './virtual-device.css';

type Snapshot = {
  connected: boolean;
  connectionState?: 'connected' | 'connecting' | 'disconnected' | 'error';
  generation?: number;
  mode?: string;
  volume: number;
  mediaTriggers: number;
  frame: number[] | null;
  error: string | null;
};
const FAULTS = [
  'fragmented',
  'silent',
  'exit',
  'wrong-major',
  'unknown-product',
  'unknown-board',
] as const;
type Control = 'clockwise' | 'counterclockwise' | 'press' | 'release' | 'tap';

export function VirtualDevice(): ReactElement {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [connectionBusy, setConnectionBusy] = useState(false);
  const [fault, setFault] = useState<string>(FAULTS[0]);
  const operationPending = useRef(false);
  const connectionRevision = useRef(0);
  const canvas = useRef<HTMLCanvasElement>(null);
  const held = useRef<number | null>(null);

  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    let watchdog: ReturnType<typeof setTimeout>;
    async function poll() {
      const revision = connectionRevision.current;
      watchdog = setTimeout(() => {
        if (!stopped && revision === connectionRevision.current) {
          setSnapshot(null);
          setError('Virtual device is not responding.');
        }
      }, 3000);
      try {
        const next = await invoke<Snapshot>('virtual_snapshot');
        if (!stopped && revision === connectionRevision.current) {
          setSnapshot(next);
          setError(next.error);
        }
      } catch (cause) {
        if (!stopped && revision === connectionRevision.current) {
          setSnapshot(null);
          setError(String(cause));
        }
      } finally {
        clearTimeout(watchdog);
      }
      if (!stopped) timer = setTimeout(() => void poll(), 100);
    }
    void poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
      clearTimeout(watchdog);
    };
  }, []);

  const connectionState =
    snapshot?.connectionState ?? (snapshot?.connected ? 'connected' : 'connecting');
  const connected =
    connectionState === 'connected' &&
    !!snapshot?.connected &&
    !error &&
    !connectionBusy &&
    !connectionError;

  useEffect(() => {
    held.current = null;
  }, [snapshot?.generation, connectionState, connectionBusy]);

  useEffect(() => {
    const ctx = canvas.current?.getContext('2d');
    if (!ctx) return;
    if (!connected || !snapshot?.frame || snapshot.frame.length !== 240 * 240) {
      ctx.clearRect(0, 0, 240, 240);
      return;
    }
    const rgba = new Uint8ClampedArray(240 * 240 * 4);
    snapshot.frame.forEach((pixel, index) => {
      rgba[index * 4] = Math.round((((pixel >> 11) & 31) * 255) / 31);
      rgba[index * 4 + 1] = Math.round((((pixel >> 5) & 63) * 255) / 63);
      rgba[index * 4 + 2] = Math.round(((pixel & 31) * 255) / 31);
      rgba[index * 4 + 3] = 255;
    });
    blitRgba(ctx, rgba, 240);
  }, [snapshot, connected]);

  async function connection(action: string) {
    if (operationPending.current) return;
    operationPending.current = true;
    connectionRevision.current += 1;
    held.current = null;
    setConnectionBusy(true);
    setConnectionError(null);
    try {
      await invoke('virtual_connection', { action });
      setSnapshot(null);
      setError(null);
    } catch (cause) {
      setConnectionError(String(cause));
    } finally {
      operationPending.current = false;
      setConnectionBusy(false);
    }
  }

  async function input(control: Control, generation = snapshot?.generation ?? 0) {
    if (!connected || operationPending.current) return;
    try {
      await invoke('virtual_input', { control, generation });
    } catch (cause) {
      setSnapshot(null);
      setError(String(cause));
    }
  }
  function press() {
    if (held.current !== null) return;
    if (!connected || operationPending.current) return;
    held.current = snapshot?.generation ?? 0;
    void input('press', held.current);
  }
  function release() {
    if (held.current === null) return;
    const generation = held.current;
    held.current = null;
    void input('release', generation);
  }
  useEffect(() => {
    const onBlur = () => {
      if (held.current !== null) {
        const generation = held.current;
        held.current = null;
        void invoke('virtual_input', { control: 'release', generation }).catch((cause) => {
          setSnapshot(null);
          setError(String(cause));
        });
      }
    };
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('blur', onBlur);
      if (held.current !== null)
        void invoke('virtual_input', { control: 'release', generation: held.current }).catch(
          () => {},
        );
    };
  }, []);

  return (
    <main className="virtual-device">
      <header>
        <p>Development only · isolated settings</p>
        <h1>Virtual Kivori device</h1>
      </header>
      <section aria-label="Simulated device">
        <h2>Simulated device</h2>
        <p role="status">
          {error || connectionError || connectionState === 'error'
            ? 'Unavailable'
            : connectionBusy || connectionState === 'connecting'
              ? 'Connecting…'
              : connected
                ? 'Connected'
                : 'Disconnected'}
        </p>
        <p>
          Generation: {snapshot?.generation ?? '—'} · Mode: {snapshot?.mode ?? 'normal'}
        </p>
        <div className="virtual-controls">
          <button
            disabled={connectionBusy || connectionState === 'disconnected'}
            onClick={() => void connection('disconnect')}
          >
            Disconnect
          </button>
          <button disabled={connectionBusy} onClick={() => void connection('reconnect')}>
            Reconnect
          </button>
          <label>
            Fault mode
            <select
              disabled={connectionBusy}
              value={fault}
              onChange={(event) => setFault(event.target.value)}
            >
              {FAULTS.map((mode) => (
                <option key={mode} value={mode}>
                  {mode}
                </option>
              ))}
            </select>
          </label>
          <button disabled={connectionBusy} onClick={() => void connection(fault)}>
            Apply fault
          </button>
        </div>
        <canvas
          ref={canvas}
          width={240}
          height={240}
          role="img"
          aria-label="Device screen rendered by firmware"
        />
        <div className="virtual-controls">
          <button disabled={!connected} onClick={() => void input('counterclockwise')}>
            Turn left
          </button>
          <button disabled={!connected} onClick={() => void input('clockwise')}>
            Turn right
          </button>
          <button disabled={!connected} onClick={() => void input('tap')}>
            Press once
          </button>
          <button
            disabled={!connected}
            onPointerDown={(event) => {
              if (event.button !== 0) return;
              event.currentTarget.setPointerCapture(event.pointerId);
              press();
            }}
            onPointerUp={release}
            onPointerCancel={release}
            onLostPointerCapture={() => {
              if (held.current !== null) release();
            }}
            onKeyDown={(event) => {
              if (event.key === ' ' || event.key === 'Enter') {
                event.preventDefault();
                press();
              }
            }}
            onKeyUp={(event) => {
              if (event.key === ' ' || event.key === 'Enter') {
                event.preventDefault();
                release();
              }
            }}
            onBlur={() => {
              if (held.current !== null) release();
            }}
          >
            Hold knob
          </button>
          <button disabled={!connected} onClick={() => void input('release')}>
            Release knob
          </button>
        </div>
        <p>
          Press once sends a 120 ms raw press. Hold with a pointer, Space or Enter; release sends
          the button-up edge. Holds automatically release after 2 seconds in this first slice.
        </p>
      </section>
      <section aria-label="Simulated OS">
        <h2>Simulated OS</h2>
        <p>Volume: {snapshot ? `${Math.round(snapshot.volume)}%` : '—'}</p>
        <p>Media triggers: {snapshot?.mediaTriggers ?? '—'}</p>
        <p>Actions affect this simulation only.</p>
      </section>
      {(connectionError || error) && <p role="alert">{connectionError || error}</p>}
    </main>
  );
}
