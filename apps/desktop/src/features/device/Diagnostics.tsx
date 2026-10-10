import { useEffect, useRef, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { Check, Copy } from 'lucide-react';
import { Button } from '@kivori/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import { Known } from '@/components/kivori/known';
import { getDiagnostics } from '@/lib/ipc';
import type { DiagnosticsDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';

const t = strings.diagnostics;
const unknown = strings.unknown;
const v = t.values;

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
    <div className="flex items-center justify-between gap-4 py-2 text-sm">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-right font-medium" data-testid={testId}>
        {children}
      </dd>
    </div>
  );
}

function Group({ title, children }: { title: string; children: ReactNode }): ReactElement {
  return (
    <section className="space-y-1">
      <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">{title}</h3>
      <dl className="divide-y">{children}</dl>
    </section>
  );
}

const seconds = (s: number): string => {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h > 0 ? `${h} h ${m} min` : m > 0 ? `${m} min ${s % 60} s` : `${s} s`;
};

const kib = (bytes: number): string => `${(bytes / 1024).toFixed(1)} KiB`;

/** Versions, link health and host services, polled while visible. Never shows ports or paths. */
export function Diagnostics(): ReactElement {
  const [data, setData] = useState<DiagnosticsDto | null>(null);
  const [failed, setFailed] = useState(false);
  const [copy, setCopy] = useState<'idle' | 'copied' | 'failed'>('idle');
  const latest = useRef<DiagnosticsDto | null>(null);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async (): Promise<void> => {
      try {
        const snapshot = await getDiagnostics();
        if (active) {
          latest.current = snapshot;
          setData(snapshot);
          setFailed(false);
        }
      } catch {
        if (active) setFailed(true);
      } finally {
        if (active) timer = setTimeout(() => void poll(), 1000);
      }
    };
    void poll();
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, []);

  const copyJson = async (): Promise<void> => {
    if (!latest.current) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(latest.current, null, 2));
      setCopy('copied');
    } catch {
      setCopy('failed');
    }
    setTimeout(() => setCopy('idle'), 2000);
  };

  const d = data;
  return (
    <Card>
      <CardHeader>
        <CardTitle id="diagnostics-heading" role="heading" aria-level={2}>
          {t.heading}
        </CardTitle>
        <CardDescription>{t.description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {failed ? <p className="text-sm text-destructive">{t.loadFailed}</p> : null}
        <Group title={t.groups.versions}>
          <Row label={t.rows.app}>{d?.versions.app ?? unknown}</Row>
          <Row label={t.rows.firmware}>
            <Known value={d?.versions.firmware} why={t.why.device}>
              {(x) => x}
            </Known>
          </Row>
          <Row label={t.rows.protocol}>{d?.versions.protocol ?? unknown}</Row>
          <Row label={t.rows.negotiatedMinor}>
            <Known value={d?.versions.negotiatedMinor} why={t.why.device}>
              {(x) => x}
            </Known>
          </Row>
          <Row label={t.rows.capabilities}>
            <Known value={d?.versions.capabilities} why={t.why.device}>
              {(names) => (names.length ? names.join(', ') : v.none)}
            </Known>
          </Row>
          <Row label={t.rows.deviceHash}>
            <Known value={d?.versions.deviceHash} why={t.why.device}>
              {(x) => <span className="font-mono">{x}</span>}
            </Known>
          </Row>
        </Group>
        <Group title={t.groups.connection}>
          <Row label={t.rows.state}>{d?.connection.state ?? unknown}</Row>
          <Row label={t.rows.connectedFor}>
            <Known value={d?.connection.connectedForSecs} why={t.why.device}>
              {seconds}
            </Known>
          </Row>
          <Row label={t.rows.reconnects}>{d?.connection.reconnects ?? unknown}</Row>
          <Row label={t.rows.retries}>{d?.connection.retryCount ?? unknown}</Row>
          <Row label={t.rows.lastPong}>
            <Known value={d?.connection.lastPongAgeMs} why={t.why.pong}>
              {(ms) => `${ms} ms ago`}
            </Known>
          </Row>
          <Row label={t.rows.rtt}>
            <Known value={d?.connection.rttMs} why={t.why.pong}>
              {(ms) => `${ms} ms`}
            </Known>
          </Row>
        </Group>
        <Group title={t.groups.health}>
          <Row label={t.rows.uptime}>
            <Known value={d?.health.deviceUptimeMs} why={t.why.pong}>
              {(ms) => seconds(Math.floor(ms / 1000))}
            </Known>
          </Row>
          <Row label={t.rows.freeMemory}>
            <Known value={d?.health.freeBytes} why={t.why.health}>
              {kib}
            </Known>
          </Row>
          <Row label={t.rows.malformed}>{d?.health.malformedFrames ?? unknown}</Row>
          <Row label={t.rows.gaps}>{d?.health.sequenceGaps ?? unknown}</Row>
          <Row label={t.rows.deviceErrors}>{d?.health.deviceErrors ?? unknown}</Row>
        </Group>
        <Group title={t.groups.host}>
          <Row label={t.rows.systemVolume}>{d ? v[d.host.systemVolume] : unknown}</Row>
          <Row label={t.rows.appVolume}>{d ? v[d.host.appVolume] : unknown}</Row>
          <Row label={t.rows.media}>{d ? v[d.host.media] : unknown}</Row>
          <Row label={t.rows.focus}>{d ? v[d.host.focus] : unknown}</Row>
          <Row label={t.rows.inputPermission}>{d ? v[d.host.inputPermission] : unknown}</Row>
        </Group>
        <Group title={t.groups.config}>
          <Row label={t.rows.configStatus}>{d ? v[d.config.status] : unknown}</Row>
          <Row label={t.rows.schemaVersion}>{d?.config.schemaVersion ?? unknown}</Row>
          <Row label={t.rows.customBindings}>{d?.config.customBindings ?? unknown}</Row>
          <Row label={t.rows.macros}>{d?.config.macros ?? unknown}</Row>
        </Group>
        <div className="flex items-center gap-3">
          <Button variant="outline" disabled={!d} onClick={() => void copyJson()}>
            {copy === 'copied' ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
            {t.copy}
          </Button>
          <span aria-live="polite" className="text-xs text-muted-foreground">
            {copy === 'copied' ? t.copied : copy === 'failed' ? t.copyFailed : ''}
          </span>
        </div>
      </CardContent>
    </Card>
  );
}
