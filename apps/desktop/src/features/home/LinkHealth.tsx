import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import { Known } from '@/components/kivori/known';
import { getDiagnostics } from '@/lib/ipc';
import type { ConnectionStatusDto, DiagnosticsDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';

const t = strings.home.health;

/** Mounted only in developer mode. Reads the safe diagnostics cache, never device internals. */
export function LinkHealth({
  connection,
}: {
  connection: ConnectionStatusDto | null;
}): ReactElement {
  const generation = connection?.connectionGeneration;
  const state = connection?.connection;
  const [health, setHealth] = useState<DiagnosticsDto['connection'] | null>(null);

  useEffect(() => {
    setHealth(null);
    if (generation === undefined || state === undefined) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async (): Promise<void> => {
      try {
        if (!document.hidden) {
          const next = (await getDiagnostics()).connection;
          if (active && !document.hidden) {
            setHealth(next.generation === generation && next.state === state ? next : null);
          }
        }
      } catch {
        if (active) setHealth(null);
      } finally {
        if (active) timer = setTimeout(() => void poll(), 1000);
      }
    };
    const visibilityChanged = (): void => {
      if (document.hidden) setHealth(null);
    };
    document.addEventListener('visibilitychange', visibilityChanged);
    void poll();
    return () => {
      active = false;
      clearTimeout(timer);
      document.removeEventListener('visibilitychange', visibilityChanged);
    };
  }, [generation, state]);

  const reconnects =
    health && health.generation === generation && health.state === state ? health.reconnects : null;
  const rows = [
    [t.link, state ? strings.connection.status[state] : null],
    [t.retries, connection?.retryCount],
    [t.reconnects, reconnects],
  ] as const;
  return (
    <section aria-labelledby="home-health-heading">
      <Card size="sm">
        <CardHeader>
          <CardTitle id="home-health-heading" role="heading" aria-level={2}>
            {t.heading}
          </CardTitle>
          <CardDescription>{t.description}</CardDescription>
        </CardHeader>
        <CardContent>
          <dl className="grid gap-4 sm:grid-cols-3">
            {rows.map(([label, value]) => (
              <div key={label} className="space-y-1">
                <dt className="text-xs text-muted-foreground">{label}</dt>
                <dd className="font-medium tabular-nums">
                  <Known value={value} why={t.unavailable}>
                    {(known) => known}
                  </Known>
                </dd>
              </div>
            ))}
          </dl>
        </CardContent>
      </Card>
    </section>
  );
}
