import { useEffect, useState } from 'react';
import type { FormEvent, ReactElement } from 'react';
import { toast } from 'sonner';
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
} from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';

const t = strings.controls.profileList;

/** What an app id reads as: `idea64.exe` -> `idea64`; a bundle id stays whole. */
export function appLabel(id: string): string {
  return id.replace(/\.exe$/, '');
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
 * Profiles and the apps they match: add the app in front of you (or a running one) as a new
 * profile, move apps between profiles, rename and delete your own, and choose which join the
 * middle button's pin cycle. Every change saves through the native core; the list re-renders
 * from the config it returns.
 */
export function ProfilesCard({
  config,
  desk,
  onShow,
}: {
  config: ConfigDto;
  desk: DeskStatusDto | null;
  onShow: (id: ProfileId) => void;
}): ReactElement {
  const [picking, setPicking] = useState<ProfileConfigDto | null | undefined>(undefined);
  const [renaming, setRenaming] = useState<{ id: ProfileId; value: string } | null>(null);
  const [deleting, setDeleting] = useState<ProfileConfigDto | null>(null);
  const customCount = config.profiles.filter((p) => p.custom).length;
  const full = customCount >= PROFILE_LIMITS.customProfiles;
  const recent = desk?.recentApp ?? null;

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

  return (
    <Card data-testid="profile-list">
      <CardHeader>
        <CardTitle>{t.title}</CardTitle>
        <CardDescription>{t.description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
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
        </div>

        <ul aria-label={t.listLabel} className="space-y-3">
          {config.profiles.map((profile) => (
            <li
              key={profile.id}
              data-testid={`profile-${profile.id}`}
              className="space-y-2 rounded-lg bg-muted px-3 py-2.5"
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
                    <Button
                      size="sm"
                      type="button"
                      variant="ghost"
                      onClick={() => setRenaming(null)}
                    >
                      {t.renameCancel}
                    </Button>
                    <p role="status" className="basis-full text-xs text-muted-foreground">
                      {nameProblem(renaming.value) ?? t.nameHint}
                    </p>
                  </form>
                ) : (
                  <span className="font-medium">{profile.name}</span>
                )}
                <Badge variant="outline">{profile.custom ? t.yourProfile : t.builtIn}</Badge>
                <div className="ml-auto flex flex-wrap items-center gap-2">
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
                  <Button size="sm" variant="outline" onClick={() => onShow(profile.id)}>
                    {t.showControls}
                    <span className="sr-only"> {profile.name}</span>
                  </Button>
                  {profile.id !== 'general' ? (
                    <Button size="sm" variant="outline" onClick={() => setPicking(profile)}>
                      {t.addApp}
                      <span className="sr-only"> {profile.name}</span>
                    </Button>
                  ) : null}
                  {profile.custom ? (
                    <>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setRenaming({ id: profile.id, value: profile.name })}
                      >
                        {t.rename}
                        <span className="sr-only"> {profile.name}</span>
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => setDeleting(profile)}>
                        {t.delete}
                        <span className="sr-only"> {profile.name}</span>
                      </Button>
                    </>
                  ) : null}
                </div>
              </div>
              {profile.id === 'general' ? (
                <p className="text-xs text-muted-foreground">{t.generalApps}</p>
              ) : profile.apps.length === 0 ? (
                <p className="text-xs text-muted-foreground">{t.noApps}</p>
              ) : (
                <ul className="flex flex-wrap gap-2">
                  {profile.apps.map((id) => (
                    <li
                      key={id}
                      data-testid={`app-${profile.id}-${id}`}
                      className="flex items-center gap-2 rounded-md bg-background px-2 py-1 text-sm"
                    >
                      <AppIcon id={id} />
                      <span>{appLabel(id)}</span>
                      <select
                        aria-label={format(t.moveTo, { app: appLabel(id) })}
                        className="h-7 rounded border bg-background text-xs"
                        value=""
                        onChange={(e) => {
                          const target = config.profiles.find((p) => p.id === e.target.value);
                          if (target) moveApp(target, { id, name: '' });
                        }}
                      >
                        <option value="">{t.moveToPlaceholder}</option>
                        {config.profiles
                          .filter((p) => p.id !== profile.id)
                          .map((p) => (
                            <option key={p.id} value={p.id}>
                              {p.name}
                            </option>
                          ))}
                      </select>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2"
                        title={t.removeHint}
                        aria-label={format(t.remove, { app: appLabel(id) })}
                        onClick={() => {
                          const general = config.profiles.find((p) => p.id === 'general');
                          if (general) moveApp(general, { id, name: '' });
                        }}
                      >
                        ×
                      </Button>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          ))}
        </ul>
      </CardContent>

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
    </Card>
  );
}
