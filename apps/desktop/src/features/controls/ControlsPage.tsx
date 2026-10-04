import type { ReactElement } from 'react';
import {
  CircleDot,
  Hand,
  MousePointerClick,
  RefreshCw,
  RotateCw,
  Sparkles,
  TimerReset,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Item, ItemContent, ItemDescription, ItemGroup, ItemMedia } from '@/components/ui/item';
import { Skeleton } from '@/components/ui/skeleton';
import { Page } from '@/components/kivori/page';
import { ResultBadge } from '@/components/kivori/result-badge';
import { DESK_RESULTS } from '@/lib/ipc/types';
import type { DeskStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';

const t = strings.controls;

interface Gesture {
  key: keyof typeof t.gestures;
  Icon: LucideIcon;
  action: string | null;
}

/** Controls: what each knob gesture and contextual button does. Read-only in M1, bindings come from the desk status. */
export function ControlsPage({ desk }: { desk: DeskStatusDto | null }): ReactElement {
  const gestures: Gesture[] = [
    // The rotate binding is not part of the desk DTO; in M1 the knob always drives system volume.
    { key: 'rotate', Icon: RotateCw, action: t.systemVolume },
    { key: 'press', Icon: MousePointerClick, action: desk && strings.actions[desk.pressAction] },
    {
      key: 'doublePress',
      Icon: RefreshCw,
      action: desk && strings.actions[desk.doublePressAction],
    },
    { key: 'hold', Icon: Hand, action: desk && strings.actions[desk.holdAction] },
    ...(['button1', 'button2', 'button3'] as const).map((key, i) => {
      const token = desk?.buttonActions[i];
      return {
        key,
        Icon: CircleDot,
        action: desk && (token ? strings.actions[token] : t.unbound),
      };
    }),
  ];

  return (
    <Page
      title={t.title}
      description={t.description}
      actions={
        <Badge variant="secondary" className="h-6 gap-1.5 px-2.5">
          <Sparkles data-icon="inline-start" aria-hidden="true" />
          {t.soon}
        </Badge>
      }
    >
      <ul className="grid gap-4 sm:grid-cols-2" aria-label={t.title}>
        {gestures.map(({ key, Icon, action }) => (
          <li key={key}>
            <Card
              className="h-full transition-shadow hover:shadow-md"
              data-testid={`gesture-${key}`}
            >
              <CardHeader>
                <div className="flex items-center gap-3">
                  <div className="flex size-10 items-center justify-center rounded-xl bg-primary/10 text-primary">
                    <Icon className="size-5" aria-hidden="true" />
                  </div>
                  <div>
                    <CardTitle>{t.gestures[key].name}</CardTitle>
                    <CardDescription>{t.gestures[key].hint}</CardDescription>
                  </div>
                </div>
              </CardHeader>
              <CardContent>
                <div className="flex items-center justify-between rounded-lg bg-muted px-3 py-2.5">
                  {action ? (
                    <span className="font-medium">{action}</span>
                  ) : (
                    <Skeleton className="h-5 w-28" />
                  )}
                  <span className="text-xs text-muted-foreground">{t.readOnly}</span>
                </div>
              </CardContent>
            </Card>
          </li>
        ))}
      </ul>

      <Alert>
        <TimerReset aria-hidden="true" />
        <AlertTitle>{t.recoveryTitle}</AlertTitle>
        <AlertDescription>{t.recovery}</AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>{t.resultsTitle}</CardTitle>
          <CardDescription>{t.resultsBody}</CardDescription>
        </CardHeader>
        <CardContent>
          <ItemGroup className="gap-1">
            {DESK_RESULTS.map((result) => (
              <Item key={result} size="sm" className="px-0">
                <ItemMedia className="w-44 shrink-0 justify-start">
                  <ResultBadge result={result} />
                </ItemMedia>
                <ItemContent>
                  <ItemDescription>{strings.resultHelp[result]}</ItemDescription>
                </ItemContent>
              </Item>
            ))}
          </ItemGroup>
        </CardContent>
      </Card>
    </Page>
  );
}
