import type { ReactElement, ReactNode } from 'react';
import { toast } from 'sonner';
import {
  Cable,
  Cpu,
  Hand,
  LayoutGrid,
  LoaderCircle,
  MemoryStick,
  Music2,
  Pause,
  Play,
  ShieldAlert,
  Square,
  TriangleAlert,
  Usb,
  Volume2,
  VolumeX,
  Zap,
} from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardAction, CardContent, CardDescription, CardHeader } from '@/components/ui/card';
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { Progress } from '@/components/ui/progress';
import { Skeleton } from '@/components/ui/skeleton';
import { DeviceArt } from '@/components/kivori/device-art';
import { Known } from '@/components/kivori/known';
import { Page } from '@/components/kivori/page';
import { ResultBadge } from '@/components/kivori/result-badge';
import { uiConnection } from '@/hooks/use-kivori';
import { playMascotAction } from '@/lib/ipc';
import type { ConnectionStatusDto, DeskStatusDto } from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { cn, errorText } from '@/lib/utils';

export type NavigateTo = (page: 'display' | 'device') => void;

const t = strings.home;

function Hero({
  connection,
  desk,
  onNavigate,
}: {
  connection: ConnectionStatusDto | null;
  desk: DeskStatusDto | null;
  onNavigate: NavigateTo;
}): ReactElement {
  const ui = uiConnection(connection);
  if (ui === 'loading' || !desk) {
    return (
      <Card className="p-6">
        <div className="grid items-center gap-6 md:grid-cols-[minmax(0,18rem)_1fr]">
          <Skeleton className="aspect-[1.45] w-full rounded-[16%]" />
          <div className="space-y-3">
            <Skeleton className="h-7 w-48" />
            <Skeleton className="h-4 w-72" />
            <Skeleton className="h-8 w-40" />
          </div>
        </div>
      </Card>
    );
  }

  if (ui === 'disconnected') {
    return (
      <Card>
        <Empty className="py-10">
          <EmptyHeader>
            <EmptyMedia>
              <div className="relative flex items-center gap-3 text-muted-foreground">
                <div className="flex size-14 items-center justify-center rounded-2xl bg-muted">
                  <Usb className="size-7" aria-hidden="true" />
                </div>
                <Cable className="size-5 opacity-60" aria-hidden="true" />
                <div className="flex size-14 items-center justify-center rounded-2xl bg-panel text-panel-blue shadow-lg">
                  <span className="flex gap-1.5">
                    <span className="h-4 w-2 rounded-full bg-current" />
                    <span className="h-4 w-2 rounded-full bg-current" />
                  </span>
                </div>
              </div>
            </EmptyMedia>
            <EmptyTitle className="text-xl">{t.empty.title}</EmptyTitle>
            <EmptyDescription>{t.empty.body}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent className="max-w-md">
            <ol className="w-full space-y-2 text-left text-sm">
              {t.empty.steps.map((step, index) => (
                <li key={step} className="flex items-start gap-3">
                  <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                    {index + 1}
                  </span>
                  <span className="pt-0.5">{step}</span>
                </li>
              ))}
            </ol>
            <p className="flex items-center gap-2 text-xs text-muted-foreground" role="status">
              <LoaderCircle className="size-3.5 animate-spin" aria-hidden="true" />
              {t.empty.searching}
            </p>
          </EmptyContent>
        </Empty>
      </Card>
    );
  }

  const busy = ui === 'connecting' || ui === 'reconnecting';
  const title = {
    connected: t.hero.connected,
    connecting: t.hero.connecting,
    reconnecting: t.hero.reconnecting,
    incompatible: t.hero.incompatible,
  }[ui];
  const body = {
    connected: t.hero.connectedBody,
    connecting: t.hero.connectingBody,
    reconnecting: t.hero.reconnectingBody,
    incompatible: connection?.incompatibleReason ?? t.hero.incompatibleBody,
  }[ui];
  const canGreet = ui === 'connected' && connection?.mascotInteraction === true;
  const modeName = strings.display.modes[desk.mode].name;

  return (
    <Card className="relative overflow-hidden p-6">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -top-24 -right-24 size-72 rounded-full bg-primary/10 blur-3xl"
      />
      <div className="relative grid items-center gap-6 md:grid-cols-[minmax(0,18rem)_1fr] lg:gap-10">
        <DeviceArt
          mode={desk.mode}
          values={desk}
          busy={busy}
          label={format(t.hero.illustration, { mode: modeName })}
          className={cn(
            'w-full max-w-72 transition-opacity',
            ui === 'incompatible' && 'opacity-60',
          )}
        />
        <div className="space-y-4">
          <div className="space-y-1.5">
            <h2 className="flex items-center gap-2 text-xl font-semibold tracking-tight">
              {ui === 'incompatible' ? (
                <TriangleAlert className="size-5 text-destructive" aria-hidden="true" />
              ) : busy ? (
                <LoaderCircle className="size-5 animate-spin text-warning" aria-hidden="true" />
              ) : null}
              {title}
            </h2>
            <p className="max-w-prose text-sm text-muted-foreground">{body}</p>
          </div>
          <dl className="flex flex-wrap gap-2 text-xs">
            <Chip label={t.hero.showing}>{modeName}</Chip>
            <Chip label={t.hero.firmware}>
              <Known value={connection?.device?.firmwareVersion} why={strings.why.firmware}>
                {(version) => version}
              </Known>
            </Chip>
            <Chip label={t.hero.buddy}>
              <Known value={connection?.reported} why={strings.why.buddy}>
                {(state) => strings.studio.states[state]}
              </Known>
            </Chip>
          </dl>
          <div className="flex flex-wrap gap-2">
            {ui === 'incompatible' ? (
              <Button onClick={() => onNavigate('device')}>
                <Zap data-icon="inline-start" aria-hidden="true" />
                {t.hero.openDevice}
              </Button>
            ) : (
              <Button onClick={() => onNavigate('display')}>
                <LayoutGrid data-icon="inline-start" aria-hidden="true" />
                {t.hero.changeView}
              </Button>
            )}
            {canGreet ? (
              <Button
                variant="outline"
                onClick={() =>
                  void playMascotAction('greet').catch((error: unknown) =>
                    toast.error(strings.companion.failed, { description: errorText(error) }),
                  )
                }
              >
                <Hand data-icon="inline-start" aria-hidden="true" />
                {t.hero.sayHi}
              </Button>
            ) : null}
          </div>
        </div>
      </div>
    </Card>
  );
}

