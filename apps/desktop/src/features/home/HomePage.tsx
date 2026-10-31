import type { ReactElement, ReactNode } from 'react';
import { openPageTab } from '@/components/kivori/page-tabs';
import {
  Cpu,
  LayoutGrid,
  Monitor,
  LoaderCircle,
  MemoryStick,
  Mic,
  MicOff,
  Music2,
  Pause,
  Play,
  Square,
  TriangleAlert,
  Volume2,
  VolumeX,
  Zap,
} from 'lucide-react';
import { Badge } from '@kivori/ui/components/badge';
import { Button } from '@kivori/ui/components/button';
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
} from '@kivori/ui/components/card';
import { Progress } from '@kivori/ui/components/progress';
import { Skeleton } from '@kivori/ui/components/skeleton';
import { ProductArt } from '@/components/kivori/product-art';
import { IconChip } from '@/components/kivori/icon-chip';
import { Known } from '@/components/kivori/known';
import { Page } from '@/components/kivori/page';
import { ResultBadge } from '@/components/kivori/result-badge';
import { uiConnection } from '@/hooks/use-kivori';
import { hasCapability, canMirrorState } from '@/lib/device-support';
import type { ConnectionStatusDto, DeskStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { useDevMode } from '@/lib/dev-mode';
import { AccessibilityHelp } from '../permissions/AccessibilityHelp';
import { LinkHealth } from './LinkHealth';
import { TONE_BAR, TONE_TEXT, meterTone } from './meter-tone';
import { cn } from '@kivori/ui/lib/utils';

export type NavigateTo = (page: 'display' | 'device') => void;

const t = strings.home;

function Hero({
  connection,
  desk,
  updateAvailable,
  onNavigate,
}: {
  connection: ConnectionStatusDto | null;
  desk: DeskStatusDto | null;
  updateAvailable: boolean;
  onNavigate: NavigateTo;
}): ReactElement {
  const ui = uiConnection(connection);
  const display = hasCapability(connection, 'deskStatusV1');
  if (ui === 'loading' || !desk) {
    return (
      <Card className="p-6">
        <div className="grid items-center gap-6 md:grid-cols-[minmax(0,18rem)_1fr]">
          <Skeleton className="aspect-square w-full rounded-[16%]" />
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
      <Card className="gap-0 py-0">
        <div
          className="flex items-center gap-2 border-b bg-gradient-to-r from-primary/15 via-primary/5 to-transparent px-5 py-2.5 text-xs text-muted-foreground"
          role="status"
        >
          <LoaderCircle className="size-3.5 animate-spin" aria-hidden="true" />
          {t.empty.searching}
        </div>
        <div className="grid items-center gap-5 p-5 sm:grid-cols-[minmax(0,11rem)_1fr] sm:gap-8 sm:p-6">
          <ProductArt connection={connection} className="mx-auto max-w-28 sm:max-w-none" />
          <div className="space-y-4">
            <div className="space-y-1">
              <h2 className="text-xl font-semibold tracking-tight">{t.empty.title}</h2>
              <p className="text-sm text-muted-foreground">{t.empty.body}</p>
            </div>
            <ol className="space-y-2 text-sm">
              {t.empty.steps.map((step, index) => (
                <li key={step} className="flex items-start gap-3">
                  <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                    {index + 1}
                  </span>
                  <span className="pt-0.5">{step}</span>
                </li>
              ))}
            </ol>
          </div>
        </div>
      </Card>
    );
  }

  const pendingIdentity = ui === 'connected' && !connection?.device?.identity;
  const unknownIdentity = ui === 'connected' && connection?.device?.identity?.known === false;
  const recovery =
    ui === 'connected' &&
    connection?.device?.identity?.known === true &&
    !canMirrorState(connection);
  const busy = ui === 'connecting' || ui === 'reconnecting' || pendingIdentity;
  const title = pendingIdentity
    ? t.hero.pending
    : unknownIdentity
      ? t.hero.unknown
      : recovery
        ? t.hero.recovery
        : {
            connected: t.hero.connected,
            connecting: t.hero.connecting,
            reconnecting: t.hero.reconnecting,
            incompatible: t.hero.incompatible,
          }[ui];
  const body = pendingIdentity
    ? t.hero.pendingBody
    : unknownIdentity
      ? t.hero.unknownBody
      : recovery
        ? t.hero.recoveryBody
        : {
            connected: t.hero.connectedBody,
            connecting: t.hero.connectingBody,
            reconnecting: t.hero.reconnectingBody,
            incompatible: connection?.incompatibleReason ?? t.hero.incompatibleBody,
          }[ui];
  const modeName = strings.display.modes[desk.mode].name;

  return (
    <Card className="relative overflow-hidden bg-gradient-to-br from-primary/10 via-card to-card p-6">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -top-24 -right-24 size-72 rounded-full bg-primary/10 blur-3xl"
      />
      <div className="relative grid items-center gap-6 md:grid-cols-[minmax(0,18rem)_1fr] lg:gap-10">
        <ProductArt connection={connection} className="max-w-72" />
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
            {display ? <Chip label={t.hero.showing}>{modeName}</Chip> : null}
            <Chip label={t.hero.firmware}>
              <Known value={connection?.device?.firmwareVersion} why={strings.why.firmware}>
                {(version) => version}
              </Known>
            </Chip>
            {canMirrorState(connection) && connection?.reported != null ? (
              <Chip label={t.hero.buddy}>
                <Known value={connection?.reported} why={strings.why.buddy}>
                  {(state) => strings.studio.states[state]}
                </Known>
              </Chip>
            ) : null}
          </dl>
          {updateAvailable ? (
            <p
              className="flex flex-wrap items-center gap-2 text-sm"
              role="status"
              data-testid="update-hint"
            >
              <Badge variant="secondary" className="h-6 px-2.5">
                {t.hero.updateAvailable}
              </Badge>
              <span className="text-muted-foreground">{t.hero.updateAvailableBody}</span>
              <Button variant="link" size="sm" onClick={() => onNavigate('device')}>
                {t.hero.openDevice}
              </Button>
            </p>
          ) : null}
          <div className="flex flex-wrap gap-2">
            {ui === 'incompatible' || unknownIdentity || recovery ? (
              <Button onClick={() => onNavigate('device')}>
                <Zap data-icon="inline-start" aria-hidden="true" />
                {t.hero.openDevice}
              </Button>
            ) : display ? (
              <Button
                onClick={() => {
                  // Lands on "Show now" when the device has it; PageTabs falls back to Views otherwise.
                  openPageTab('display', 'show-now');
                  onNavigate('display');
                }}
              >
                <LayoutGrid data-icon="inline-start" aria-hidden="true" />
                {t.hero.changeView}
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
  className,
  neutral,
}: {
  icon: ReactNode;
  label: string;
  value: number | null | undefined;
  why: string;
  testId: string;
  badge?: ReactNode;
  hint?: ReactNode;
  danger?: boolean;
  className?: string;
  /** Not a load: a high value is not a warning (volume). */
  neutral?: boolean;
}): ReactElement {
  const tone = neutral ? 'normal' : meterTone(value, danger);
  return (
    <Card size="sm" className={cn('gap-3', className)}>
      <CardHeader>
        <CardDescription className="flex items-center gap-2 text-sm">
          <IconChip>{icon}</IconChip>
          {label}
        </CardDescription>
        {badge ? <CardAction>{badge}</CardAction> : null}
      </CardHeader>
      <CardContent className="space-y-3">
        <p
          className={cn('text-3xl font-semibold tracking-tight tabular-nums', TONE_TEXT[tone])}
          data-testid={testId}
          data-tone={tone}
        >
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
            className={cn('[&_[data-slot=progress-track]]:h-1.5', TONE_BAR[tone])}
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
          <IconChip>
            <Music2 />
          </IconChip>
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

function Microphone({ desk }: { desk: DeskStatusDto }): ReactElement {
  const { mic } = desk;
  const Icon = mic.muted === 'yes' ? MicOff : Mic;
  return (
    <Card size="sm">
      <CardHeader>
        <CardDescription className="flex items-center gap-2 text-sm">
          <IconChip>
            <Icon />
          </IconChip>
          {t.microphone}
        </CardDescription>
        <CardAction className="flex gap-1.5">
          {mic.live ? (
            <Badge variant="destructive" className="h-6 px-2.5" data-testid="mic-live">
              {t.micLive}
            </Badge>
          ) : null}
          <Badge variant="secondary" className="h-6 px-2.5" data-testid="mic-muted">
            {t.micMutedStates[mic.muted]}
          </Badge>
        </CardAction>
      </CardHeader>
      <CardContent className="space-y-3">
        {mic.devices.length === 0 ? (
          <p className="text-sm text-muted-foreground" data-testid="mic-none">
            {mic.muted === 'unknown' ? strings.why.mic : t.micNone}
          </p>
        ) : (
          <ul className="space-y-1 text-sm" data-testid="mic-devices">
            {mic.devices.map((device, index) => (
              <li
                key={`${device.name}-${index}`}
                className="flex items-center justify-between gap-3"
              >
                <span className="truncate">{device.name}</span>
                <span
                  className={cn(
                    'shrink-0 text-xs',
                    device.state === 'live' || device.state === 'cannotMuteLive'
                      ? 'font-medium text-destructive'
                      : device.state === 'cannotMuteUnknown'
                        ? 'font-medium text-warning'
                        : 'text-muted-foreground',
                  )}
                >
                  {t.micDeviceStates[device.state]}
                </span>
              </li>
            ))}
          </ul>
        )}
        {mic.live ? <p className="text-xs text-muted-foreground">{t.micLiveHint}</p> : null}
        <p className="text-xs text-muted-foreground">{t.micScope}</p>
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
          <IconChip>
            <Zap />
          </IconChip>
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
              <AccessibilityHelp title={t.permissionTitle} body={t.permission} />
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
  updateAvailable = false,
  onNavigate,
}: {
  connection: ConnectionStatusDto | null;
  desk: DeskStatusDto | null;
  updateAvailable?: boolean;
  onNavigate: NavigateTo;
}): ReactElement {
  const devMode = useDevMode();
  return (
    <Page title={t.title} description={t.description}>
      <Hero
        connection={connection}
        desk={desk}
        updateAvailable={updateAvailable}
        onNavigate={onNavigate}
      />

      {devMode ? <LinkHealth connection={connection} /> : null}

      <section aria-labelledby="monitoring-heading" className="space-y-3">
        <div className="flex flex-wrap items-baseline justify-between gap-2">
          <h2 id="monitoring-heading" className="flex items-center gap-2 text-base font-semibold">
            <IconChip>
              <Monitor />
            </IconChip>
            {t.monitoring}
          </h2>
          <p className="text-xs text-muted-foreground">{t.monitoringNote}</p>
        </div>
        {desk ? (
          <>
            <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 lg:gap-4">
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
                neutral
                className="col-span-2 lg:col-span-1"
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
            <div className="grid gap-4 lg:grid-cols-3">
              <NowPlaying desk={desk} />
              <Microphone desk={desk} />
              <LastAction desk={desk} />
            </div>
          </>
        ) : (
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 lg:gap-4">
            {[0, 1, 2].map((index) => (
              <Skeleton key={index} className="h-36 rounded-xl" />
            ))}
          </div>
        )}
      </section>
    </Page>
  );
}
