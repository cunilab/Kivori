import type { ReactElement } from 'react';
import { Pause, Play, Send, StepForward } from 'lucide-react';
import { Button } from '../../components/ui/button';
import { Slider } from '../../components/ui/slider';
import { ToggleGroup, ToggleGroupItem } from '../../components/ui/toggle-group';
import { useConfig } from '../../hooks/use-kivori';
import { mirrorState, playMascotAction } from '../../lib/ipc';
import type { CompanionState, MascotAction, SendableState } from '../../lib/ipc/types';
import {
  COMPANION_STATES,
  INTENSITY_PERSONALITY,
  SENDABLE_STATES,
  MAX_ANIMATION_EVENTS,
} from '../../lib/ipc/types';
import { strings } from '../../lib/i18n/strings';
import { SCENE_DURATION_MS, STEP_MS, useStudioStore } from './store';

const SENDABLE = new Set<CompanionState>(SENDABLE_STATES);
const ACTIONS: readonly MascotAction[] = ['greet', 'pet', 'tickle', 'surprise', 'comfort'];

/** Device Studio control surface: state selection, timeline scrub, transport, and mirror-to-device. */
export function Controls(): ReactElement {
  const personality = INTENSITY_PERSONALITY[useConfig()?.buddy.intensity ?? 'normal'];
  const state = useStudioStore((s) => s.state);
  const elapsedMs = useStudioStore((s) => s.elapsedMs);
  const playing = useStudioStore((s) => s.playing);
  const setState = useStudioStore((s) => s.setState);
  const playAction = useStudioStore((s) => s.playAction);
  const seek = useStudioStore((s) => s.seek);
  const step = useStudioStore((s) => s.step);
  const toggle = useStudioStore((s) => s.toggle);
  const reset = useStudioStore((s) => s.reset);
  const eventLimit = useStudioStore(
    (s) => s.events.filter((e) => e.atMs <= s.elapsedMs).length >= MAX_ANIMATION_EVENTS,
  );

  const t = strings.studio;
  const canMirror = SENDABLE.has(state);

  return (
    <div className="studio-controls space-y-6">
      <fieldset className="space-y-3">
        <legend className="text-sm font-medium">{t.stateGroup}</legend>
        <ToggleGroup
          aria-label={t.stateGroup}
          value={[state]}
          onValueChange={(value) => {
            const nextState = value[0] as CompanionState | undefined;
            if (nextState) setState(nextState);
          }}
          variant="outline"
          disabled={eventLimit}
          className="flex-wrap"
        >
          {COMPANION_STATES.map((candidate) => (
            <ToggleGroupItem key={candidate} value={candidate}>
              {t.states[candidate]}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </fieldset>
      {eventLimit && <p>Restart the preview to record more state changes.</p>}

      <fieldset className="space-y-3">
        <legend className="text-sm font-medium">{strings.companion.actions}</legend>
        <div className="flex flex-wrap gap-2">
          {ACTIONS.map((action) => (
            <Button
              key={action}
              type="button"
              variant="outline"
              disabled={eventLimit}
              onClick={() => {
                playAction(action, personality);
                void playMascotAction(action).catch(() => {});
              }}
            >
              {strings.companion.actionLabels[action]}
            </Button>
          ))}
        </div>
      </fieldset>

      <div className="scrub space-y-2">
        <div className="flex items-center justify-between gap-3 text-sm">
          <span>{t.scrub}</span>
          <output className="font-mono text-muted-foreground">{Math.round(elapsedMs)}</output>
        </div>
        <Slider
          aria-label={t.scrub}
          min={0}
          max={Math.max(
            SCENE_DURATION_MS,
            Math.ceil(elapsedMs / SCENE_DURATION_MS) * SCENE_DURATION_MS,
          )}
          step={STEP_MS}
          value={elapsedMs}
          onValueChange={(value) => seek(Array.isArray(value) ? (value[0] ?? 0) : value)}
        />
      </div>

      <div className="transport flex flex-wrap gap-2" role="group" aria-label={t.transport}>
        <Button type="button" onClick={toggle} aria-pressed={playing}>
          {playing ? <Pause data-icon="inline-start" /> : <Play data-icon="inline-start" />}
          {playing ? t.pause : t.play}
        </Button>
        <Button type="button" variant="outline" onClick={() => step()}>
          <StepForward data-icon="inline-start" />
          {t.step}
        </Button>
        <Button type="button" variant="outline" onClick={reset}>
          Restart preview
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={!canMirror}
          onClick={() => {
            if (canMirror) void mirrorState(state as SendableState);
          }}
        >
          <Send data-icon="inline-start" />
          {t.mirror}
        </Button>
      </div>
    </div>
  );
}