function Chip({ label, children }: { label: string; children: ReactNode }): ReactElement {
  return (
    <div className="flex items-center gap-1.5 rounded-full bg-muted px-3 py-1">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-medium">{children}</dd>
    </div>
  );
}

function Metric({
  icon,
  label,
  value,
  why,
  testId,
  badge,
  hint,
  danger,
}: {
  icon: ReactNode;
  label: string;
  value: number | null | undefined;
  why: string;
  testId: string;
  badge?: ReactNode;
  hint?: ReactNode;
  danger?: boolean;
}): ReactElement {
  return (
    <Card size="sm" className="gap-3">
      <CardHeader>
        <CardDescription className="flex items-center gap-2 text-sm">
          {icon}
          {label}
        </CardDescription>
        {badge ? <CardAction>{badge}</CardAction> : null}
      </CardHeader>
      <CardContent className="space-y-3">
        <p className="text-3xl font-semibold tracking-tight tabular-nums" data-testid={testId}>
          <Known value={value} why={why}>
            {(known) => `${known}%`}
          </Known>
        </p>
        {value === null || value === undefined ? (
          // Unknown is not 0: an empty dashed track, no fill and no indeterminate animation.
          <div
            aria-hidden="true"
            className="h-1.5 rounded-full border border-dashed border-border"
          />
        ) : (
          <Progress
            value={value}
            aria-label={label}
            className={cn(
              '[&_[data-slot=progress-track]]:h-1.5',
              danger && '[&_[data-slot=progress-indicator]]:bg-destructive',
            )}
          />
        )}
        {hint ? <p className="text-xs text-muted-foreground">{hint}</p> : null}
      </CardContent>
    </Card>
  );
}

