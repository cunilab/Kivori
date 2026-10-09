import { useEffect, useMemo, useState } from 'react';
import type { ReactElement } from 'react';
import { CircleAlert, Info, ListFilter, Search, TriangleAlert, X } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { Page } from '@/components/kivori/page';
import { getActivityLog, onActivityLog, type Unlisten } from '@/lib/ipc';
import type { ActivityEventDto, ActivitySeverity, ActivitySource } from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { cn } from '@/lib/utils';

/** The native session view is deliberately bounded to its 256 newest event IDs. */
export const VIEW_LIMIT = 256;

type Filter<T extends string> = 'all' | T;
type LogStatus = 'starting' | 'live' | 'unavailable';

const SEVERITIES: readonly ActivitySeverity[] = ['info', 'warning', 'error'];
const SOURCES: readonly ActivitySource[] = [
  'connection',
  'action',
  'device',
  'protocol',
  'firmware',
  'config',
];
const SEVERITY_STYLE = {
  info: { Icon: Info, className: 'text-primary' },
  warning: { Icon: TriangleAlert, className: 'text-warning' },
  error: { Icon: CircleAlert, className: 'text-destructive' },
} as const;

/** `deskActionUnverified` -> "Desk action unverified". */
export function humanize(type: string): string {
  const words = type.replace(/([A-Z])/g, ' $1').toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

function sameText(event: ActivityEventDto): boolean {
  const norm = (text: string): string => text.toLowerCase().replace(/[^a-z]/g, '');
  return norm(event.summary) === norm(humanize(event.type));
}

function clock(at: string): string {
  const date = new Date(at);
  return Number.isNaN(date.getTime())
    ? at
    : date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function mergeEvents(
  history: readonly ActivityEventDto[],
  live: readonly ActivityEventDto[],
): ActivityEventDto[] {
  const byId = new Map<number, ActivityEventDto>();
  for (const event of history) byId.set(event.id, event);
  for (const event of live) byId.set(event.id, event);
  return [...byId.values()].sort((a, b) => a.id - b.id).slice(-VIEW_LIMIT);
}

/** Formats only fixed metadata properties; unrecognised runtime fields cannot reach the UI. */
function details(event: ActivityEventDto): string[] {
  const metadata = event.metadata;
  if (!metadata) return [];
  const values: string[] = [];
  if (metadata.connection) values.push(`connection ${metadata.connection}`);
  if (metadata.retryCount > 0) values.push(`retry ${metadata.retryCount}`);
  if (metadata.elapsedMs > 0) values.push(`elapsed ${metadata.elapsedMs} ms`);
  if (metadata.diagnosticCategory) values.push(`diagnostic ${metadata.diagnosticCategory}`);
  if (metadata.diagnosticCode !== undefined) values.push(`code ${metadata.diagnosticCode}`);
  if (metadata.firmwareVersion) values.push(`firmware ${metadata.firmwareVersion}`);
  if (metadata.protocolVersion)
    values.push(`protocol ${metadata.protocolVersion.major}.${metadata.protocolVersion.minor}`);
  if (metadata.deviceIdHashShort) values.push(`device ${metadata.deviceIdHashShort}`);
  if (metadata.capabilities !== undefined) values.push(`capabilities ${metadata.capabilities}`);
  if (metadata.state) values.push(`state ${metadata.state}`);
  if (metadata.personality) values.push(`personality ${metadata.personality}`);
  if (metadata.selfPlay !== undefined) values.push(`self-play ${metadata.selfPlay ? 'on' : 'off'}`);
  if (metadata.action) values.push(`action ${metadata.action}`);
  if (metadata.seed !== undefined) values.push(`seed ${metadata.seed}`);
  if (metadata.appliedAtMs !== undefined) values.push(`applied ${metadata.appliedAtMs} ms`);
  if (metadata.autonomous !== undefined)
    values.push(`autonomous ${metadata.autonomous ? 'yes' : 'no'}`);
  if (metadata.protocolCategory) values.push(`protocol ${metadata.protocolCategory}`);
  if (metadata.payloadLen !== undefined) values.push(`bytes ${metadata.payloadLen}`);
  if (metadata.sequence !== undefined) values.push(`sequence ${metadata.sequence}`);
  if (metadata.skipped !== undefined) values.push(`skipped ${metadata.skipped}`);
  if (metadata.reported) values.push(`reported ${metadata.reported}`);
  return values;
}

export function ActivityPage(): ReactElement {
  const [events, setEvents] = useState<ActivityEventDto[]>([]);
  const [severity, setSeverity] = useState<Filter<ActivitySeverity>>('all');
  const [source, setSource] = useState<Filter<ActivitySource>>('all');
  const [status, setStatus] = useState<LogStatus>('starting');
  const [query, setQuery] = useState('');

  useEffect(() => {
    let active = true;
    let unlisten: Unlisten | undefined;
    const setup = async (): Promise<void> => {
      try {
        const handle = await onActivityLog((event) => {
          if (active) setEvents((current) => mergeEvents(current, [event]));
        });
        if (!active) {
          handle();
          return;
        }
        unlisten = handle;
        setStatus('live');
        const history = await getActivityLog(VIEW_LIMIT);
        if (active) setEvents((current) => mergeEvents(history, current));
      } catch {
        if (active) setStatus('unavailable');
      }
    };
    void setup();
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return events
      .filter(
        (event) =>
          (severity === 'all' || event.severity === severity) &&
          (source === 'all' || event.source === source) &&
          (!needle ||
            event.summary.toLowerCase().includes(needle) ||
            humanize(event.type).toLowerCase().includes(needle)),
      )
      .reverse();
  }, [events, severity, source, query]);
  const t = strings.log;
  const filteredOut = severity !== 'all' || source !== 'all' || query !== '';
  const clear = (): void => {
    setSeverity('all');
    setSource('all');
    setQuery('');
  };

  return (
    <Page
      title={t.heading}
      description={t.description}
      actions={
        <p
          role="status"
          aria-live="polite"
          className={cn(
            'flex items-center gap-2 rounded-full px-3 py-1 text-xs font-medium',
            status === 'live' ? 'bg-success/10 text-success' : 'bg-muted text-muted-foreground',
          )}
        >
          <span
            aria-hidden="true"
            className={cn(
              'size-2 rounded-full bg-current',
              status === 'live' && 'motion-safe:animate-pulse',
            )}
          />
          {t.status[status]}
        </p>
      }
    >
      <Card className="gap-0 py-0">
        <CardHeader className="border-b py-3">
          <div
            role="group"
            aria-label={t.filters.label}
            className="flex flex-wrap items-center gap-2"
          >
            <ToggleGroup
              aria-label={t.filters.severity}
              value={[severity]}
              onValueChange={(value) => {
                const next = value[0] as Filter<ActivitySeverity> | undefined;
                if (next) setSeverity(next);
              }}
              variant="outline"
              size="sm"
            >
              <ToggleGroupItem value="all">{t.filters.all}</ToggleGroupItem>
              {SEVERITIES.map((candidate) => (
                <ToggleGroupItem key={candidate} value={candidate}>
                  {t.severities[candidate]}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
            <Select
              value={source}
              onValueChange={(value) => setSource((value ?? 'all') as Filter<ActivitySource>)}
              items={[
                { value: 'all', label: t.filters.allSources },
                ...SOURCES.map((candidate) => ({
                  value: candidate,
                  label: t.sources[candidate],
                })),
              ]}
            >
              <SelectTrigger size="sm" aria-label={t.filters.source} className="min-w-36">
                <ListFilter aria-hidden="true" className="text-muted-foreground" />
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">{t.filters.allSources}</SelectItem>
                {SOURCES.map((candidate) => (
                  <SelectItem key={candidate} value={candidate}>
                    {t.sources[candidate]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <div className="relative min-w-40 flex-1">
              <Search
                aria-hidden="true"
                className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
              />
              <Input
                type="search"
                aria-label={t.search}
                placeholder={t.search}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                className="h-7 pl-8"
              />
            </div>
            {events.length > 0 ? (
              <span className="ml-auto text-xs text-muted-foreground tabular-nums">
                {format(t.count, { shown: filtered.length, total: events.length })}
              </span>
            ) : null}
          </div>
        </CardHeader>
        <CardContent className="px-0">
          {events.length === 0 ? (
            <Empty className="py-12">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Info aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>{t.empty}</EmptyTitle>
                <EmptyDescription>{t.emptyHint}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ) : filtered.length === 0 ? (
            <Empty className="py-12">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Search aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>{t.noMatches}</EmptyTitle>
              </EmptyHeader>
              {filteredOut ? (
                <EmptyContent>
                  <Button variant="outline" size="sm" onClick={clear}>
                    <X data-icon="inline-start" aria-hidden="true" />
                    {t.clearFilters}
                  </Button>
                </EmptyContent>
              ) : null}
            </Empty>
          ) : (
            <ScrollArea className="max-h-[calc(100vh-16rem)] min-h-64">
              <div role="log" aria-live="polite" aria-label={t.live}>
                <Table>
                  <TableHeader className="sticky top-0 z-10 bg-card">
                    <TableRow>
                      <TableHead className="w-24 pl-4">{t.columns.time}</TableHead>
                      <TableHead>{t.columns.event}</TableHead>
                      <TableHead className="w-28">{t.columns.source}</TableHead>
                      <TableHead className="w-28">{t.columns.result}</TableHead>
                      <TableHead className="pr-4">{t.columns.details}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {filtered.map((event) => {
                      const { Icon, className } = SEVERITY_STYLE[event.severity];
                      return (
                        <TableRow key={event.id} data-event-id={event.id}>
                          <TableCell className="pl-4 align-top text-xs text-muted-foreground tabular-nums">
                            <time dateTime={event.at} title={event.at}>
                              {clock(event.at)}
                            </time>
                          </TableCell>
                          <TableCell className="align-top whitespace-normal">
                            <div className="flex items-start gap-2">
                              <Icon
                                className={cn('mt-0.5 size-4 shrink-0', className)}
                                aria-label={t.severities[event.severity]}
                              />
                              <div className="min-w-0">
                                <p className="font-medium">{humanize(event.type)}</p>
                                <p
                                  className={cn(
                                    'text-muted-foreground',
                                    // Hide a summary that only restates the title (still read aloud).
                                    sameText(event) && 'sr-only',
                                  )}
                                >
                                  <span className="sr-only">{event.type}: </span>
                                  {event.summary}
                                </p>
                              </div>
                            </div>
                          </TableCell>
                          <TableCell className="align-top">
                            <Badge variant="secondary">{t.sources[event.source]}</Badge>
                          </TableCell>
                          <TableCell className="align-top text-xs">
                            {humanize(event.outcome)}
                          </TableCell>
                          <TableCell className="pr-4 align-top">
                            <div className="flex flex-wrap gap-1">
                              {details(event).map((detail) => (
                                <span
                                  key={detail}
                                  className="rounded-md bg-muted px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground"
                                >
                                  {detail}
                                </span>
                              ))}
                            </div>
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </div>
            </ScrollArea>
          )}
        </CardContent>
      </Card>
      <p className="text-xs text-muted-foreground">{t.note}</p>
    </Page>
  );
}
