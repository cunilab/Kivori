import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { TriangleAlert } from 'lucide-react';
import { Alert, AlertAction, AlertTitle } from '@kivori/ui/components/alert';
import { Button } from '@kivori/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import { Label } from '@kivori/ui/components/label';
import { Switch } from '@kivori/ui/components/switch';
import { ToggleGroup, ToggleGroupItem } from '@kivori/ui/components/toggle-group';
import { useDevMode } from '@/lib/dev-mode';
import { playMascotAction, setBuddySettings } from '@/lib/ipc';
import { INTENSITIES } from '@/lib/ipc/types';
import type { ConfigDto, Intensity, MascotAction } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';

const ACTIONS: readonly MascotAction[] = ['greet', 'pet', 'tickle', 'surprise', 'comfort'];

type BuddySettings = ConfigDto['buddy'];

interface CompanionControlsProps {
  config: ConfigDto | null;
  connected: boolean;
  supported: boolean;
}

/**
 * The Buddy card: reactions and intensity are saved settings for everyone; the direct social
 * reactions are a developer tool. A change shows at once (UI only) and is dropped as soon as the
 * saved config arrives, or when the save fails.
 */
export function CompanionControls({
  config,
  connected,
  supported,
}: CompanionControlsProps): ReactElement {
  const [pending, setPending] = useState<BuddySettings | null>(null);
  const [failed, setFailed] = useState<BuddySettings | null>(null);
  const t = strings.companion;
  const devMode = useDevMode();

  // The saved config arrived: it is the truth again.
  const revision = config?.revision;
  useEffect(() => setPending(null), [revision]);

  const save = (next: BuddySettings): void => {
    setPending(next);
    setFailed(null);
    void setBuddySettings(next.reactions, next.intensity).catch(() => {
      setPending(null);
      setFailed(next);
    });
  };

  const shown = pending ?? config?.buddy ?? null;
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
        <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
          <div className="space-y-0.5">
            <Label htmlFor="buddy-reactions">{t.reactions}</Label>
            <p className="text-xs text-muted-foreground">{t.reactionsHint}</p>
          </div>
          <Switch
            id="buddy-reactions"
            checked={shown?.reactions ?? false}
            disabled={shown === null}
            onCheckedChange={(reactions) => shown && save({ ...shown, reactions })}
          />
        </div>

        <fieldset className="space-y-2">
          <legend className="mb-2 text-sm font-medium">{t.intensity}</legend>
          <ToggleGroup
            aria-label={t.intensity}
            value={shown ? [shown.intensity] : []}
            disabled={shown === null}
            onValueChange={(value) => {
              const selected = value[0] as Intensity | undefined;
              if (selected && shown) save({ ...shown, intensity: selected });
            }}
            variant="outline"
            className="flex-wrap"
          >
            {INTENSITIES.map((candidate) => (
              <ToggleGroupItem key={candidate} value={candidate} className="px-4">
                {t.intensities[candidate]}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </fieldset>

        {/* Direct reactions are a developer tool. */}
        {devMode ? (
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
        ) : null}

        {failed ? (
          <Alert variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertTitle>{t.configureFailed}</AlertTitle>
            <AlertAction>
              <Button type="button" size="sm" variant="outline" onClick={() => save(failed)}>
                {t.retry}
              </Button>
            </AlertAction>
          </Alert>
        ) : null}
      </CardContent>
    </Card>
  );
}
