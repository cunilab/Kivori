import type { ReactElement } from 'react';
import { toast } from 'sonner';
import { Button } from '@kivori/ui/components/button';
import { Card, CardContent, CardHeader, CardTitle } from '@kivori/ui/components/card';
import { hasCapability } from '@/lib/device-support';
import { useDevMode } from '@/lib/dev-mode';
import { playMascotAction } from '@/lib/ipc';
import type { ConnectionStatusDto, MascotAction } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';

const ACTIONS: readonly MascotAction[] = ['greet', 'pet', 'tickle', 'surprise', 'comfort'];

export function ReactionControls({
  connection,
}: {
  connection: ConnectionStatusDto | null;
}): ReactElement {
  const enabled = useDevMode() && hasCapability(connection, 'mascotInteraction');
  const t = strings.companion;
  return (
    <Card>
      <CardHeader>
        <CardTitle role="heading" aria-level={2}>
          {t.actions}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {enabled ? (
          <div className="flex flex-wrap gap-2">
            {ACTIONS.map((action) => (
              <Button
                key={action}
                variant="secondary"
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
        ) : (
          <p className="text-sm text-muted-foreground">{strings.developer.reactionsUnavailable}</p>
        )}
      </CardContent>
    </Card>
  );
}
