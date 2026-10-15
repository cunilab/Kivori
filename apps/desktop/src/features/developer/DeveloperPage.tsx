import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@kivori/ui/components/card';
import { Label } from '@kivori/ui/components/label';
import { Switch } from '@kivori/ui/components/switch';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@kivori/ui/components/tabs';
import { Known } from '@/components/kivori/known';
import { Page } from '@/components/kivori/page';
import { setDevMode, useDevMode } from '@/lib/dev-mode';
import type { AppInfoDto, ConnectionStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { ActivityPage } from '../activity/ActivityPage';
import { Diagnostics } from '../device/Diagnostics';
import { DeviceInternals } from '../device/DeviceInternals';
import { DeviceStudio } from '../device-studio/DeviceStudio';
import { ReactionControls } from './ReactionControls';

type Tool = 'diagnostics' | 'activity' | 'reactions' | 'studio';
const t = strings.developer;

export function DeveloperPage({
  connection,
  appInfo,
}: {
  connection: ConnectionStatusDto | null;
  appInfo: AppInfoDto | null;
}): ReactElement {
  const devMode = useDevMode();
  // Keep the static build gate here: a runtime prop alone would retain Studio in production bundles.
  const showStudio = import.meta.env.DEV && appInfo?.deviceStudioEnabled === true;
  const [tool, setTool] = useState<Tool>('diagnostics');
  const current = !devMode || (tool === 'studio' && !showStudio) ? 'diagnostics' : tool;
  useEffect(() => {
    if (current !== tool) setTool(current);
  }, [current, tool]);
  const version = (value: { major: number; minor: number } | undefined): string | null =>
    value ? `${value.major}.${value.minor}` : null;
  const rows: [string, string | null, string][] = [
    [strings.device.protocol, version(connection?.device?.protocolVersion), strings.why.firmware],
    [strings.device.appProtocol, version(appInfo?.protocolVersion), t.appInfoWaiting],
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
      <Card>
        <CardContent className="flex items-center justify-between gap-4">
          <div className="space-y-1">
            <Label htmlFor="developer-mode">{strings.device.developerMode}</Label>
            <p id="developer-mode-hint" className="text-sm text-muted-foreground">
              {t.toggleHint}
            </p>
          </div>
          <Switch
            id="developer-mode"
            aria-describedby="developer-mode-hint"
            checked={devMode}
            onCheckedChange={setDevMode}
          />
        </CardContent>
      </Card>
      {devMode ? (
        <Tabs value={current} onValueChange={(value) => setTool(value as Tool)}>
          <TabsList aria-label={t.tools} className="h-auto flex-wrap">
            <TabsTrigger value="diagnostics">{strings.diagnostics.heading}</TabsTrigger>
            <TabsTrigger value="activity">{strings.navigation.activity}</TabsTrigger>
            <TabsTrigger value="reactions">{strings.companion.actions}</TabsTrigger>
            {showStudio ? (
              <TabsTrigger value="studio">{strings.navigation.studio}</TabsTrigger>
            ) : null}
          </TabsList>
          <TabsContent value="diagnostics" className="space-y-4">
            {current === 'diagnostics' ? (
              <>
                <Diagnostics />
                <Card>
                  <CardHeader>
                    <CardTitle role="heading" aria-level={2}>
                      {t.session}
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <dl className="divide-y">
                      {rows.map(([label, value, why]) => (
                        <div
                          key={label}
                          className="flex items-center justify-between gap-4 py-2 text-sm"
                        >
                          <dt className="text-muted-foreground">{label}</dt>
                          <dd>
                            <Known value={value} why={why}>
                              {(known) => known}
                            </Known>
                          </dd>
                        </div>
                      ))}
                    </dl>
                  </CardContent>
                </Card>
                <DeviceInternals />
              </>
            ) : null}
          </TabsContent>
          <TabsContent value="activity">
            {current === 'activity' ? <ActivityPage /> : null}
          </TabsContent>
          <TabsContent value="reactions">
            {current === 'reactions' ? <ReactionControls connection={connection} /> : null}
          </TabsContent>
          {showStudio ? (
            <TabsContent value="studio">
              {current === 'studio' ? <DeviceStudio connection={connection} /> : null}
            </TabsContent>
          ) : null}
        </Tabs>
      ) : (
        <Diagnostics technical={false} />
      )}
    </Page>
  );
}
