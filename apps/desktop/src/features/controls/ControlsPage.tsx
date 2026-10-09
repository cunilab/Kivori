import type { ReactElement } from 'react';
import {
  AppWindow,
  CircleDot,
  Pin,
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
import type { DeskActionToken, DeskStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';

const t = strings.controls;

interface Gesture {
  key: keyof typeof t.gestures;
  Icon: LucideIcon;
  action: string | null;
}

// A shortcut or launch is named by the profile ("Reload"); an empty label means it is suspended.
function bound(token: DeskActionToken | null | undefined, label: string): string {
  if (!token) return t.unbound;
  if (token === 'shortcut' || token === 'launch') return label || t.suspended;
  return strings.actions[token];
}

// A Hold has no device label, so a shortcut or launch is named by its kind.
function held(token: DeskActionToken | null | undefined): string {
  return token ? strings.actions[token] : t.unbound;
}

/** Controls: what each control does under the active profile. Read-only; bindings come from the desk status. */
export function ControlsPage({ desk }: { desk: DeskStatusDto | null }): ReactElement {
  const rotate = desk && (desk.rotateLabel === 'Volume' ? t.systemVolume : desk.rotateLabel);
  const gestures: Gesture[] = [
    { key: 'rotate', Icon: RotateCw, action: desk && (rotate || t.suspended) },
    { key: 'press', Icon: MousePointerClick, action: desk && held(desk.pressAction) },
    {
      key: 'doublePress',
      Icon: RefreshCw,
      action: desk && strings.actions[desk.doublePressAction],
    },
    { key: 'hold', Icon: Hand, action: desk && held(desk.holdAction) },
    ...(['button1', 'button2', 'button3'] as const).map((key, i) => {
      return {
        key,
        Icon: CircleDot,
        action: desk && bound(desk.buttonActions[i], desk.buttonLabels[i]),
      };
    }),
    ...(['button1Hold', 'button2Hold', 'button3Hold'] as const).map((key, i) => {
      return {
        key,
        Icon: Hand,
        // The middle button's Hold pins a profile; it can never be rebound.
        action: desk && (i === 1 ? t.pinProfile : held(desk.buttonHoldActions[i])),
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
      <Card data-testid="active-profile">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <AppWindow className="size-5" aria-hidden="true" />
            </div>
            <div className="min-w-0 flex-1">
              <CardDescription>{t.profile}</CardDescription>
              {desk ? (
                <CardTitle>{desk.profile ?? t.profileGeneral}</CardTitle>
              ) : (
                <Skeleton className="h-5 w-28" />
              )}
            </div>
            {desk?.pinned && (
              <Badge variant="secondary" className="h-6 gap-1.5 px-2.5">
                <Pin data-icon="inline-start" aria-hidden="true" />
                {t.pinned}
              </Badge>
            )}
          </div>
          <CardDescription>{desk?.pinned ? t.pinnedHint : t.followHint}</CardDescription>
        </CardHeader>
      </Card>

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
