import type { ReactElement } from 'react';
import { Monitor, Moon, Sun, type LucideIcon } from 'lucide-react';
import { Button } from '@kivori/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import { RadioGroup, RadioGroupItem } from '@kivori/ui/components/radio-group';
import { Skeleton } from '@kivori/ui/components/skeleton';
import { Page } from '@/components/kivori/page';
import { useDevMode } from '@/lib/dev-mode';
import type { AppInfoDto, ConnectionStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { setTheme, useTheme, type ThemeChoice } from '@/lib/theme';
import { cn } from '@kivori/ui/lib/utils';
import { Diagnostics } from '../device/Diagnostics';
import { StartupCard } from '../device/StartupCard';
import { DeveloperTools } from '../developer/DeveloperTools';
import { AppUpdates } from '../updates/AppUpdates';

const t = strings.settings;

const THEMES: { value: ThemeChoice; label: string; Icon: LucideIcon }[] = [
  { value: 'light', label: strings.theme.light, Icon: Sun },
  { value: 'dark', label: strings.theme.dark, Icon: Moon },
  { value: 'system', label: strings.theme.system, Icon: Monitor },
];

function Appearance(): ReactElement {
  const { theme } = useTheme();
  return (
    <Card size="sm">
      <CardHeader>
        <CardTitle id="appearance-heading">{t.appearance.title}</CardTitle>
        <CardDescription>{t.appearance.description}</CardDescription>
      </CardHeader>
      <CardContent>
        <RadioGroup
          aria-labelledby="appearance-heading"
          value={theme}
          onValueChange={(value) => setTheme(value as ThemeChoice)}
          className="grid grid-cols-3 gap-2"
        >
          {THEMES.map(({ value, label, Icon }) => (
            <label
              key={value}
              data-theme-choice={value}
              className={cn(
                'flex cursor-pointer flex-col items-center gap-2 rounded-lg bg-card px-2 py-3 text-sm font-medium ring-1 ring-foreground/10 transition-colors hover:bg-muted/50 has-focus-visible:ring-2 has-focus-visible:ring-ring',
                theme === value && 'ring-2 ring-primary',
              )}
            >
              <Icon className="size-5" aria-hidden="true" />
              <span className="flex items-center gap-2">
                <RadioGroupItem value={value} />
                {label}
              </span>
            </label>
          ))}
        </RadioGroup>
      </CardContent>
    </Card>
  );
}

/** Settings: this computer's app preferences, updates, support tools and developer mode. */
export function SettingsPage({
  connection,
  appInfo,
  onRunSetupAgain,
}: {
  connection: ConnectionStatusDto | null;
  appInfo: AppInfoDto | null;
  onRunSetupAgain: () => void;
}): ReactElement {
  const devMode = useDevMode();
  return (
    <Page title={t.title} description={t.description}>
      <div className="grid gap-4 lg:grid-cols-2">
        <div className="flex flex-col gap-4">
          <Appearance />
          <StartupCard />
          <Card size="sm">
            <CardHeader>
              <CardTitle>{t.updates}</CardTitle>
            </CardHeader>
            <CardContent>
              <dl className="divide-y">
                <div className="flex items-center justify-between gap-4 py-2.5 text-sm">
                  <dt className="text-muted-foreground">{strings.device.app}</dt>
                  <dd className="text-right font-medium" data-testid="app-version">
                    {appInfo ? appInfo.appVersion : <Skeleton className="h-4 w-16" />}
                  </dd>
                </div>
              </dl>
              <AppUpdates />
            </CardContent>
          </Card>
          <Card size="sm">
            <CardHeader>
              <CardTitle>{strings.onboarding.runAgain.title}</CardTitle>
              <CardDescription>{strings.onboarding.runAgain.body}</CardDescription>
            </CardHeader>
            <CardContent>
              <Button variant="outline" onClick={onRunSetupAgain}>
                {strings.onboarding.runAgain.action}
              </Button>
            </CardContent>
          </Card>
        </div>
        <div className="flex flex-col gap-4">
          <Diagnostics technical={devMode} connection={connection} />
        </div>
      </div>
      <DeveloperTools connection={connection} appInfo={appInfo} />
    </Page>
  );
}
