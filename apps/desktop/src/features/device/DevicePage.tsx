import type { ReactElement, ReactNode } from 'react';
import { CircleCheck, LoaderCircle, TriangleAlert, Unplug } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@kivori/ui/components/alert';
import { Card, CardContent, CardHeader, CardTitle } from '@kivori/ui/components/card';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@kivori/ui/components/empty';
import { Skeleton } from '@kivori/ui/components/skeleton';
import { Known } from '@/components/kivori/known';
import { Page } from '@/components/kivori/page';
import { uiConnection, type UiConnection } from '@/hooks/use-kivori';
import type { AppInfoDto, ConnectionStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { cn } from '@kivori/ui/lib/utils';
import { useDevMode } from '@/lib/dev-mode';
import { DeviceInternals } from './DeviceInternals';
import { FirmwareUpdate } from './FirmwareUpdate';

const t = strings.device;

const STATUS: Record<Exclude<UiConnection, 'loading'>, { Icon: LucideIcon; tone: string }> = {
  connected: { Icon: CircleCheck, tone: 'bg-success/10 text-success' },
  connecting: { Icon: LoaderCircle, tone: 'bg-warning/10 text-warning' },
  reconnecting: { Icon: LoaderCircle, tone: 'bg-warning/10 text-warning' },
  disconnected: { Icon: Unplug, tone: 'bg-muted text-muted-foreground' },
  incompatible: { Icon: TriangleAlert, tone: 'bg-destructive/10 text-destructive' },
};

function Row({
  label,
  children,
  testId,
}: {
  label: string;
  children: ReactNode;
  testId?: string;
}): ReactElement {
  return (
    <div className="flex items-center justify-between gap-4 py-2.5 text-sm">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-right font-medium" data-testid={testId}>
        {children}
      </dd>
    </div>
  );
}

/** Device: everything about the hardware: who it is, how it is connected, and its firmware. */
export function DevicePage({
  connection,
  appInfo,
}: {
  connection: ConnectionStatusDto | null;
  appInfo: AppInfoDto | null;
}): ReactElement {
  const ui = uiConnection(connection);
  const device = connection?.device ?? null;
  const devMode = useDevMode();
  const version = (value: { major: number; minor: number } | undefined): string | null =>
    value ? `${value.major}.${value.minor}` : null;
  const session: [string, string | null, string][] = [
    [strings.device.protocol, version(device?.protocolVersion), strings.why.firmware],
    [
      strings.device.appProtocol,
      version(appInfo?.protocolVersion),
      strings.developer.appInfoWaiting,
    ],
    [
      strings.device.buddyRequested,
      connection ? strings.studio.states[connection.desired] : null,
      strings.why.buddy,
    ],
    [
      strings.device.buddyReported,
      connection?.reported ? strings.studio.states[connection.reported] : null,
      strings.why.buddy,
    ],
  ];

  return (
    <Page title={t.title} description={t.description}>
      <div className="grid gap-4 lg:grid-cols-2">
        <div className="flex flex-col gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{t.connection}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {ui === 'loading' ? (
                <Skeleton className="h-10 w-full" />
              ) : (
                <StatusBanner ui={ui} retries={connection?.retryCount ?? 0} />
              )}
              {connection?.incompatibleReason ? (
                <Alert variant="destructive">
                  <TriangleAlert aria-hidden="true" />
                  <AlertTitle>{strings.connection.status.incompatible}</AlertTitle>
                  <AlertDescription>{connection.incompatibleReason}</AlertDescription>
                </Alert>
              ) : null}
              {device ? (
                <>
                  <div className="space-y-0.5" data-testid="device-summary">
                    <p
                      className="text-lg font-semibold tracking-tight"
                      data-testid="device-product"
                    >
                      {device.identity?.product ?? t.unnamed}
                    </p>
                    <p className="text-sm text-muted-foreground">
                      {t.firmware}{' '}
                      <span data-testid="device-firmware">
                        <Known value={device.firmwareVersion} why={strings.why.firmware}>
                          {(v) => v}
                        </Known>
                      </span>
                    </p>
                  </div>
                  <section aria-labelledby="device-details-heading" className="space-y-1">
                    <h2
                      id="device-details-heading"
                      className="text-xs font-medium uppercase tracking-wide text-muted-foreground"
                    >
                      {t.details}
                    </h2>
                    <dl className="divide-y">
                      <Row label={t.board} testId="device-board">
                        <Known value={device.identity?.board} why={t.identity.why}>
                          {(name) => name}
                        </Known>
                      </Row>
                      <Row label={t.hardwareRevision} testId="device-hw-rev">
                        <Known value={device.identity?.hwRev} why={t.identity.revisionWhy}>
                          {(rev) => rev}
                        </Known>
                      </Row>
                      <Row label={t.identitySource} testId="device-identity-source">
                        <Known value={device.identity?.source} why={t.identity.why}>
                          {(source) => t.identity.sources[source]}
                        </Known>
                      </Row>
                    </dl>
                  </section>
                </>
              ) : ui !== 'loading' ? (
                <Empty className="border-0 p-2" data-testid="device-empty">
                  <EmptyHeader>
                    <EmptyMedia variant="icon">
                      <Unplug aria-hidden="true" />
                    </EmptyMedia>
                    <EmptyTitle>{t.empty.title}</EmptyTitle>
                    <EmptyDescription>{t.empty.body}</EmptyDescription>
                  </EmptyHeader>
                </Empty>
              ) : null}
            </CardContent>
          </Card>
          {devMode ? (
            <Card>
              <CardHeader>
                <CardTitle role="heading" aria-level={2}>
                  {strings.developer.session}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <dl className="divide-y">
                  {session.map(([label, value, why]) => (
                    <Row key={label} label={label}>
                      <Known value={value} why={why}>
                        {(known) => known}
                      </Known>
                    </Row>
                  ))}
                </dl>
              </CardContent>
            </Card>
          ) : null}
        </div>

        <div className="flex flex-col gap-4">
          <FirmwareUpdate connected={ui === 'connected'} incompatible={ui === 'incompatible'} />
          {devMode ? <DeviceInternals connection={connection} /> : null}
        </div>
      </div>
    </Page>
  );
}

function StatusBanner({
  ui,
  retries,
}: {
  ui: Exclude<UiConnection, 'loading'>;
  retries: number;
}): ReactElement {
  const { Icon, tone } = STATUS[ui];
  const spinning = ui === 'connecting' || ui === 'reconnecting';
  return (
    <div
      role="status"
      aria-live="polite"
      data-state={ui}
      className={cn('flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium', tone)}
    >
      <Icon className={cn('size-4', spinning && 'animate-spin')} aria-hidden="true" />
      {strings.connection.status[ui]}
      {retries > 0 ? (
        <span className="ml-auto text-xs font-normal opacity-80">
          {t.retries}: {retries}
        </span>
      ) : null}
    </div>
  );
}
