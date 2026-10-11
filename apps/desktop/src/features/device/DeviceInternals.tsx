import { useEffect, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import { Progress } from '@kivori/ui/components/progress';
import { Tooltip, TooltipContent, TooltipTrigger } from '@kivori/ui/components/tooltip';
import { getDeviceInternals } from '@/lib/ipc';
import type { DeviceInternalsDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { setLatestInternals } from './internals-latest';

const t = strings.internals;

type Snapshot = NonNullable<DeviceInternalsDto['snapshot']>;

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

/** A value the device could not measure: said in words, never as 0. */
function Unavailable(): ReactElement {
  return (
    <Tooltip>
      <TooltipTrigger
        aria-label={`${t.unavailable}. ${t.unavailableWhy}`}
        className="cursor-help rounded-sm text-muted-foreground underline decoration-dotted underline-offset-4 outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        {t.unavailable}
      </TooltipTrigger>
      <TooltipContent>{t.unavailableWhy}</TooltipContent>
    </Tooltip>
  );
}

function Value<T>({
  value,
  children,
}: {
  value: T | null;
  children: (value: T) => ReactNode;
}): ReactElement {
  return value === null ? <Unavailable /> : <>{children(value)}</>;
}

export function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${bytes} B`;
}

const percent = (part: number, whole: number): number => (whole > 0 ? (part / whole) * 100 : 0);

const seconds = (s: number): string => {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h > 0 ? `${h} h ${m} min` : m > 0 ? `${m} min ${s % 60} s` : `${s} s`;
};

/** `used` of `limit` with a percentage and a bar; only the size when the limit is unknown. */
function Usage({
  label,
  used,
  limit,
  testId,
}: {
  label: string;
  used: number | null;
  limit: number | null;
  testId: string;
}): ReactElement {
  return (
    <div className="space-y-1.5 py-2 text-sm">
      <div className="flex items-center justify-between gap-4">
        <dt className="text-muted-foreground">{label}</dt>
        <dd className="text-right font-medium" data-testid={testId}>
          <Value value={used}>
            {(u) =>
              limit === null
                ? formatBytes(u)
                : `${formatBytes(u)} of ${formatBytes(limit)} (${percent(u, limit).toFixed(1)}%)`
            }
          </Value>
        </dd>
      </div>
      {used !== null && limit !== null ? (
        <Progress value={Math.min(100, percent(used, limit))} aria-label={label} />
      ) : null}
    </div>
  );
}

function Body({ s }: { s: Snapshot }): ReactElement {
  return (
    <>
      <Group title={t.groups.flash}>
        <Usage
          label={t.rows.appImage}
          used={s.appImageBytes}
          limit={s.appPartitionBytes}
          testId="internals-app-image"
        />
        <Usage
          label={t.rows.assetBlob}
          used={s.assetBlobBytes}
          limit={s.assetBlobCapBytes}
          testId="internals-asset-blob"
        />
        <Row label={t.rows.flashTotal}>
          <Value value={s.flashTotalBytes}>{formatBytes}</Value>
        </Row>
      </Group>
      <Group title={t.groups.ram}>
        <Usage
          label={t.rows.ramStatic}
          used={s.ramStaticBytes}
          limit={s.ramTotalBytes}
          testId="internals-ram-static"
        />
        <Row label={t.rows.ramFree}>
          <Value value={s.ramFreeBytes}>{formatBytes}</Value>
        </Row>
        <Row label={t.rows.stackFreeMin}>
          <Value value={s.stackFreeMinBytes}>{formatBytes}</Value>
        </Row>
      </Group>
      <Group title={t.groups.cpu}>
        <Row label={t.rows.busy}>
          <Value value={s.busyPercent}>{(p) => `${p}%`}</Value>
        </Row>
        <Row label={t.rows.compose}>
          <Value value={s.composeMs}>{(ms) => `${ms} ms`}</Value>
        </Row>
        <Row label={t.rows.flush}>
          <Value value={s.flushMs}>{(ms) => `${ms} ms`}</Value>
        </Row>
        <Row label={t.rows.fps}>
          <Value value={s.fps}>{(n) => `${n} fps`}</Value>
        </Row>
      </Group>
      <Group title={t.groups.chip}>
        <Row label={t.rows.chip}>
          <Value value={s.chipModel}>
            {(model) =>
              s.chipRevision
                ? `${t.values[model]} rev v${s.chipRevision.major}.${s.chipRevision.minor}`
                : t.values[model]
            }
          </Value>
        </Row>
        <Row label={t.rows.temperature}>
          <Value value={s.temperatureDeciCelsius}>{(d) => `${(d / 10).toFixed(1)} °C`}</Value>
        </Row>
        <Row label={t.rows.uptime}>
          <Value value={s.uptimeMs}>{(ms) => seconds(Math.floor(ms / 1000))}</Value>
        </Row>
        <Row label={t.rows.resetReason}>
          <Value value={s.resetReason}>{(reason) => t.values[reason]}</Value>
        </Row>
      </Group>
      <Group title={t.groups.link}>
        <Row label={t.rows.edgeQueue}>
          <Value value={s.edgeQueueHighWater}>{(n) => n}</Value>
        </Row>
        <Row label={t.rows.droppedEdges}>
          <Value value={s.droppedEdges}>{(n) => n}</Value>
        </Row>
        <Row label={t.rows.invalidTransitions}>
          <Value value={s.invalidTransitions}>{(n) => n}</Value>
        </Row>
        <Row label={t.rows.framesIn}>
          <Value value={s.framesIn}>{(n) => n}</Value>
        </Row>
        <Row label={t.rows.framesOut}>
          <Value value={s.framesOut}>{(n) => n}</Value>
        </Row>
        <Row label={t.rows.framesRejected}>
          <Value value={s.framesRejected}>{(n) => n}</Value>
        </Row>
      </Group>
    </>
  );
}

/**
 * Developer-mode numbers from the device itself. Mounted only with developer mode on; asks the
 * device about once a second while it is mounted and the window is visible, and never otherwise.
 */
export function DeviceInternals(): ReactElement {
  const [data, setData] = useState<DeviceInternalsDto | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async (): Promise<void> => {
      try {
        if (!document.hidden) {
          const next = await getDeviceInternals();
          if (active) {
            setLatestInternals(next);
            setData(next);
            setFailed(false);
          }
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
      setLatestInternals(null);
    };
  }, []);

  const notice = failed
    ? t.loadFailed
    : !data
      ? t.waiting
      : !data.connected
        ? t.notConnected
        : !data.supported
          ? t.unsupported
          : !data.snapshot
            ? t.waiting
            : null;

  return (
    <Card>
      <CardHeader>
        <CardTitle id="internals-heading" role="heading" aria-level={2}>
          {t.heading}
        </CardTitle>
        <CardDescription>{t.description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {notice ? (
          <p
            aria-live="polite"
            className={failed ? 'text-sm text-destructive' : 'text-sm text-muted-foreground'}
          >
            {notice}
          </p>
        ) : null}
        {!failed && data?.connected && data.supported && data.snapshot ? (
          <Body s={data.snapshot} />
        ) : null}
      </CardContent>
    </Card>
  );
}
