import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { Card, CardContent } from '@kivori/ui/components/card';
import { Label } from '@kivori/ui/components/label';
import { Switch } from '@kivori/ui/components/switch';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@kivori/ui/components/tabs';
import { setDevMode, useDevMode } from '@/lib/dev-mode';
import type { AppInfoDto, ConnectionStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { ActivityPage } from '../activity/ActivityPage';
import { DeviceStudio } from '../device-studio/DeviceStudio';
import { ReactionControls } from './ReactionControls';

type Tool = 'activity' | 'reactions' | 'studio';
const t = strings.developer;

/** The developer-mode switch and, while it is on, the technical tools. Lives on Settings. */
export function DeveloperTools({
  connection,
  appInfo,
}: {
  connection: ConnectionStatusDto | null;
  appInfo: AppInfoDto | null;
}): ReactElement {
  const devMode = useDevMode();
  // Keep the static build gate here: a runtime prop alone would retain Studio in production bundles.
  const showStudio = import.meta.env.DEV && appInfo?.deviceStudioEnabled === true;
  const [tool, setTool] = useState<Tool>('activity');
  const current = tool === 'studio' && !showStudio ? 'activity' : tool;
  useEffect(() => {
    if (current !== tool) setTool(current);
  }, [current, tool]);
  // Turning developer mode off resets the tools, so it always reopens on the first one.
  useEffect(() => {
    if (!devMode) setTool('activity');
  }, [devMode]);
  return (
    <section aria-labelledby="developer-mode-heading" className="space-y-4">
      <h2 id="developer-mode-heading" className="sr-only">
        {t.title}
      </h2>
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
            <TabsTrigger value="activity">{strings.navigation.activity}</TabsTrigger>
            <TabsTrigger value="reactions">{strings.companion.actions}</TabsTrigger>
            {showStudio ? (
              <TabsTrigger value="studio">{strings.navigation.studio}</TabsTrigger>
            ) : null}
          </TabsList>
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
      ) : null}
    </section>
  );
}
