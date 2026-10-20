import { useEffect, useState } from 'react';
import type { FormEvent, ReactElement, ReactNode } from 'react';
import { toast } from 'sonner';
import { Ellipsis, Plus } from 'lucide-react';
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
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@kivori/ui/components/dropdown-menu';
import { Input } from '@kivori/ui/components/input';
import { Label } from '@kivori/ui/components/label';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@kivori/ui/components/sheet';
import { Switch } from '@kivori/ui/components/switch';
import {
  addCustomProfile,
  assignApp,
  deleteProfile,
  getAppIcon,
  getStartupSettings,
  listRunningApps,
  renameProfile,
  setProfilePinCycle,
} from '@/lib/ipc';
import { PROFILE_LIMITS } from '@/lib/ipc/types';
import type {
  AppRefDto,
  ConfigDto,
  DeskStatusDto,
  ProfileConfigDto,
  ProfileId,
  StartupPlatform,
} from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';
import { friendlyAppName, groupApps } from './app-names';
import type { AppGroup } from './app-names';

/** How many apps a profile shows before "+N more". */
const APPS_SHOWN = 6;

const t = strings.controls.profileList;

/** What an app id reads as: a friendly name (`idea64.exe` -> `IntelliJ IDEA`). */
export function appLabel(id: string): string {
  return friendlyAppName(id);
}

/** The app's icon, or a neutral letter while there is none (or when this OS gives none). */
function AppIcon({ id }: { id: string }): ReactElement {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void getAppIcon(id).then((found) => active && setUrl(found));
    return () => {
      active = false;
    };
  }, [id]);
  return url ? (
    <img src={url} alt="" data-testid={`app-icon-${id}`} className="size-5 shrink-0 rounded" />
  ) : (
    <span
      aria-hidden="true"
      data-testid={`app-avatar-${id}`}
      className="flex size-5 shrink-0 items-center justify-center rounded bg-muted text-[11px] font-medium uppercase text-muted-foreground"
    >
      {[...appLabel(id)][0] ?? '?'}
    </span>
  );
}

/** Lists running apps (or takes a typed id) and hands the choice back. */
function AppPicker({
  target,
  config,
  onPick,
  onClose,
}: {
  /** `undefined` = closed, `null` = a new profile, else the profile receiving the app. */
  target: ProfileConfigDto | null | undefined;
  config: ConfigDto;
  onPick: (app: AppRefDto) => void;
  onClose: () => void;
}): ReactElement {
  const open = target !== undefined;
  const [apps, setApps] = useState<AppRefDto[] | null>(null);
  const [typed, setTyped] = useState('');
  useEffect(() => {
    if (!open) return;
    let active = true;
    setApps(null);
    setTyped('');
    listRunningApps()
      .then((found) => active && setApps(found))
      .catch(() => active && setApps([]));
    return () => {
      active = false;
    };
  }, [open]);
  const owner = (id: string): ProfileConfigDto | undefined =>
    config.profiles.find((p) => p.apps.includes(id));
  const submit = (event: FormEvent): void => {
    event.preventDefault();
    const id = typed.trim().toLowerCase();
    if (id) onPick({ id, name: '' });
  };
  return (
    <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
      <SheetContent className="w-full overflow-y-auto sm:max-w-md">
        <SheetHeader>
          <SheetTitle>{t.pickTitle}</SheetTitle>
          <SheetDescription>
            {target
              ? format(t.pickDescriptionInto, { profile: target.name })
              : t.pickDescriptionNew}
          </SheetDescription>
        </SheetHeader>
        <div className="flex-1 space-y-4 overflow-y-auto px-4">
          <p className="text-xs text-muted-foreground">{t.moveNote}</p>
          {apps === null ? (
            <p role="status" className="text-sm text-muted-foreground">
              {t.pickLoading}
            </p>
          ) : apps.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t.pickEmpty}</p>
          ) : (
            <ul aria-label={t.pickGroup} className="space-y-1">
              {apps.map((app) => {
                const held = owner(app.id);
                return (
                  <li key={app.id}>
                    <Button
                      type="button"
                      variant="ghost"
                      className="h-auto w-full justify-start gap-2 py-2"
                      onClick={() => onPick(app)}
                    >
                      <AppIcon id={app.id} />
                      <span className="min-w-0 flex-1 truncate text-left">{app.name}</span>
                      {held ? (
                        <span className="text-xs text-muted-foreground">
                          {format(t.inProfile, { profile: held.name })}
                        </span>
                      ) : null}
                    </Button>
                  </li>
                );
              })}
            </ul>
          )}
          <form onSubmit={submit} className="space-y-2">
            <Label htmlFor="app-by-name">{t.byNameLabel}</Label>
            <Input
              id="app-by-name"
              value={typed}
              maxLength={128}
              aria-describedby="app-by-name-hint"
              onChange={(e) => setTyped(e.target.value)}
            />
            <p id="app-by-name-hint" className="text-xs text-muted-foreground">
              {t.byNameHint}
            </p>
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={onClose}>
                {t.pickCancel}
              </Button>
              <Button type="submit" disabled={!typed.trim()}>
                {t.pickConfirm}
              </Button>
            </div>
          </form>
        </div>
      </SheetContent>
    </Sheet>
  );
}

