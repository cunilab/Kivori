import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import {
  AppWindow,
  CircleDot,
  Pin,
  Hand,
  Lock,
  MousePointerClick,
  RefreshCw,
  RotateCw,
  TimerReset,
  TriangleAlert,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Item, ItemContent, ItemDescription, ItemGroup, ItemMedia } from '@/components/ui/item';
import { Skeleton } from '@/components/ui/skeleton';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Page } from '@/components/kivori/page';
import { ResultBadge } from '@/components/kivori/result-badge';
import {
  listActionCatalog,
  resetConfig,
  resetProfile,
  setBinding,
  setRotate,
  testAction,
} from '@/lib/ipc';
import { DESK_RESULTS, PROFILE_IDS } from '@/lib/ipc/types';
import type {
  ActionCatalogEntryDto,
  ActionSpec,
  ConfigDto,
  ControlRef,
  DeskStatusDto,
  ProfileConfigDto,
  ProfileId,
  RotateSpec,
  SlotDto,
} from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';
import { ActionPicker, catalogId } from './ActionPicker';
import type { PickerTarget } from './ActionPicker';

const t = strings.controls;

/** How long a keyboard test waits, so the user can click the app that should receive the keys. */
export const TEST_COUNTDOWN_SECONDS = 3;

type GestureKey = keyof typeof t.gestures;

interface Row {
  key: GestureKey;
  Icon: LucideIcon;
  /** What `setBinding` / `setRotate` edits; `null` = not editable. */
  control: ControlRef | 'rotate' | null;
  /** The bound action (`null` = unbound), or `undefined` for rows that are not bindings. */
  spec?: ActionSpec | RotateSpec | null;
  label?: string | null;
  deviceLabel?: string;
  overridden?: boolean;
  /** Fixed text for rows that cannot be edited. */
  fixed?: string;
}

function slotRow(key: GestureKey, Icon: LucideIcon, control: ControlRef, slot: SlotDto): Row {
  return {
    key,
    Icon,
    control,
    spec: slot.action,
    label: slot.label,
    deviceLabel: slot.deviceLabel,
    overridden: slot.overridden,
  };
}

function rows(profile: ProfileConfigDto): Row[] {
  const { rotate, press, hold, buttons } = profile;
  const holdRow = (i: 0 | 1 | 2, key: GestureKey, control: ControlRef): Row => {
    const slot = buttons[i].hold;
    // The middle button's Hold pins a profile; it can never be rebound.
    return slot === 'pin'
      ? { key, Icon: Hand, control: null, fixed: t.pinProfile }
      : slotRow(key, Hand, control, slot);
  };
  return [
    {
      key: 'rotate',
      Icon: RotateCw,
      control: 'rotate',
      spec: rotate.spec,
      deviceLabel: rotate.deviceLabel,
      overridden: rotate.overridden,
    },
    slotRow('press', MousePointerClick, 'press', press),
    { key: 'doublePress', Icon: RefreshCw, control: null, fixed: t.doublePressFixed },
    slotRow('hold', Hand, 'hold', hold),
    slotRow('button1', CircleDot, 'button1Press', buttons[0].press),
    slotRow('button2', CircleDot, 'button2Press', buttons[1].press),
    slotRow('button3', CircleDot, 'button3Press', buttons[2].press),
    holdRow(0, 'button1Hold', 'button1Hold'),
    holdRow(1, 'button2Hold', 'button1Hold'),
    holdRow(2, 'button3Hold', 'button3Hold'),
  ];
}

/** "Keyboard shortcut: Ctrl+R" style summary of one binding. */
function describe(spec: ActionSpec | RotateSpec | null): string {
  if (!spec) return t.unbound;
  const entry = t.picker.entries[catalogId(spec) as keyof typeof t.picker.entries];
  switch (spec.kind) {
    case 'shortcut':
      return `${entry}: ${spec.keys}`;
    case 'launch':
      return `${entry}: ${spec.target}`;
    case 'appMute':
    case 'appVolume':
      return `${entry}: ${spec.app}`;
    case 'shortcuts':
      return `${entry}: ${spec.cw} / ${spec.ccw}`;
    default:
      return entry;
  }
}

function isKeyboard(spec: ActionSpec, catalog: ActionCatalogEntryDto[] | null): boolean {
  const entry = catalog?.find((e) => e.id === catalogId(spec));
  return entry ? entry.scope === 'keyboard' : spec.kind === 'shortcut';
}

/**
 * Controls: what each control does in each profile. Rows edit the saved config (the page re-renders
 * from `config://changed`, so a failed save leaves the row as it was); Test runs the saved action
 * and shows the outcome from the desk status.
 */
