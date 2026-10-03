import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { TriangleAlert } from 'lucide-react';
import { Alert, AlertAction, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { useDevMode } from '@/lib/dev-mode';
import { configureCompanion, playMascotAction } from '@/lib/ipc';
import type { MascotAction, MascotPersonality } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';

const PERSONALITY_KEY = 'kivori.mascot.personality';
const SELF_PLAY_KEY = 'kivori.mascot.selfPlay';
const PERSONALITIES: readonly MascotPersonality[] = ['cozy', 'playful', 'calm'];
const ACTIONS: readonly MascotAction[] = ['greet', 'pet', 'tickle', 'surprise', 'comfort'];

export function currentMascotPersonality(): MascotPersonality {
  if (typeof localStorage === 'undefined') return 'cozy';
  const saved = localStorage.getItem(PERSONALITY_KEY);
  return PERSONALITIES.includes(saved as MascotPersonality) ? (saved as MascotPersonality) : 'cozy';
}

function initialSelfPlay(): boolean {
  return localStorage.getItem(SELF_PLAY_KEY) !== 'false';
}

interface CompanionControlsProps {
  connected: boolean;
  supported: boolean;
}

/** Production companion controls for personality, ambient self-play, and direct social reactions. */
export function CompanionControls({ connected, supported }: CompanionControlsProps): ReactElement {
  const [personality, setPersonality] = useState<MascotPersonality>(currentMascotPersonality);
  const [selfPlay, setSelfPlay] = useState(initialSelfPlay);
  const [configurationError, setConfigurationError] = useState(false);
  const configurationAttempt = useRef(0);
  const t = strings.companion;
  const devMode = useDevMode();

  const applyConfiguration = useCallback(
    (nextPersonality: MascotPersonality, nextSelfPlay: boolean): void => {
      const attempt = ++configurationAttempt.current;
      setConfigurationError(false);
      void configureCompanion(nextPersonality, nextSelfPlay).catch(() => {
        if (configurationAttempt.current === attempt) setConfigurationError(true);
      });
    },
    [],
  );

  useEffect(() => {
    localStorage.setItem(PERSONALITY_KEY, personality);
    localStorage.setItem(SELF_PLAY_KEY, String(selfPlay));
    applyConfiguration(personality, selfPlay);
  }, [personality, selfPlay, applyConfiguration]);

  const available = connected && supported;

  return (
    <Card>
      <CardHeader>
        <CardTitle id="companion-heading" role="heading" aria-level={2}>
          {t.heading}
        </CardTitle>
        <CardDescription>{t.description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        <fieldset className="space-y-2">
          <legend className="mb-2 text-sm font-medium">{t.personality}</legend>
          <ToggleGroup
            aria-label={t.personality}
            value={[personality]}
            onValueChange={(value) => {
              const selected = value[0] as MascotPersonality | undefined;
              if (selected) setPersonality(selected);
            }}
            variant="outline"
            className="flex-wrap"
          >
            {PERSONALITIES.map((candidate) => (
              <ToggleGroupItem key={candidate} value={candidate} className="px-4">
                {t.personalities[candidate]}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </fieldset>

        {/* Self-play and the direct reactions are developer tools; personality is for everyone. */}
        {devMode ? (
          <>
            <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
              <div className="space-y-0.5">
                <Label htmlFor="self-play">{t.selfPlay}</Label>
                <p className="text-xs text-muted-foreground">{t.selfPlayHint}</p>
              </div>
              <Switch id="self-play" checked={selfPlay} onCheckedChange={setSelfPlay} />
            </div>

            <fieldset className="space-y-2">
              <legend className="mb-2 text-sm font-medium">{t.actions}</legend>
              <div className="flex flex-wrap gap-2">
                {ACTIONS.map((action) => (
                  <Button
                    key={action}
                    type="button"
                    variant="secondary"
                    disabled={!available}
                    onClick={() => {
                      void playMascotAction(action).catch((error: unknown) =>
                        toast.error(t.failed, { description: errorText(error) }),
                      );
                    }}
                  >
                    {t.actionLabels[action]}
                  </Button>
                ))}
              </div>
              {!connected ? <p className="text-xs text-muted-foreground">{t.connect}</p> : null}
              {connected && !supported ? (
                <p className="text-xs text-muted-foreground">{t.update}</p>
              ) : null}
            </fieldset>
          </>
        ) : null}

        {configurationError ? (
          <Alert variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertTitle>{t.configureFailed}</AlertTitle>
            <AlertAction>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => applyConfiguration(personality, selfPlay)}
              >
                {t.retry}
              </Button>
            </AlertAction>
          </Alert>
        ) : null}
      </CardContent>
    </Card>
  );
}
