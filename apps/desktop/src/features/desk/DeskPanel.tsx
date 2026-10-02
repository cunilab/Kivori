import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { CheckCircle2, CircleHelp, LoaderCircle, TriangleAlert } from 'lucide-react';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '../../components/ui/card';
import { getDeskStatus, onDeskStatus, setDisplayMode, type Unlisten } from '../../lib/ipc';
import { DISPLAY_MODES } from '../../lib/ipc/types';
import type { DeskActionDto, DeskStatusDto, DisplayMode } from '../../lib/ipc/types';
import { strings } from '../../lib/i18n/strings';

const t = strings.desk;

/** Native rejections arrive as plain strings; show them verbatim. */
export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function percent(value: number | null | undefined): string {
  return value === null || value === undefined ? t.unknown : `${value}%`;
}

// `tone` is the honest-result contract: only a confirmed/started outcome is "success". An
// unverified action is neutral and carries no check mark.
const RESULT_STYLE = {
  processing: { tone: 'neutral', variant: 'secondary', Icon: LoaderCircle },
  stateConfirmed: { tone: 'success', variant: 'default', Icon: CheckCircle2 },
  executionConfirmed: { tone: 'success', variant: 'default', Icon: CheckCircle2 },
  unverified: { tone: 'neutral', variant: 'outline', Icon: CircleHelp },
  error: { tone: 'error', variant: 'destructive', Icon: TriangleAlert },
} as const;

function LastAction({ last }: { last: DeskActionDto | null | undefined }): ReactElement {
  if (!last) return <span data-testid="last-action">{t.noLastAction}</span>;
  const { tone, variant, Icon } = RESULT_STYLE[last.result];
  return (
    <span data-testid="last-action" className="flex flex-wrap items-center gap-2">
      <span>{t.actions[last.action]}</span>
      <Badge variant={variant} data-result={last.result} data-tone={tone}>
        <Icon data-icon="inline-start" aria-hidden="true" />
        {t.results[last.result]}
      </Badge>
    </span>
  );
}

/** Overview "Desk" panel: display-mode selector, live readouts (null is always "—"), last action. */
export function DeskPanel(): ReactElement {
  const [status, setStatus] = useState<DeskStatusDto | null>(null);
  const [modeError, setModeError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: Unlisten = () => {};
    void getDeskStatus()
      .then((snapshot) => {
        if (active) setStatus((current) => current ?? snapshot);
      })
      .catch(() => {});
    void onDeskStatus((snapshot) => {
      if (active) setStatus(snapshot);
    })
      .then((handle) => {
        if (active) unlisten = handle;
        else handle();
      })
      .catch(() => {});
    return () => {
      active = false;
      unlisten();
    };
  }, []);

  const choose = (mode: DisplayMode): void => {
    setModeError(null);
    void setDisplayMode(mode).catch((error: unknown) => setModeError(errorText(error)));
  };

  const media = status?.media;
  return (
    <section aria-labelledby="desk-heading" className="desk-panel">
      <Card>
        <CardHeader>
          <CardTitle id="desk-heading">{t.heading}</CardTitle>
          <CardDescription>{t.description}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div role="group" aria-label={t.mode} className="flex flex-wrap gap-2">
            {DISPLAY_MODES.map((mode) => (
              <Button
                key={mode}
                type="button"
                variant={status?.mode === mode ? 'default' : 'outline'}
                aria-pressed={status?.mode === mode}
                onClick={() => choose(mode)}
              >
                {t.modes[mode]}
              </Button>
            ))}
          </div>
          {modeError ? (
            <p role="alert" className="text-sm text-destructive">
              {modeError}
            </p>
          ) : null}

          <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <div>
              <dt className="text-xs text-muted-foreground">{t.volume}</dt>
              <dd className="mt-1 font-medium" data-testid="desk-volume">
                {percent(status?.volumePercent)}
                {status?.muted === true ? ` · ${t.muted}` : ''}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">{t.media}</dt>
              <dd className="mt-1 font-medium" data-testid="desk-media">
                {status === null ? t.unknown : media ? t.mediaStates[media] : t.mediaUnobservable}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">{t.cpu}</dt>
              <dd className="mt-1 flex items-center gap-2 font-medium" data-testid="desk-cpu">
                {percent(status?.cpuPercent)}
                {status?.highLoad ? <Badge variant="destructive">{t.highLoad}</Badge> : null}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">{t.ram}</dt>
              <dd className="mt-1 font-medium" data-testid="desk-ram">
                {percent(status?.ramPercent)}
              </dd>
            </div>
          </dl>

          <p className="text-sm" data-testid="desk-bindings">
            {t.bindings
              .replace('{press}', status ? t.actions[status.pressAction] : t.unknown)
              .replace('{hold}', status ? t.actions[status.holdAction] : t.unknown)}
          </p>

          <div className="text-sm">
            <span className="mr-2 text-muted-foreground">{t.lastAction}</span>
            <LastAction last={status?.lastAction} />
          </div>
          {status?.lastAction?.permissionRequired ? (
            <p role="status" className="text-sm text-muted-foreground">
              {t.permission}
            </p>
          ) : null}
        </CardContent>
      </Card>
    </section>
  );
}
