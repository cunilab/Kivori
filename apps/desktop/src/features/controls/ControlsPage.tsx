import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import {
  AppWindow,
  ChevronDown,
  CircleCheck,
  CircleDot,
  CircleHelp,
  Ellipsis,
  Pencil,
  Pin,
  Play,
  Plus,
  SlidersHorizontal,
  Workflow,
  Hand,
  Lock,
  MousePointerClick,
  RefreshCw,
  RotateCw,
  TimerReset,
  TriangleAlert,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@kivori/ui/components/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@kivori/ui/components/alert-dialog';
import { Badge } from '@kivori/ui/components/badge';
import { Button } from '@kivori/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@kivori/ui/components/dropdown-menu';
import {
  Item,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
} from '@kivori/ui/components/item';
import { Skeleton } from '@kivori/ui/components/skeleton';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@kivori/ui/components/tabs';
import { Tooltip, TooltipContent, TooltipTrigger } from '@kivori/ui/components/tooltip';
import { cn } from '@kivori/ui/lib/utils';
import { Page } from '@/components/kivori/page';
import { PageTabs } from '@/components/kivori/page-tabs';
import { ProductArt } from '@/components/kivori/product-art';
import type { DeviceControl } from '@/components/kivori/device-visual';
import { DeviceScreen } from '@/components/kivori/device-screen';
import { ResultBadge } from '@/components/kivori/result-badge';
import {
  deleteMacro,
  getStartupSettings,
  listActionCatalog,
  resetConfig,
  resetProfile,
  setBinding,
  setRotate,
  testAction,
  testRotate,
} from '@/lib/ipc';
import { DESK_RESULTS, MACRO_LIMITS } from '@/lib/ipc/types';
import type {
  ActionCatalogEntryDto,
  ActionSpec,
  ConfigDto,
  ControlRef,
  MacroSpec,
  DeskResult,
  DeskStatusDto,
  ProfileConfigDto,
  ProfileId,
  RotateSpec,
  SlotDto,
  StartupPlatform,
  StepSpec,
} from '@/lib/ipc/types';
import { controlSupported } from '@/lib/device-support';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { mascotUrl } from '@/lib/brand';
import { format, strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';
import { GnomeFocusHelp } from '../permissions/GnomeFocusHelp';
import { AccessibilityHelp } from '../permissions/AccessibilityHelp';
import { KeyboardShortcutsStatus } from '../permissions/KeyboardShortcutsStatus';
import { needsKeyEvents } from '../permissions/needs-key-events';
import { ActionPicker, catalogId } from './ActionPicker';
import { NewProfileArt } from './illustrations';
import { MacroEditor, describeStep } from './MacroEditor';
import { macroTemplates } from './macro-templates';
import { macroUsage } from './macro-usage';
import { ProfileRail } from './ProfileRail';
import { ProfilesCard } from './ProfilesCard';
import type { PickerTarget } from './ActionPicker';
import './controls-motion.css';

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
  const { rotate, press, buttons } = profile;
  const holdRow = (i: 0 | 1 | 2, key: GestureKey, control: ControlRef): Row => {
    const slot = buttons[i].hold;
    // The middle button's Hold mutes every microphone; it cannot be rebound.
    return slot === 'micMute'
      ? { key, Icon: Hand, control: null, fixed: t.micMuteFixed }
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
    // The device's own profile picker; nothing to bind (like Double press).
    { key: 'heldRotate', Icon: RotateCw, control: null, fixed: t.profilePickerFixed },
    slotRow('press', MousePointerClick, 'press', press),
    { key: 'doublePress', Icon: RefreshCw, control: null, fixed: t.doublePressFixed },
    { key: 'hold', Icon: Hand, control: null, fixed: t.knobHoldFixed },
    slotRow('button1', CircleDot, 'button1Press', buttons[0].press),
    slotRow('button2', CircleDot, 'button2Press', buttons[1].press),
    slotRow('button3', CircleDot, 'button3Press', buttons[2].press),
    holdRow(0, 'button1Hold', 'button1Hold'),
    holdRow(1, 'button2Hold', 'button1Hold'),
    holdRow(2, 'button3Hold', 'button3Hold'),
  ];
}

/** "Keyboard shortcut: Ctrl+R" style summary of one binding. */
function describe(spec: ActionSpec | RotateSpec | null, macros: MacroSpec[]): string {
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
    case 'macro':
      return `${entry}: ${macros.find((m) => m.id === spec.id)?.name ?? spec.id}`;
    default:
      return entry;
  }
}