function NowPlaying({ desk }: { desk: DeskStatusDto }): ReactElement {
  const StateIcon = desk.media === 'playing' ? Play : desk.media === 'paused' ? Pause : Square;
  return (
    <Card size="sm">
      <CardHeader>
        <CardDescription className="flex items-center gap-2 text-sm">
          <Music2 className="size-4" aria-hidden="true" />
          {t.nowPlaying}
        </CardDescription>
        <CardAction data-testid="desk-media">
          <Known value={desk.media} why={strings.why.media}>
            {(media) => (
              <Badge variant="secondary" className="h-6 gap-1.5 px-2.5">
                <StateIcon data-icon="inline-start" className="fill-current" aria-hidden="true" />
                {t.mediaStates[media]}
              </Badge>
            )}
          </Known>
        </CardAction>
      </CardHeader>
      <CardContent className="flex items-center gap-4">
        <div className="flex size-14 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-panel-amber to-[#e0702a] text-panel shadow-sm">
          <Music2 className="size-6" aria-hidden="true" />
        </div>
        <div className="min-w-0 space-y-0.5">
          <p className="truncate font-medium" data-testid="desk-title">
            <Known value={desk.mediaTitle} why={strings.why.mediaText}>
              {(title) => title || t.noTitle}
            </Known>
          </p>
          <p className="truncate text-sm text-muted-foreground" data-testid="desk-artist">
            <Known value={desk.mediaArtist} why={strings.why.mediaText}>
              {(artist) => artist || t.noArtist}
            </Known>
          </p>
        </div>
      </CardContent>
    </Card>
  );
}

function LastAction({ desk }: { desk: DeskStatusDto }): ReactElement {
  const last = desk.lastAction;
  return (
    <Card size="sm">
      <CardHeader>
        <CardDescription className="flex items-center gap-2 text-sm">
          <Zap className="size-4" aria-hidden="true" />
          {t.lastAction}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-3" data-testid="last-action">
        {last ? (
          <>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-medium">{strings.actions[last.action]}</span>
              <ResultBadge result={last.result} />
            </div>
            <p className="text-sm text-muted-foreground">{strings.resultHelp[last.result]}</p>
            {last.permissionRequired ? (
              <Alert className="border-warning/40 bg-warning/5">
                <ShieldAlert className="text-warning" aria-hidden="true" />
                <AlertTitle>{t.permissionTitle}</AlertTitle>
                <AlertDescription>{t.permission}</AlertDescription>
              </Alert>
            ) : null}
          </>
        ) : (
          <p className="text-sm text-muted-foreground">{t.noLastAction}</p>
        )}
      </CardContent>
    </Card>
  );
}

/** Home: the device at a glance, live monitoring, and the last action's honest outcome. */
export function HomePage({
  connection,
  desk,
  onNavigate,
}: {
  connection: ConnectionStatusDto | null;
  desk: DeskStatusDto | null;
  onNavigate: NavigateTo;
}): ReactElement {
  return (
    <Page title={t.title} description={t.description}>
      <Hero connection={connection} desk={desk} onNavigate={onNavigate} />

      <section aria-labelledby="monitoring-heading" className="space-y-3">
        <div className="flex flex-wrap items-baseline justify-between gap-2">
          <h2 id="monitoring-heading" className="text-base font-semibold">
            {t.monitoring}
          </h2>
          <p className="text-xs text-muted-foreground">{t.monitoringNote}</p>
        </div>
        {desk ? (
          <>
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              <Metric
                icon={<Cpu className="size-4" aria-hidden="true" />}
                label={t.cpu}
                value={desk.cpuPercent}
                why={strings.why.cpu}
                testId="desk-cpu"
                danger={desk.highLoad}
                badge={
                  desk.highLoad ? (
                    <Badge variant="destructive" className="h-6 px-2.5">
                      {t.highLoad}
                    </Badge>
                  ) : null
                }
              />
              <Metric
                icon={<MemoryStick className="size-4" aria-hidden="true" />}
                label={t.ram}
                value={desk.ramPercent}
                why={strings.why.ram}
                testId="desk-ram"
              />
              <Metric
                icon={
                  desk.muted ? (
                    <VolumeX className="size-4" aria-hidden="true" />
                  ) : (
                    <Volume2 className="size-4" aria-hidden="true" />
                  )
                }
                label={t.volume}
                value={desk.volumePercent}
                why={strings.why.volume}
                testId="desk-volume"
                badge={
                  desk.muted === true ? (
                    <Badge variant="secondary" className="h-6 gap-1 px-2.5">
                      <VolumeX data-icon="inline-start" aria-hidden="true" />
                      {t.muted}
                    </Badge>
                  ) : null
                }
                hint={
                  <span data-testid="desk-muted">
                    <Known value={desk.muted} why={strings.why.muted}>
                      {(muted) => (muted ? t.muted : t.notMuted)}
                    </Known>
                  </span>
                }
              />
            </div>
            <div className="grid gap-4 lg:grid-cols-2">
              <NowPlaying desk={desk} />
              <LastAction desk={desk} />
            </div>
          </>
        ) : (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {[0, 1, 2].map((index) => (
              <Skeleton key={index} className="h-36 rounded-xl" />
            ))}
          </div>
        )}
      </section>
    </Page>
  );
}
