import { useEffect, useMemo, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '../../components/ui/card';
import { DevicePreview } from '../../lib/canvas/DevicePreview';
import { onConnectionStatus, openPreviewStream, type PreviewStream } from '../../lib/ipc';
import { PREVIEW_FPS } from '../../lib/ipc/types';
import type { AnimationTimeline, ConnectionStatusDto } from '../../lib/ipc/types';
import { strings } from '../../lib/i18n/strings';
import { Page } from '../../components/kivori/page';
import { Controls } from './Controls';
import { TestAction } from './TestAction';
import { useStudioStore } from './store';

/**
 * Dev-only Device Studio: live preview of the shared renderer plus playback controls (US4).
 *
 * Playback uses the native preview-frame stream (contracts/ipc.md §3): the Rust producer renders at a
 * capped FPS and pushes RGBA bytes, which the canvas blits. Paused scrub/step falls back to a per-frame
 * `render_preview_frame` request, which returns the exact frame for an exact millisecond (SC-011).
 */
export function DeviceStudio(): ReactElement {
  const state = useStudioStore((s) => s.state);
  const elapsedMs = useStudioStore((s) => s.elapsedMs);
  const playing = useStudioStore((s) => s.playing);
  const events = useStudioStore((s) => s.events);
  const actionEvents = useStudioStore((s) => s.actionEvents);
  const animation = useMemo<AnimationTimeline>(
    () => ({ initialState: 'idle', events, actionEvents }),
    [events, actionEvents],
  );
  const advance = useStudioStore((s) => s.advance);
  const recordAppliedAction = useStudioStore((s) => s.recordAppliedAction);
  const rafRef = useRef<number | null>(null);
  const [streamFrame, setStreamFrame] = useState<Uint8ClampedArray | null>(null);
  const [streamError, setStreamError] = useState<string | null>(null);
  const requestRef = useRef({ state, animation, elapsedMs });
  requestRef.current = { state, animation, elapsedMs };

  useEffect(() => {
    const apply = (status: ConnectionStatusDto): void => {
      if (status.connection !== 'connected') return;
      const applied = status.mascotAction;
      if (!applied) return;
      recordAppliedAction(applied, status.connectionGeneration);
    };
    let unlisten = (): void => {};
    void onConnectionStatus(apply).then((stop) => {
      unlisten = stop;
    });
    return () => unlisten();
  }, [recordAppliedAction]);

  // Advance the timeline while playing (drift-free integer stepping lives in the store).
  useEffect(() => {
    if (!playing) return;
    const started = performance.now();
    let last = 0;
    const tick = (now: number): void => {
      const elapsed = Math.round(now - started);
      advance(elapsed - last);
      last = elapsed;
      rafRef.current = requestAnimationFrame(tick);
    };
    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
    };
  }, [playing, advance]);

  // One stream per playback session. Coalesce timeline updates so a slow native renderer
  // cannot accumulate requests; the same explicit timestamp drives stream and scrub paths.
  useEffect(() => {
    if (!playing) {
      setStreamFrame(null);
      return;
    }
    let active = true;
    let stream: PreviewStream | null = null;
    let updating = false;
    setStreamError(null);
    const current = requestRef.current;
    void openPreviewStream(
      current.state,
      PREVIEW_FPS,
      (frame) => {
        if (active) setStreamFrame(frame);
      },
      current.animation,
      current.elapsedMs,
    )
      .then((opened) => {
        if (active) stream = opened;
        else void opened.close().catch(() => {});
      })
      .catch((error: unknown) => {
        if (active) {
          setStreamFrame(null);
          setStreamError(String(error));
        }
      });
    const updates = setInterval(() => {
      if (!stream?.update || updating) return;
      updating = true;
      const request = requestRef.current;
      void stream
        .update(request.animation, request.elapsedMs)
        .catch((error: unknown) => {
          if (active) {
            setStreamFrame(null);
            setStreamError(String(error));
          }
        })
        .finally(() => {
          updating = false;
        });
    }, 1000 / PREVIEW_FPS);
    return () => {
      active = false;
      clearInterval(updates);
      setStreamFrame(null);
      void stream?.close().catch(() => {});
    };
  }, [playing]);

  const t = strings.studio;
  return (
    <Page title={t.heading} description={t.description}>
      <section aria-label={t.heading} className="device-studio space-y-4">
        <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(20rem,0.8fr)]">
          <Card>
            <CardHeader>
              <CardTitle>{t.preview}</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col items-center gap-3">
              <DevicePreview
                state={state}
                elapsedMs={elapsedMs}
                label={t.preview}
                frame={playing && !streamError ? streamFrame : null}
                animation={animation}
                streaming={playing && !streamError}
              />
              {streamError && <p role="alert">{streamError}</p>}
            </CardContent>
          </Card>
          <div className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>{t.controls}</CardTitle>
              </CardHeader>
              <CardContent>
                <Controls />
              </CardContent>
            </Card>
            <Card>
              <CardContent>
                <TestAction />
              </CardContent>
            </Card>
          </div>
        </div>
      </section>
    </Page>
  );
}