/** A Test target: a discrete action, or the knob's binding (`kind`s never overlap). */
type TestSpec = ActionSpec | RotateSpec;

function isRotateSpec(spec: TestSpec): spec is RotateSpec {
  return spec.kind === 'systemVolume' || spec.kind === 'shortcuts' || spec.kind === 'appVolume';
}

function isKeyboard(
  spec: TestSpec,
  catalog: ActionCatalogEntryDto[] | null,
  macros: MacroSpec[],
): boolean {
  // Only a shortcut pair sends keys; a volume knob injects none.
  if (isRotateSpec(spec)) return spec.kind === 'shortcuts';
  // A macro with a shortcut step needs the countdown too.
  if (spec.kind === 'macro') {
    return (
      macros
        .find((m) => m.id === spec.id)
        ?.steps.some((s) => s.kind === 'action' && isKeyboard(s.action, catalog, macros)) ?? false
    );
  }
  const entry = catalog?.find((e) => e.id === catalogId(spec));
  return entry ? entry.scope === 'keyboard' : spec.kind === 'shortcut';
}

/** A subtle mark for how far Kivori can vouch for an action, with the reason on hover or focus. */
function StatusMark({ verification }: { verification: ActionCatalogEntryDto['verification'] }) {
  const text =
    verification in t.status
      ? t.status[verification as keyof typeof t.status]
      : t.verificationHelp[verification];
  const Icon = verification === 'confirmed' ? CircleCheck : CircleHelp;
  return (
    <Tooltip>
      <TooltipTrigger
        type="button"
        aria-label={text}
        data-verification={verification}
        className="inline-flex shrink-0 cursor-help rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <Icon
          aria-hidden="true"
          className={
            verification === 'confirmed'
              ? 'size-3.5 text-success'
              : 'size-3.5 text-muted-foreground'
          }
        />
      </TooltipTrigger>
      <TooltipContent>{text}</TooltipContent>
    </Tooltip>
  );
}

/**
 * Controls: what each control does in each profile. Rows edit the saved config (the page re-renders
 * from `config://changed`, so a failed save leaves the row as it was); Test runs the saved action
 * and shows the outcome returned for that invocation.
 */
