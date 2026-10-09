import type { ReactElement, ReactNode } from 'react';
import { CircleCheck, LoaderCircle, TriangleAlert, Unplug } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Skeleton } from '@/components/ui/skeleton';
import { Switch } from '@/components/ui/switch';
import { Known } from '@/components/kivori/known';
import { Page } from '@/components/kivori/page';
import { uiConnection, type UiConnection } from '@/hooks/use-kivori';
import type { AppInfoDto, ConnectionStatusDto } from '@/lib/ipc/types';
import { setDevMode, useDevMode } from '@/lib/dev-mode';
import { strings } from '@/lib/i18n/strings';
import { cn } from '@/lib/utils';
import { Diagnostics } from './Diagnostics';
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

const version = (v: { major: number; minor: number }): string => `${v.major}.${v.minor}`;

/** Device: connection health, versions, and the user-initiated firmware update. */
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

  return (
    <Page title={t.title} description={t.description}>
      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>{t.connection}</CardTitle>
            {ui !== 'connected' ? <CardDescription>{t.notConnected}</CardDescription> : null}
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
            <dl className="divide-y">
              <Row label={t.firmware} testId="device-firmware">
                <Known value={device?.firmwareVersion} why={strings.why.firmware}>
                  {(v) => v}
                </Known>
              </Row>
              {devMode ? (
                <>
                  <Row label={t.protocol}>
                    <Known value={device?.protocolVersion} why={strings.why.firmware}>
                      {version}
                    </Known>
                  </Row>
                  <Row label={t.deviceId}>
                    <Known value={device?.deviceIdHashShort} why={strings.why.firmware}>
                      {(id) => <span className="font-mono">{id}</span>}
                    </Known>
                  </Row>
                  <Row label={t.buddyRequested} testId="desired">
                    <Known value={connection?.desired} why={strings.why.buddy}>
                      {(state) => strings.studio.states[state]}
                    </Known>
                  </Row>
                  <Row label={t.buddyReported} testId="reported">
                    <Known value={connection?.reported} why={strings.why.buddy}>
                      {(state) => strings.studio.states[state]}
                    </Known>
                  </Row>
                </>
              ) : null}
            </dl>
          </CardContent>
        </Card>

        <div className="flex flex-col gap-4">
          <FirmwareUpdate connected={ui === 'connected'} />
          <Diagnostics />
          <Card size="sm">
            <CardHeader>
              <CardTitle>{t.app}</CardTitle>
            </CardHeader>
            <CardContent>
              <dl className="divide-y">
                <Row label={t.appVersion}>
                  {appInfo ? appInfo.appVersion : <Skeleton className="h-4 w-16" />}
                </Row>
                {devMode ? (
                  <Row label={t.appProtocol}>
                    {appInfo ? version(appInfo.protocolVersion) : <Skeleton className="h-4 w-10" />}
                  </Row>
                ) : null}
              </dl>
            </CardContent>
          </Card>
          <Card size="sm">
            <CardHeader>
              <CardTitle id="advanced-heading" role="heading" aria-level={2}>
                {t.advanced}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="flex items-center justify-between gap-4">
                <div className="space-y-0.5">
                  <Label htmlFor="developer-mode">{t.developerMode}</Label>
                  <p id="developer-mode-hint" className="text-xs text-muted-foreground">
                    {t.developerModeHint}
                  </p>
                </div>
                <Switch
                  id="developer-mode"
                  aria-describedby="developer-mode-hint"
                  checked={devMode}
                  onCheckedChange={setDevMode}
                />
              </div>
            </CardContent>
          </Card>
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