export function ControlsPage({
  desk,
  config,
}: {
  desk: DeskStatusDto | null;
  config: ConfigDto | null;
}): ReactElement {
  const [chosen, setChosen] = useState<ProfileId | null>(null);
  const [catalog, setCatalog] = useState<ActionCatalogEntryDto[] | null>(null);
  const [editing, setEditing] = useState<PickerTarget | null>(null);
  const [confirm, setConfirm] = useState<'profile' | 'all' | null>(null);
  // The row last tested (its result badge shows desk.lastAction) and the one counting down.
  const [tested, setTested] = useState<string | null>(null);
  const [countdown, setCountdown] = useState<{ row: string; left: number } | null>(null);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    let active = true;
    listActionCatalog()
      .then((entries) => active && setCatalog(entries))
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const stopCountdown = (): void => {
    if (timer.current) clearInterval(timer.current);
    timer.current = null;
    setCountdown(null);
  };
  useEffect(() => () => stopCountdown(), []);

  const selected = chosen ?? desk?.profileId ?? 'general';
  const profile = config?.profiles.find((candidate) => candidate.id === selected) ?? null;

  const runTest = (rowId: string, spec: ActionSpec): void => {
    setTested(rowId);
    testAction(spec).catch((error: unknown) =>
      toast.error(t.testFailed, { description: errorText(error) }),
    );
  };
  const startTest = (rowId: string, spec: ActionSpec): void => {
    stopCountdown();
    if (!isKeyboard(spec, catalog)) {
      runTest(rowId, spec);
      return;
    }
    let left = TEST_COUNTDOWN_SECONDS;
    setCountdown({ row: rowId, left });
    timer.current = setInterval(() => {
      left -= 1;
      if (left > 0) {
        setCountdown({ row: rowId, left });
        return;
      }
      stopCountdown();
      runTest(rowId, spec);
    }, 1000);
  };

  const resetRow = (control: ControlRef | 'rotate'): void => {
    const saved =
      control === 'rotate' ? setRotate(selected, null) : setBinding(selected, control, null);
    saved.catch((error: unknown) => toast.error(t.resetFailed, { description: errorText(error) }));
  };
  const doReset = (): void => {
    const scope = confirm;
    setConfirm(null);
    (scope === 'all' ? resetConfig() : resetProfile(selected)).catch((error: unknown) =>
      toast.error(t.resetFailed, { description: errorText(error) }),
    );
  };

  return (
    <Page title={t.title} description={t.description}>
      {config?.notice ? (
        <Alert data-testid="config-notice">
          <TriangleAlert aria-hidden="true" />
          <AlertTitle>{t.notice[config.notice].title}</AlertTitle>
          <AlertDescription>{t.notice[config.notice].body}</AlertDescription>
        </Alert>
      ) : null}

      <Card data-testid="active-profile">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <AppWindow className="size-5" aria-hidden="true" />
            </div>
            <div className="min-w-0 flex-1">
              <CardDescription>{t.profile}</CardDescription>
              {desk ? (
                <CardTitle>{desk.profile ?? t.profileGeneral}</CardTitle>
              ) : (
                <Skeleton className="h-5 w-28" />
              )}
            </div>
            {desk?.pinned && (
              <Badge variant="secondary" className="h-6 gap-1.5 px-2.5">
                <Pin data-icon="inline-start" aria-hidden="true" />
                {t.pinned}
              </Badge>
            )}
          </div>
          <CardDescription>{desk?.pinned ? t.pinnedHint : t.followHint}</CardDescription>
        </CardHeader>
      </Card>

      {config ? (
        <Tabs value={selected} onValueChange={(value) => setChosen(value as ProfileId)}>
          <TabsList aria-label={t.profilesLabel} className="h-auto flex-wrap">
            {PROFILE_IDS.map((id) => {
              const entry = config.profiles.find((candidate) => candidate.id === id);
              return entry ? (
                <TabsTrigger key={id} value={id} data-active-profile={desk?.profileId === id}>
                  {entry.name}
                </TabsTrigger>
              ) : null;
            })}
          </TabsList>
          {config.profiles
            .filter((entry) => entry.id === selected)
            .map((entry) => (
              <TabsContent key={entry.id} value={entry.id} className="space-y-4">
                <ul className="grid gap-4 sm:grid-cols-2" aria-label={t.title}>
                  {rows(entry).map((row) => {
                    const rowId = `${entry.id}:${row.key}`;
                    const bound = row.spec ?? null;
                    // Test runs a discrete action; the knob (Rotate) has nothing to run once.
                    const testable: ActionSpec | null =
                      row.control !== 'rotate' &&
                      bound &&
                      bound.kind !== 'shortcuts' &&
                      bound.kind !== 'systemVolume' &&
                      bound.kind !== 'appVolume'
                        ? bound
                        : null;
                    const counting = countdown?.row === rowId;
                    const entryInfo = bound
                      ? catalog?.find((e) => e.id === catalogId(bound))
                      : undefined;
                    return (
                      <li key={row.key}>
                        <Card className="h-full" data-testid={`gesture-${row.key}`}>
                          <CardHeader>
                            <div className="flex items-center gap-3">
                              <div className="flex size-10 items-center justify-center rounded-xl bg-primary/10 text-primary">
                                <row.Icon className="size-5" aria-hidden="true" />
                              </div>
                              <div>
                                <CardTitle>{t.gestures[row.key].name}</CardTitle>
                                <CardDescription>{t.gestures[row.key].hint}</CardDescription>
                              </div>
                            </div>
                          </CardHeader>
                          <CardContent className="space-y-3">
                            <div className="flex flex-wrap items-center gap-2 rounded-lg bg-muted px-3 py-2.5">
                              <span className="font-medium">{row.fixed ?? describe(bound)}</span>
                              {row.control === null && row.key === 'button2Hold' && (
                                <Lock
                                  className="size-3.5 text-muted-foreground"
                                  aria-label={t.gestures.button2Hold.hint}
                                />
                              )}
                              {entryInfo && (
                                <Badge
                                  variant="outline"
                                  title={t.verificationHelp[entryInfo.verification]}
                                >
                                  {t.verification[entryInfo.verification]}
                                </Badge>
                              )}
                              {row.overridden && <Badge variant="secondary">{t.custom}</Badge>}
                              {tested === rowId && desk?.lastAction && (
                                <ResultBadge result={desk.lastAction.result} />
                              )}
                            </div>
                            {bound && row.deviceLabel && row.deviceLabel !== describe(bound) ? (
                              <p className="text-xs text-muted-foreground">
                                {format(t.deviceLabel, { label: row.deviceLabel })}
                              </p>
                            ) : null}
                            {counting ? (
                              <p role="status" className="text-sm">
                                {format(t.countdown, { seconds: countdown.left })}{' '}
                                <span className="text-muted-foreground">{t.countdownHint}</span>
                              </p>
                            ) : null}
                            {row.control !== null && (
                              <div className="flex flex-wrap gap-2">
                                <Button
                                  size="sm"
                                  variant="outline"
                                  onClick={() => {
                                    const { control } = row;
                                    if (control === null) return;
                                    setEditing({
                                      profile: entry,
                                      control,
                                      name: t.gestures[row.key].name,
                                      spec: bound,
                                      label: row.label ?? null,
                                    });
                                  }}
                                >
                                  {t.edit}
                                </Button>
                                {testable &&
                                  (counting ? (
                                    <Button size="sm" variant="ghost" onClick={stopCountdown}>
                                      {t.cancelTest}
                                    </Button>
                                  ) : (
                                    <Button
                                      size="sm"
                                      variant="outline"
                                      onClick={() => startTest(rowId, testable)}
                                    >
                                      {t.test}
                                    </Button>
                                  ))}
                                <Button
                                  size="sm"
                                  variant="ghost"
                                  disabled={!row.overridden}
                                  onClick={() => row.control && resetRow(row.control)}
                                >
                                  {t.reset}
                                </Button>
                              </div>
                            )}
                          </CardContent>
                        </Card>
                      </li>
                    );
                  })}
                </ul>
              </TabsContent>
            ))}
        </Tabs>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2" aria-label={t.loading}>
          {Array.from({ length: 6 }, (_, i) => (
            <Skeleton key={i} className="h-40 rounded-xl" />
          ))}
        </div>
      )}

      <p className="text-xs text-muted-foreground">{t.holdNote}</p>

      <div className="flex flex-wrap gap-2">
        <Button variant="outline" disabled={!profile} onClick={() => setConfirm('profile')}>
          {t.resetProfile}
        </Button>
        <Button variant="outline" disabled={!config} onClick={() => setConfirm('all')}>
          {t.resetEverything}
        </Button>
      </div>

      <Alert>
        <TimerReset aria-hidden="true" />
        <AlertTitle>{t.recoveryTitle}</AlertTitle>
        <AlertDescription>{t.recovery}</AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>{t.resultsTitle}</CardTitle>
          <CardDescription>{t.resultsBody}</CardDescription>
        </CardHeader>
        <CardContent>
          <ItemGroup className="gap-1">
            {DESK_RESULTS.map((result) => (
              <Item key={result} size="sm" className="px-0">
                <ItemMedia className="w-44 shrink-0 justify-start">
                  <ResultBadge result={result} />
                </ItemMedia>
                <ItemContent>
                  <ItemDescription>{strings.resultHelp[result]}</ItemDescription>
                </ItemContent>
              </Item>
            ))}
          </ItemGroup>
        </CardContent>
      </Card>

      <ActionPicker
        target={editing}
        catalog={catalog}
        onClose={() => setEditing(null)}
        onSaveSlot={async (target, slot) => {
          if (target.control === 'rotate') return;
          await setBinding(target.profile.id, target.control, slot);
        }}
        onSaveRotate={async (target, rotate) => {
          await setRotate(target.profile.id, rotate);
        }}
      />

      <AlertDialog open={confirm !== null} onOpenChange={(open) => !open && setConfirm(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirm === 'all'
                ? t.resetEverythingTitle
                : format(t.resetProfileTitle, { profile: profile?.name ?? '' })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {confirm === 'all' ? t.resetEverythingBody : t.resetProfileBody}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t.keep}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={doReset}>
              {t.confirmReset}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Page>
  );
}