export function ControlsPage({
  desk,
  config,
  connection = null,
}: {
  desk: DeskStatusDto | null;
  config: ConfigDto | null;
  connection?: ConnectionStatusDto | null;
}): ReactElement {
  const [chosen, setChosen] = useState<ProfileId | null>(null);
  const [selectedControl, setSelectedControl] = useState<DeviceControl>('knob');
  // The control the pointer or keyboard is on; the device screen previews it until it leaves.
  const [previewControl, setPreviewControl] = useState<DeviceControl | undefined>(undefined);
  const [catalog, setCatalog] = useState<ActionCatalogEntryDto[] | null>(null);
  const [editing, setEditing] = useState<PickerTarget | null>(null);
  // The New profile block under the profile chips.
  const [newOpen, setNewOpen] = useState(false);
  const [confirm, setConfirm] = useState<'profile' | 'all' | null>(null);
  // `undefined` = closed, `null` = a new macro, else the macro being edited.
  const [editingMacro, setEditingMacro] = useState<MacroSpec | null | undefined>(undefined);
  const [macroDraft, setMacroDraft] = useState<{ name: string; steps: StepSpec[] } | undefined>(
    undefined,
  );
  const [createdMacroId, setCreatedMacroId] = useState<string | null>(null);
  const [deletingMacro, setDeletingMacro] = useState<MacroSpec | null>(null);
  const [platform, setPlatform] = useState<StartupPlatform | null>(null);
  const [tested, setTested] = useState<{ row: string; result: DeskResult } | null>(null);
  const invocation = useRef(0);
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

  useEffect(() => {
    let active = true;
    getStartupSettings()
      .then((settings) => {
        if (active) setPlatform(settings.platform);
      })
      .catch(() => {
        if (active) setPlatform('other');
      });
    return () => {
      active = false;
    };
  }, []);

  const stopCountdown = (): void => {
    if (timer.current) clearInterval(timer.current);
    timer.current = null;
    setCountdown(null);
  };

  const selectControl = (control: DeviceControl): void => {
    if (control === selectedControl) return;
    stopCountdown();
    invocation.current += 1;
    setTested(null);
    setSelectedControl(control);
  };

  const has = (id: ProfileId | null | undefined): id is ProfileId =>
    !!id && !!config?.profiles.some((candidate) => candidate.id === id);
  // A profile that was deleted (or not loaded yet) falls back to the active one, then General.
  const selected = has(chosen) ? chosen : has(desk?.profileId) ? desk.profileId : 'general';
  const profile = config?.profiles.find((candidate) => candidate.id === selected) ?? null;
  const macros = config?.macros ?? [];
  const profileRows = profile ? rows(profile) : [];

  useEffect(() => {
    invocation.current += 1;
    setTested(null);
    stopCountdown();
    return () => {
      invocation.current += 1;
      if (timer.current) clearInterval(timer.current);
      timer.current = null;
    };
  }, [config, selected]);

  const runTest = (rowId: string, spec: TestSpec): void => {
    const current = ++invocation.current;
    setTested({ row: rowId, result: 'processing' });
    (isRotateSpec(spec) ? testRotate(spec) : testAction(spec)).then(
      (outcome) => {
        if (current === invocation.current) setTested({ row: rowId, result: outcome.result });
      },
      (error: unknown) => {
        if (current !== invocation.current) return;
        setTested({ row: rowId, result: 'error' });
        toast.error(t.testFailed, { description: errorText(error) });
      },
    );
  };
  const startTest = (rowId: string, spec: TestSpec): void => {
    stopCountdown();
    invocation.current += 1;
    setTested(null);
    if (!isKeyboard(spec, catalog, macros)) {
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
  const removeMacro = (target: MacroSpec): void => {
    deleteMacro(target.id).catch((error: unknown) =>
      toast.error(strings.controls.macros.deleteFailed, { description: errorText(error) }),
    );
  };
  const doDeleteMacro = (): void => {
    const target = deletingMacro;
    setDeletingMacro(null);
    if (!target || !config || macroUsage(config, target.id).length > 0) return;
    removeMacro(target);
  };
  const doReset = (): void => {
    const scope = confirm;
    setConfirm(null);
    (scope === 'all' ? resetConfig() : resetProfile(selected)).catch((error: unknown) =>
      toast.error(t.resetFailed, { description: errorText(error) }),
    );
  };

  const renderRow = (entry: ProfileConfigDto, row: Row, quiet = false): ReactElement => {
    const rowId = `${entry.id}:${row.key}`;
    const bound = row.spec ?? null;
    // Test runs the saved binding once: a discrete action, or for the knob one step up and back
    // (a shortcut pair sends its clockwise key).
    const testable: TestSpec | null = bound;
    const counting = countdown?.row === rowId;
    const entryInfo = bound ? catalog?.find((e) => e.id === catalogId(bound)) : undefined;
    const name = t.gestures[row.key].name;
    const unbound = row.control !== null && (row.spec === null || row.spec === undefined);
    const flashed =
      tested?.row === rowId &&
      (tested.result === 'stateConfirmed' || tested.result === 'executionConfirmed');
    const edit = (): void => {
      const { control } = row;
      if (control === null) return;
      setEditing({ profile: entry, control, name, spec: bound, label: row.label ?? null });
    };
    const text = row.fixed ?? describe(bound, macros);
    const deviceNote =
      bound && row.deviceLabel && row.deviceLabel !== text
        ? format(t.deviceLabel, { label: row.deviceLabel })
        : null;
    return (
      <li
        key={row.key}
        data-testid={`gesture-${row.key}`}
        className={cn(
          'flex items-center gap-3 px-3 py-2.5',
          !quiet && row.control !== null && 'transition-colors hover:bg-muted/50',
          flashed && 'motion-safe:animate-[kivori-row-flash_600ms_ease-out]',
        )}
      >
        {quiet ? (
          <row.Icon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
        ) : (
          <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <row.Icon className="size-4" aria-hidden="true" />
          </div>
        )}
        {/* The text is a large click target for Edit; the Edit button is the keyboard route. */}
        <div
          className={
            quiet
              ? 'min-w-0 flex-1 text-xs text-muted-foreground'
              : row.control === null
                ? 'min-w-0 flex-1'
                : 'min-w-0 flex-1 cursor-pointer'
          }
          onClick={row.control === null ? undefined : edit}
        >
          <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5">
            <span className={quiet ? 'text-xs' : 'text-xs font-medium text-muted-foreground'}>
              {name}
            </span>
            {connection?.connection === 'connected' &&
            connection.device?.identity?.known === true &&
            !controlSupported(connection, row.key) ? (
              <Badge variant="outline">{strings.savedOnly}</Badge>
            ) : null}
            {row.overridden && <Badge variant="secondary">{t.custom}</Badge>}
            {tested?.row === rowId && <ResultBadge result={tested.result} />}
          </div>
          <div className="flex items-center gap-1.5">
            {unbound ? (
              <Button
                size="sm"
                variant="outline"
                className="border-dashed"
                aria-label={format(t.editRow, { control: name })}
                onClick={(event) => {
                  event.stopPropagation();
                  edit();
                }}
              >
                <Plus data-icon="inline-start" aria-hidden="true" />
                {t.addAction}
              </Button>
            ) : (
              <span className={quiet ? 'text-xs' : 'truncate font-medium'}>{text}</span>
            )}
            {row.control === null && row.key === 'button2Hold' && (
              <Lock
                className="size-3.5 shrink-0 text-muted-foreground"
                aria-label={t.gestures.button2Hold.hint}
              />
            )}
            {entryInfo && <StatusMark verification={entryInfo.verification} />}
          </div>
          {counting ? (
            <p role="status" className="text-xs">
              {format(t.countdown, { seconds: countdown.left })}{' '}
              <span className="text-muted-foreground">{t.countdownHint}</span>
            </p>
          ) : (
            <p className="text-xs text-muted-foreground">
              {t.gestures[row.key].hint}
              {row.key === 'heldRotate' &&
              connection?.connection === 'connected' &&
              connection.device?.identity?.known === true &&
              !controlSupported(connection, 'heldRotate')
                ? ` ${t.heldRotateUnsupported}`
                : ''}
              {deviceNote ? ` ${deviceNote}` : ''}
            </p>
          )}
        </div>
        {row.control !== null && !quiet && (
          <div className="flex shrink-0 items-center gap-1">
            {counting && (
              <Button size="sm" variant="ghost" onClick={stopCountdown}>
                {t.cancelTest}
              </Button>
            )}
            {testable && !counting ? (
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={format(t.testRow, { control: name })}
                title={t.test}
                onClick={() => startTest(rowId, testable)}
              >
                <Play aria-hidden="true" />
              </Button>
            ) : null}
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={format(t.editRow, { control: name })}
              title={t.edit}
              onClick={edit}
            >
              <Pencil aria-hidden="true" />
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    aria-label={format(t.moreActions, { control: name })}
                  />
                }
              >
                <Ellipsis aria-hidden="true" />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-auto min-w-32">
                <DropdownMenuItem
                  disabled={!row.overridden}
                  onClick={() => row.control && resetRow(row.control)}
                >
                  {t.reset}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        )}
      </li>
    );
  };

  const renderGroup = (entry: ProfileConfigDto, title: string, list: Row[]): ReactElement => (
    <section className="space-y-1.5">
      <h2 className="px-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {title}
      </h2>
      <ul aria-label={title} className="divide-y rounded-xl border bg-card">
        {list.map((row) => renderRow(entry, row))}
      </ul>
    </section>
  );

  const renderBuiltIn = (entry: ProfileConfigDto, list: Row[]): ReactElement | null =>
    list.length === 0 ? null : (
      <section className="space-y-1.5">
        <h3 className="px-1 text-xs font-medium text-muted-foreground">{t.builtInTitle}</h3>
        <ul aria-label={t.builtInTitle}>{list.map((row) => renderRow(entry, row, true))}</ul>
      </section>
    );

  const notes = (
    <details data-testid="controls-notes" className="group text-sm text-muted-foreground" open>
      <summary className="inline-flex cursor-pointer list-none items-center gap-1 rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden">
        {t.howTitle}
        <ChevronDown
          className="size-3.5 transition-transform group-open:rotate-180"
          aria-hidden="true"
        />
      </summary>
      <div className="mt-2 space-y-2 rounded-lg bg-muted px-3 py-2.5 text-xs">
        <p>{strings.savedConfiguration}</p>
        <p data-testid="mic-scope-note">{t.micScope}</p>
        <p>{t.holdNote}</p>
        <p>{t.rotateTestNote}</p>
      </div>
    </details>
  );

  const loadingRows = (
    <div className="space-y-2" aria-label={t.loading}>
      {Array.from({ length: 6 }, (_, i) => (
        <Skeleton key={i} className="h-14 rounded-xl" />
      ))}
    </div>
  );

  const controlsTab = (
    <>
      <div
        data-testid="active-profile"
        className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm"
      >
        <AppWindow className="size-4 shrink-0 text-primary" aria-hidden="true" />
        <div className="flex min-w-0 max-w-full items-center gap-2">
          <p className="shrink-0 text-xs whitespace-nowrap text-muted-foreground">{t.profile}:</p>
          {desk ? (
            <p className="truncate font-medium">{desk.profile ?? t.profileGeneral}</p>
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
        <p className="min-w-0 flex-1 basis-56 text-xs text-muted-foreground">
          {desk?.pinned ? t.pinnedHint : t.followHint}
        </p>
      </div>

      <GnomeFocusHelp />
      <KeyboardShortcutsStatus hideWhenEnabled />

      {needsKeyEvents(config) ? (
        <AccessibilityHelp
          title={strings.accessibility.controlsTitle}
          body={strings.accessibility.controlsBody}
        />
      ) : null}

      {config ? (
        <div className="space-y-4">
          <ProfileRail
            profiles={config.profiles}
            desk={desk}
            selected={selected}
            newOpen={newOpen}
            onSelect={setChosen}
            onNew={() => setNewOpen((open) => !open)}
          />
          {newOpen && profile ? (
            <div data-testid="new-profile" className="space-y-3 rounded-xl border bg-card p-3">
              <div className="flex items-center gap-3">
                <NewProfileArt />
                <img src={mascotUrl} alt="" className="size-12" />
              </div>
              <ProfilesCard
                config={config}
                desk={desk}
                profile={profile}
                onShow={(id) => {
                  setChosen(id);
                  setNewOpen(false);
                }}
                part="new"
              />
            </div>
          ) : null}
          {profile ? (
            <ProfilesCard
              config={config}
              desk={desk}
              profile={profile}
              onShow={setChosen}
              part="profile"
            >
              <div
                key={profile.id}
                className="grid items-start gap-6 motion-safe:animate-in motion-safe:fade-in motion-safe:duration-200 min-[900px]:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)]"
              >
                <div className="mx-auto w-full max-w-sm space-y-3 min-[900px]:sticky min-[900px]:top-4 min-[900px]:max-w-none">
                  <ProductArt
                    connection={connection}
                    selectedControl={selectedControl}
                    onSelectControl={selectControl}
                    onPreviewControl={setPreviewControl}
                    screen={<DeviceScreen activeControl={previewControl ?? selectedControl} />}
                  />
                  <p className="text-center text-xs text-muted-foreground">{t.workbench.hint}</p>
                </div>
                <div className="min-w-0 space-y-3">
                  {profile.custom ? (
                    <p className="text-xs text-muted-foreground">{t.customProfileNote}</p>
                  ) : null}
                  <Tabs
                    value={selectedControl}
                    onValueChange={(value) => selectControl(value as DeviceControl)}
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <h2 className="text-sm font-semibold">{t.workbench.title}</h2>
                      <TabsList
                        aria-label={t.workbench.title}
                        className="grid h-auto grid-cols-4 gap-0.5"
                      >
                        <TabsTrigger value="knob">{t.groups.knob}</TabsTrigger>
                        <TabsTrigger value="button1" aria-label={t.gestures.button1.name}>
                          {t.workbench.left}
                        </TabsTrigger>
                        <TabsTrigger value="button2" aria-label={t.gestures.button2.name}>
                          {t.workbench.middle}
                        </TabsTrigger>
                        <TabsTrigger value="button3" aria-label={t.gestures.button3.name}>
                          {t.workbench.right}
                        </TabsTrigger>
                      </TabsList>
                    </div>
                    {(
                      [
                        ['knob', t.groups.knob, profileRows.slice(0, 5)],
                        ['button1', t.gestures.button1.name, [profileRows[5]!, profileRows[8]!]],
                        ['button2', t.gestures.button2.name, [profileRows[6]!, profileRows[9]!]],
                        ['button3', t.gestures.button3.name, [profileRows[7]!, profileRows[10]!]],
                      ] as const
                    ).map(([control, title, entries]) => {
                      const editable = entries.filter((row) => row.control !== null);
                      const fixed = entries.filter((row) => row.control === null);
                      return (
                        <TabsContent
                          key={control}
                          value={control}
                          className="mt-3 space-y-3 motion-safe:animate-in motion-safe:fade-in motion-safe:slide-in-from-bottom-1 motion-safe:duration-200"
                        >
                          {editable.length > 0 ? renderGroup(profile, title, editable) : null}
                          {renderBuiltIn(profile, fixed)}
                        </TabsContent>
                      );
                    })}
                  </Tabs>
                </div>
              </div>
            </ProfilesCard>
          ) : null}
        </div>
      ) : (
        loadingRows
      )}
    </>
  );

  const mm = t.macros;
  const atMacroLimit = !config || macros.length >= MACRO_LIMITS.macros;
  const macroTemplateList = macroTemplates(platform);

  const macrosTab = (
    <Card data-testid="macros">
      <CardHeader>
        <CardTitle>{mm.title}</CardTitle>
        <CardDescription>{mm.description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {macros.length === 0 ? (
          <div className="flex items-center gap-3">
            <img src={mascotUrl} alt="" className="size-12" />
            <div className="space-y-1">
              <p className="text-sm text-muted-foreground">{mm.empty}</p>
              <p className="text-sm text-muted-foreground">{mm.emptyHint}</p>
            </div>
          </div>
        ) : (
          <ul className="space-y-2" aria-label={mm.title}>
            {macros.map((macro) => {
              const rowId = `macro:${macro.id}`;
              const uses = config ? macroUsage(config, macro.id) : [];
              const blockId = `macro-${macro.id}-delete-reason`;
              const pending = countdown && countdown.row === rowId ? countdown : null;
              return (
                <li
                  key={macro.id}
                  data-testid={`macro-${macro.id}`}
                  className="flex flex-wrap items-center gap-2 rounded-lg bg-muted px-3 py-2.5"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="font-medium">{macro.name}</p>
                      {tested?.row === rowId && <ResultBadge result={tested.result} />}
                    </div>
                    <p className="truncate text-xs text-muted-foreground">
                      {macro.steps.map(describeStep).join(' → ')}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {uses.length === 0
                        ? mm.unused
                        : format(mm.usedBy, {
                            list: uses
                              .map((use) => `${use.profileName} · ${use.controlName}`)
                              .join(', '),
                          })}
                    </p>
                    {pending ? (
                      <p role="status" className="text-xs">
                        {format(t.countdown, { seconds: pending.left })}{' '}
                        <span className="text-muted-foreground">{t.countdownHint}</span>
                      </p>
                    ) : null}
                    {uses.length > 0 ? (
                      <p id={blockId} className="text-xs text-muted-foreground">
                        {mm.deleteBlocked}
                      </p>
                    ) : null}
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    {pending ? (
                      <Button size="sm" variant="ghost" onClick={stopCountdown}>
                        {t.cancelTest}
                      </Button>
                    ) : (
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        aria-label={format(mm.testMacro, { name: macro.name })}
                        onClick={() => startTest(rowId, { kind: 'macro', id: macro.id })}
                      >
                        <Play aria-hidden="true" />
                      </Button>
                    )}
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        setMacroDraft(undefined);
                        setEditingMacro(macro);
                      }}
                    >
                      {mm.edit}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={uses.length > 0}
                      aria-describedby={uses.length > 0 ? blockId : undefined}
                      onClick={() => setDeletingMacro(macro)}
                    >
                      {mm.delete}
                    </Button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
        <Button
          variant="outline"
          disabled={atMacroLimit}
          onClick={() => {
            setMacroDraft(undefined);
            setEditingMacro(null);
          }}
        >
          {mm.create}
        </Button>
        <section className="space-y-2">
          <h3 className="text-sm font-medium">{mm.templatesTitle}</h3>
          <div className="grid gap-2 sm:grid-cols-3">
            {macroTemplateList.map((template) => {
              const label = mm.templates[template.key];
              return (
                <button
                  key={template.key}
                  type="button"
                  disabled={atMacroLimit || platform === null}
                  className="rounded-xl border bg-card px-3 py-2.5 text-left outline-none hover:bg-muted/50 focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-60"
                  onClick={() => {
                    setMacroDraft({ name: label.name, steps: template.steps });
                    setEditingMacro(null);
                  }}
                >
                  <span className="block text-sm font-medium">{label.name}</span>
                  <span className="block text-xs text-muted-foreground">{label.description}</span>
                </button>
              );
            })}
          </div>
        </section>
      </CardContent>
    </Card>
  );

  const moreTab = (
    <>
      <div className="flex flex-wrap gap-2">
        <Button variant="outline" disabled={!profile} onClick={() => setConfirm('profile')}>
          {profile?.custom ? t.resetCustomLabel : t.resetProfile}
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

      {notes}

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
    </>
  );

  return (
    <Page title={t.title} description={t.description}>
      {config?.notice ? (
        <Alert data-testid="config-notice">
          <TriangleAlert aria-hidden="true" />
          <AlertTitle>{t.notice[config.notice].title}</AlertTitle>
          <AlertDescription>{t.notice[config.notice].body}</AlertDescription>
        </Alert>
      ) : null}

      <PageTabs
        id="controls"
        label={t.tabsLabel}
        tabs={[
          {
            value: 'controls',
            label: t.tabs.controls,
            Icon: SlidersHorizontal,
            content: controlsTab,
          },
          { value: 'macros', label: t.tabs.macros, Icon: Workflow, content: macrosTab },
          { value: 'more', label: t.tabs.more, Icon: CircleHelp, content: moreTab },
        ]}
      />

      <ActionPicker
        target={editing}
        catalog={catalog}
        macros={macros}
        newMacroId={createdMacroId}
        onNewMacro={() => {
          setMacroDraft(undefined);
          setEditingMacro(null);
        }}
        onClose={() => {
          setEditing(null);
          setCreatedMacroId(null);
        }}
        onSaveSlot={async (target, slot) => {
          if (target.control === 'rotate' || target.control === 'step' || !target.profile) return;
          await setBinding(target.profile.id, target.control, slot);
        }}
        onSaveRotate={async (target, rotate) => {
          if (!target.profile) return;
          await setRotate(target.profile.id, rotate);
        }}
      />

      <MacroEditor
        macro={editingMacro}
        macros={macros}
        catalog={catalog}
        initial={editingMacro === null ? macroDraft : undefined}
        onSaved={(id) => {
          if (editing) setCreatedMacroId(id);
        }}
        onClose={() => {
          setEditingMacro(undefined);
          setMacroDraft(undefined);
        }}
      />

      <AlertDialog open={confirm !== null} onOpenChange={(open) => !open && setConfirm(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirm === 'all'
                ? t.resetEverythingTitle
                : format(profile?.custom ? t.resetCustomTitle : t.resetProfileTitle, {
                    profile: profile?.name ?? '',
                  })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {confirm === 'all'
                ? t.resetEverythingBody
                : profile?.custom
                  ? t.resetCustomBody
                  : t.resetProfileBody}
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

      <AlertDialog
        open={deletingMacro !== null}
        onOpenChange={(open) => !open && setDeletingMacro(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {format(mm.deleteTitle, { name: deletingMacro?.name ?? '' })}
            </AlertDialogTitle>
            <AlertDialogDescription>{mm.deleteBody}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{mm.keep}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={doDeleteMacro}>
              {mm.confirmDelete}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Page>
  );
}
