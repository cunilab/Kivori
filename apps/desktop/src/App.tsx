import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './components/ui/tabs';
import { DeskPanel } from './features/desk/DeskPanel';
import { ConnectionStatus } from './features/connection/ConnectionStatus';
import { Log } from './features/connection/Log';
import { DeviceStudio } from './features/device-studio/DeviceStudio';
import { getAppInfo } from './lib/ipc';
import { strings } from './lib/i18n/strings';

// The Device Studio route is served in dev builds only (FR-028). The backend independently gates its
// dev-only IPC commands behind the `device-studio` Cargo feature, so both layers must agree.
const DEVICE_STUDIO_BUILD = import.meta.env.DEV;

export function App(): ReactElement {
  const [studioEnabled, setStudioEnabled] = useState(false);

  useEffect(() => {
    let active = true;
    void getAppInfo().then((info) => {
      if (active) setStudioEnabled(info.deviceStudioEnabled);
    });
    return () => {
      active = false;
    };
  }, []);

  const showStudio = DEVICE_STUDIO_BUILD && studioEnabled;
  const nav = strings.navigation;

  return (
    <main className="mx-auto min-h-screen w-full max-w-6xl px-4 py-6 sm:px-6">
      <header className="mb-6">
        <h1 className="text-2xl font-semibold tracking-tight">{strings.appTitle}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{strings.connectingHint}</p>
      </header>

      <Tabs defaultValue="overview" className="w-full">
        <TabsList aria-label={strings.appTitle}>
          <TabsTrigger value="overview">{nav.overview}</TabsTrigger>
          {showStudio ? <TabsTrigger value="studio">{nav.studio}</TabsTrigger> : null}
          <TabsTrigger value="log">{nav.log}</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="pt-4">
          <div className="space-y-4">
            <ConnectionStatus />
            <DeskPanel />
          </div>
        </TabsContent>
        {showStudio ? (
          <TabsContent value="studio" className="pt-4">
            <DeviceStudio />
          </TabsContent>
        ) : null}
        <TabsContent value="log" className="pt-4">
          <Log />
        </TabsContent>
      </Tabs>
    </main>
  );
}
