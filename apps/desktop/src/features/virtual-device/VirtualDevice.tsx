import { useEffect, useRef, useState, type ReactElement } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { blitRgba } from '../../lib/canvas/blit';
import './virtual-device.css';

type Snapshot = {
  connected: boolean;
  volume: number;
  mediaTriggers: number;
  frame: number[] | null;
  error: string | null;
};
type Control = 'clockwise' | 'counterclockwise' | 'press' | 'release' | 'tap';

export function VirtualDevice(): ReactElement {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const held = useRef(false);

  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    let watchdog: ReturnType<typeof setTimeout>;
    async function poll() {
      watchdog = setTimeout(() => {
        if (!stopped) {
          setSnapshot(null);
          setError('Virtual device is not responding.');
        }
      }, 3000);
      try {
        const next = await invoke<Snapshot>('virtual_snapshot');
        if (!stopped) {
          setSnapshot(next);
          setError(next.error);
        }
      } catch (cause) {
        if (!stopped) {
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

  useEffect(() => {
    if (!snapshot?.frame || snapshot.frame.length !== 240 * 240) return;
    const ctx = canvas.current?.getContext('2d');
    if (!ctx) return;
    const rgba = new Uint8ClampedArray(240 * 240 * 4);
    snapshot.frame.forEach((pixel, index) => {
      rgba[index * 4] = Math.round((((pixel >> 11) & 31) * 255) / 31);
      rgba[index * 4 + 1] = Math.round((((pixel >> 5) & 63) * 255) / 63);
      rgba[index * 4 + 2] = Math.round(((pixel & 31) * 255) / 31);
      rgba[index * 4 + 3] = 255;
    });
    blitRgba(ctx, rgba, 240);
  }, [snapshot]);

  async function input(control: Control) {
    try {
      await invoke('virtual_input', { control });
    } catch (cause) {
      setSnapshot(null);
      setError(String(cause));
    }
  }
  function press() {
    if (held.current) return;
    held.current = true;
    void input('press');
  }
  function release() {
    held.current = false;
    void input('release');
  }
  useEffect(() => {
    const onBlur = () => {
      if (held.current) release();
    };
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('blur', onBlur);
      if (held.current) void invoke('virtual_input', { control: 'release' }).catch(() => {});
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
          {error ? 'Unavailable' : snapshot?.connected ? 'Connected' : 'Connecting…'}
        </p>
        <canvas
          ref={canvas}
          width={240}
          height={240}
          role="img"
          aria-label="Device screen rendered by firmware"
        />
        <div className="virtual-controls">
          <button onClick={() => void input('counterclockwise')}>Turn left</button>
          <button onClick={() => void input('clockwise')}>Turn right</button>
          <button onClick={() => void input('tap')}>Press once</button>
          <button
            onPointerDown={(event) => {
              if (event.button !== 0) return;
              event.currentTarget.setPointerCapture(event.pointerId);
              press();
            }}
            onPointerUp={release}
            onPointerCancel={release}
            onLostPointerCapture={() => {
              if (held.current) release();
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
              if (held.current) release();
            }}
          >
            Hold knob
          </button>
          <button onClick={release}>Release knob</button>
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
      {error && <p role="alert">{error}</p>}
    </main>
  );
}