/**
 * The selected profile: its name and options, the apps it matches (each app once, however many ids
 * the config stores for it), the controls passed as `children`, and the way to add a profile.
 * Every change saves through the native core; the page re-renders from the config it returns.
 */
export function ProfilesCard({
  config,
  desk,
  profile,
  onShow,
  children,
  part = 'all',
}: {
  config: ConfigDto;
  desk: DeskStatusDto | null;
  profile: ProfileConfigDto;
  onShow: (id: ProfileId) => void;
  children?: ReactNode;
  /** `profile`: the apps panel and children; `new`: only the New profile block; `all`: both. */
  part?: 'all' | 'profile' | 'new';
}): ReactElement {
  const [picking, setPicking] = useState<ProfileConfigDto | null | undefined>(undefined);
  const [renaming, setRenaming] = useState<{ id: ProfileId; value: string } | null>(null);
  const [deleting, setDeleting] = useState<ProfileConfigDto | null>(null);
  const [expanded, setExpanded] = useState<ProfileId | null>(null);
  const [platform, setPlatform] = useState<StartupPlatform | null>(null);
  const customCount = config.profiles.filter((p) => p.custom).length;
  const full = customCount >= PROFILE_LIMITS.customProfiles;
  const recent = desk?.recentApp ?? null;

  useEffect(() => {
    let active = true;
    getStartupSettings()
      .then((settings) => active && setPlatform(settings.platform))
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const fail = (error: unknown): void => {
    toast.error(t.failed, { description: errorText(error) });
  };
  const ownerOf = (id: string): ProfileConfigDto | undefined =>
    config.profiles.find((p) => p.apps.includes(id));

  /** Saves a move and says where the app came from when it came from another profile. */
  const moveApp = (target: ProfileConfigDto, app: AppRefDto): void => {
    const from = ownerOf(app.id.toLowerCase());
    assignApp(target.id, app.id)
      .then(() =>
        toast.success(
          from && from.id !== target.id
            ? format(t.moved, {
                app: app.name || appLabel(app.id),
                from: from.name,
                profile: target.name,
              })
            : format(t.added, { app: app.name || appLabel(app.id), profile: target.name }),
        ),
      )
      .catch(fail);
  };
  /** Moves every stored id of a merged app, so no alias is left behind. */
  const moveGroup = async (target: ProfileConfigDto, group: AppGroup): Promise<void> => {
    try {
      for (const id of group.ids) await assignApp(target.id, id);
      toast.success(
        format(target.id === 'general' ? t.removed : t.moved, {
          app: group.name,
          from: profile.name,
          profile: target.name,
        }),
      );
    } catch (error) {
      fail(error);
    }
  };
  const createFor = (app: AppRefDto): void => {
    const from = ownerOf(app.id.toLowerCase());
    addCustomProfile(app.id, app.name)
      .then((saved) => {
        const made = saved.profiles.at(-1);
        toast.success(
          from
            ? format(t.moved, {
                app: app.name || appLabel(app.id),
                from: from.name,
                profile: made?.name ?? '',
              })
            : format(t.created, { profile: made?.name ?? '' }),
        );
        if (made) onShow(made.id);
      })
      .catch(fail);
  };
  const pick = (app: AppRefDto): void => {
    const target = picking;
    setPicking(undefined);
    if (target) moveApp(target, app);
    else if (target === null) createFor(app);
  };
  const saveName = (event: FormEvent): void => {
    event.preventDefault();
    if (!renaming) return;
    const { id, value } = renaming;
    renameProfile(id, value)
      .then(() => setRenaming(null))
      .catch(fail);
  };
  const nameProblem = (value: string): string | null => {
    const length = [...value.trim()].length;
    if (length === 0) return t.nameEmpty;
    return length > PROFILE_LIMITS.nameMax ? t.nameTooLong : null;
  };

  const groups = groupApps(profile.apps, platform);
  const showAll = expanded === profile.id;
  const visible = showAll ? groups : groups.slice(0, APPS_SHOWN);
  const hidden = groups.length - visible.length;
  const others = config.profiles.filter((p) => p.id !== profile.id);

  return (
    <div data-testid={part === 'new' ? 'new-profile-list' : 'profile-list'} className="space-y-4">
      {part !== 'new' ? (
        <section
          data-testid={`profile-${profile.id}`}
          aria-label={profile.name}
          className="space-y-3 rounded-xl border bg-card p-3 sm:p-4"
        >
          <div className="flex flex-wrap items-center gap-2">
            {renaming?.id === profile.id ? (
              <form onSubmit={saveName} className="flex flex-wrap items-center gap-2">
                <Label htmlFor={`rename-${profile.id}`} className="sr-only">
                  {t.renameLabel}
                </Label>
                <Input
                  id={`rename-${profile.id}`}
                  className="h-8 w-48"
                  value={renaming.value}
                  aria-invalid={nameProblem(renaming.value) !== null}
                  onChange={(e) => setRenaming({ id: profile.id, value: e.target.value })}
                />
                <Button size="sm" type="submit" disabled={nameProblem(renaming.value) !== null}>
                  {t.renameSave}
                </Button>
                <Button size="sm" type="button" variant="ghost" onClick={() => setRenaming(null)}>
                  {t.renameCancel}
                </Button>
                <p role="status" className="basis-full text-xs text-muted-foreground">
                  {nameProblem(renaming.value) ?? t.nameHint}
                </p>
              </form>
            ) : (
              <h2 className="text-base font-semibold">{profile.name}</h2>
            )}
            <Badge variant="outline">{profile.custom ? t.yourProfile : t.builtIn}</Badge>
            <div className="ml-auto flex items-center gap-2">
              {profile.custom ? (
                <div className="flex items-center gap-2">
                  <Switch
                    id={`cycle-${profile.id}`}
                    checked={profile.inPinCycle}
                    aria-describedby={`cycle-hint-${profile.id}`}
                    onCheckedChange={(on) => {
                      setProfilePinCycle(profile.id, on).catch(fail);
                    }}
                  />
                  <Label htmlFor={`cycle-${profile.id}`} className="text-xs">
                    {t.inPinCycle}
                  </Label>
                  <span id={`cycle-hint-${profile.id}`} className="sr-only">
                    {t.inPinCycleHint}
                  </span>
                </div>
              ) : null}
              {profile.id !== 'general' ? (
                <Button size="sm" variant="outline" onClick={() => setPicking(profile)}>
                  <Plus data-icon="inline-start" aria-hidden="true" />
                  {t.addApp}
                  <span className="sr-only"> {profile.name}</span>
                </Button>
              ) : null}
              {profile.custom ? (
                <DropdownMenu>
                  <DropdownMenuTrigger
                    render={
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        aria-label={format(t.profileOptions, { profile: profile.name })}
                      />
                    }
                  >
                    <Ellipsis aria-hidden="true" />
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end" className="w-auto min-w-40">
                    <DropdownMenuItem
                      onClick={() => setRenaming({ id: profile.id, value: profile.name })}
                    >
                      {t.rename}
                      <span className="sr-only"> {profile.name}</span>
                    </DropdownMenuItem>
                    <DropdownMenuItem variant="destructive" onClick={() => setDeleting(profile)}>
                      {t.delete}
                      <span className="sr-only"> {profile.name}</span>
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              ) : null}
            </div>
          </div>

          {profile.id === 'general' ? (
            <p className="text-xs text-muted-foreground">{t.generalApps}</p>
          ) : groups.length === 0 ? (
            <p className="text-xs text-muted-foreground">{t.noApps}</p>
          ) : (
            <ul aria-label={t.appsLabel} className="flex flex-wrap gap-2">
              {visible.map((group) => (
                <li
                  key={group.primary}
                  data-testid={`app-${profile.id}-${group.primary}`}
                  className="flex items-center gap-2 rounded-md bg-muted py-1 pr-1 pl-2 text-sm"
                >
                  <AppIcon id={group.primary} />
                  <span>{group.name}</span>
                  <DropdownMenu>
                    <DropdownMenuTrigger
                      render={
                        <Button
                          size="icon-sm"
                          variant="ghost"
                          className="size-6"
                          aria-label={format(t.appOptions, { app: group.name })}
                        />
                      }
                    >
                      <Ellipsis aria-hidden="true" />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" className="max-h-72 w-auto min-w-44">
                      {others
                        .filter((p) => p.id !== 'general')
                        .map((p) => (
                          <DropdownMenuItem key={p.id} onClick={() => void moveGroup(p, group)}>
                            {format(t.moveToItem, { profile: p.name })}
                          </DropdownMenuItem>
                        ))}
                      {others.some((p) => p.id !== 'general') ? <DropdownMenuSeparator /> : null}
                      <DropdownMenuGroup>
                        <DropdownMenuLabel>{t.removeHint}</DropdownMenuLabel>
                        <DropdownMenuItem
                          variant="destructive"
                          onClick={() => {
                            const general = config.profiles.find((p) => p.id === 'general');
                            if (general) void moveGroup(general, group);
                          }}
                        >
                          {format(t.remove, { app: group.name })}
                        </DropdownMenuItem>
                      </DropdownMenuGroup>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </li>
              ))}
              {groups.length > APPS_SHOWN ? (
                <li>
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-expanded={showAll}
                    onClick={() => setExpanded(showAll ? null : profile.id)}
                  >
                    {showAll ? t.fewerApps : format(t.moreApps, { count: hidden })}
                  </Button>
                </li>
              ) : null}
            </ul>
          )}
        </section>
      ) : null}

      {part !== 'new' ? children : null}

      {part !== 'profile' ? (
        <section aria-label={t.newProfileTitle} className="space-y-2">
          <h2 className={part === 'new' ? 'sr-only' : 'text-sm font-medium'}>
            {t.newProfileTitle}
          </h2>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              disabled={!recent || full}
              onClick={() => recent && createFor(recent)}
              aria-describedby="add-front-hint"
            >
              {recent ? format(t.addFrontNamed, { app: recent.name }) : t.addFront}
            </Button>
            <Button variant="outline" disabled={full} onClick={() => setPicking(null)}>
              {t.pickRunning}
            </Button>
          </div>
          <p id="add-front-hint" className="text-xs text-muted-foreground">
            {full
              ? t.limitReached
              : recent
                ? format(t.addFrontHint, { app: recent.name })
                : t.addFrontNone}
          </p>
          <p className="text-xs text-muted-foreground">
            {format(t.limit, { count: customCount, max: PROFILE_LIMITS.customProfiles })}
          </p>
        </section>
      ) : null}

      <AppPicker
        target={picking}
        config={config}
        onPick={pick}
        onClose={() => setPicking(undefined)}
      />

      <AlertDialog open={deleting !== null} onOpenChange={(open) => !open && setDeleting(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {format(t.deleteTitle, { profile: deleting?.name ?? '' })}
            </AlertDialogTitle>
            <AlertDialogDescription>{t.deleteBody}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t.keep}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                const doomed = deleting;
                setDeleting(null);
                if (doomed) deleteProfile(doomed.id).catch(fail);
              }}
            >
              {t.confirmDelete}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
