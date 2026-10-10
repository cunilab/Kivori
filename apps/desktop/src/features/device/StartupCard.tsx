import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@kivori/ui/components/card';
import { Label } from '@kivori/ui/components/label';
import { Switch } from '@kivori/ui/components/switch';
import { getStartupSettings, onStartupChanged, setLaunchAtLogin } from '@/lib/ipc';
import type { StartupSettingsDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';

const t = strings.device.startup;

/** The "Start with Windows/macOS" switch. The OS entry is the source of truth, so it is read back. */
export function StartupCard(): ReactElement | null {
  const [settings, setSettings] = useState<StartupSettingsDto | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    getStartupSettings()
      .then((value) => {
        if (active) setSettings(value);
      })
      .catch(() => {
        if (active) setError(t.failed);
      });
    // The tray switch changes the same OS entry while this page is open.
    const unlisten = onStartupChanged((value) => {
      if (active) setSettings(value);
    }).catch(() => () => {});
    return () => {
      active = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  const change = (enabled: boolean): void => {
    setBusy(true);
    setError(null);
    setLaunchAtLogin(enabled)
      .then(setSettings)
      .catch(() => setError(t.failed))
      .finally(() => setBusy(false));
  };

  if (!settings && !error) return null;
  return (
    <Card size="sm">
      <CardHeader>
        <CardTitle>{t.title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {settings ? (
          <div className="flex items-center justify-between gap-4">
            <div className="space-y-0.5">
              <Label htmlFor="launch-at-login">{t.label[settings.platform]}</Label>
              <p id="launch-at-login-hint" className="text-xs text-muted-foreground">
                {t.hint}
              </p>
            </div>
            <Switch
              id="launch-at-login"
              aria-describedby="launch-at-login-hint"
              checked={settings.launchAtLogin}
              disabled={busy}
              onCheckedChange={change}
            />
          </div>
        ) : null}
        {error ? (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}
