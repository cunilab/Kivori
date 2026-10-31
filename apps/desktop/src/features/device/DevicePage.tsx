import type { ReactElement, ReactNode } from 'react';
import {
  CircleCheck,
  Code,
  Cpu,
  Download,
  LoaderCircle,
  TriangleAlert,
  Unplug,
} from 'lucide-react';
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
import { PageTabs, type PageTab } from '@/components/kivori/page-tabs';
import { uiConnection, type UiConnection } from '@/hooks/use-kivori';
import type { AppInfoDto, ConnectionStatusDto } from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { cn } from '@kivori/ui/lib/utils';
import { useDevMode } from '@/lib/dev-mode';
import { buildNotices, shortCommit } from './build';
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
  const notices = buildNotices(device?.build, appInfo?.commit, devMode);
  // Without a device, its own rows can only say "Unknown": they collapse into one muted line.
  const hasDevice = ui === 'connected' || ui === 'incompatible';
  const session: [string, string | null, string, boolean][] = [
    [strings.device.protocol, version(device?.protocolVersion), strings.why.firmware, true],
    [
      strings.device.appProtocol,
      version(appInfo?.protocolVersion),
      strings.developer.appInfoWaiting,
      false,
    ],
    [
      strings.device.buddyRequested,
      connection ? strings.studio.states[connection.desired] : null,
      strings.why.buddy,
      false,
    ],
    [
      strings.device.buddyReported,
      connection?.reported ? strings.studio.states[connection.reported] : null,
      strings.why.buddy,
      true,
    ],
  ];

  const tabs: PageTab[] = [
    {
      value: 'overview',
      label: t.tabs.overview,
      Icon: Cpu,
      content: (
        <Card>
          <CardHeader>
            <CardTitle>{t.connection}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {ui === 'loading' ? (
              <Skeleton className="h-10 w-full" />
            ) : ui === 'disconnected' && !device ? null : (
              // Disconnected, the empty state below already says so.
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
                  <p className="text-lg font-semibold tracking-tight" data-testid="device-product">
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
                {notices.dirty ? (
                  <Alert data-testid="device-build-dirty">
                    <TriangleAlert aria-hidden="true" />
                    <AlertTitle>{t.build.dirtyTitle}</AlertTitle>
                    <AlertDescription>{t.build.dirtyBody}</AlertDescription>
                  </Alert>
                ) : null}
                {notices.differs && device.build?.commit && appInfo?.commit ? (
                  <Alert data-testid="device-build-differs">
                    <TriangleAlert aria-hidden="true" />
                    <AlertTitle>{t.build.differsTitle}</AlertTitle>
                    <AlertDescription>
                      {format(t.build.differsBody, {
                        device: shortCommit(device.build.commit),
                        app: shortCommit(appInfo.commit),
                      })}
                    </AlertDescription>
                  </Alert>
                ) : null}
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
                    {device.build || devMode ? (
                      <Row label={t.build.label} testId="device-build">
                        <Known value={device.build} why={t.build.notReported}>
                          {(build) =>
                            build.commit ? (
                              <>
                                <span className="font-mono">{shortCommit(build.commit)}</span>
                                {build.dirty ? ` · ${t.build.uncommitted}` : ''}
                              </>
                            ) : (
                              t.build.noGit
                            )
                          }
                        </Known>
                      </Row>
                    ) : null}
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
      ),
    },
    {
      value: 'firmware',
      label: t.tabs.firmware,
      Icon: Download,
      content: (
        <FirmwareUpdate connected={ui === 'connected'} incompatible={ui === 'incompatible'} />
      ),
    },
    ...(devMode
      ? [
          {
            value: 'internals',
            label: t.tabs.internals,
            Icon: Code,
            content: (
              <>
                <Card>
                  <CardHeader>
                    <CardTitle role="heading" aria-level={2}>
                      {strings.developer.session}
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <dl className="divide-y">
                      {session
                        .filter(([, , , deviceOnly]) => hasDevice || !deviceOnly)
                        .map(([label, value, why]) => (
                          <Row key={label} label={label}>
                            <Known value={value} why={why}>
                              {(known) => known}
                            </Known>
                          </Row>
                        ))}
                    </dl>
                    {hasDevice ? null : (
                      <p className="pt-2 text-sm text-muted-foreground">{t.sessionDeviceOnly}</p>
                    )}
                  </CardContent>
                </Card>
                <DeviceInternals connection={connection} />
              </>
            ),
          },
        ]
      : []),
  ];

  return (
    <Page title={t.title} description={t.description}>
      <PageTabs id="device" label={t.tabsLabel} tabs={tabs} />
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
