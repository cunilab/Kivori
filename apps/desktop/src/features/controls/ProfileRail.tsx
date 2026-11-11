import type { ReactElement } from 'react';
import { AppWindow, Globe, Lock, Plus, Smartphone, Terminal } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { DeskStatusDto, ProfileConfigDto, ProfileId } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { AppIcon } from './ProfilesCard';

const t = strings.controls;

function fallbackIcon(id: ProfileId): LucideIcon {
  if (id === 'general') return Globe;
  if (id === 'claudecode' || id === 'codex' || id === 'opencode') return Terminal;
  return AppWindow;
}

/** The profiles to edit, as chips, plus the way to add one. */
export function ProfileRail({
  profiles,
  desk,
  selected,
  newOpen,
  onSelect,
  onNew,
}: {
  profiles: ProfileConfigDto[];
  desk: DeskStatusDto | null;
  selected: ProfileId;
  newOpen: boolean;
  onSelect: (id: ProfileId) => void;
  onNew: () => void;
}): ReactElement {
  return (
    <nav aria-label={t.profilesLabel} className="flex flex-wrap gap-2">
      {profiles.map((profile) => {
        const active = desk?.profileId === profile.id;
        const locked = !!desk?.pinned && active;
        const appId = profile.apps[0];
        const Fallback = fallbackIcon(profile.id);
        const marks = [
          active ? t.rail.active : '',
          locked ? t.rail.locked : '',
          profile.inPinCycle ? t.rail.onDevice : '',
        ].filter((mark) => mark !== '');
        // The name is set explicitly so status text stays separated from the profile name.
        const label = marks.length > 0 ? `${profile.name}, ${marks.join(', ')}` : profile.name;
        return (
          <button
            key={profile.id}
            type="button"
            data-testid={`profile-chip-${profile.id}`}
            aria-label={label}
            aria-pressed={selected === profile.id}
            onClick={() => onSelect(profile.id)}
            className={
              selected === profile.id
                ? 'inline-flex items-center gap-1.5 rounded-lg border border-primary bg-primary/10 px-2.5 py-1.5 text-sm transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring'
                : 'inline-flex items-center gap-1.5 rounded-lg border border-border bg-card px-2.5 py-1.5 text-sm transition-colors outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring'
            }
          >
            {appId ? (
              <AppIcon id={appId} />
            ) : (
              <Fallback className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            )}
            <span>{profile.name}</span>
            {active ? (
              <>
                <span
                  aria-hidden="true"
                  className="size-1.5 shrink-0 rounded-full bg-primary motion-safe:animate-pulse"
                />
                <span className="sr-only"> {t.rail.active}</span>
              </>
            ) : null}
            {locked ? (
              <>
                <Lock className="size-3 shrink-0" aria-hidden="true" />
                <span className="sr-only"> {t.rail.locked}</span>
              </>
            ) : null}
            {profile.inPinCycle ? (
              <>
                <Smartphone className="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
                <span className="sr-only"> {t.rail.onDevice}</span>
              </>
            ) : null}
          </button>
        );
      })}
      <button
        type="button"
        aria-expanded={newOpen}
        onClick={onNew}
        className="inline-flex items-center gap-1.5 rounded-lg border border-dashed border-border bg-card px-2.5 py-1.5 text-sm transition-colors outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
      >
        <Plus className="size-3.5" aria-hidden="true" />
        {t.newProfileToggle}
      </button>
    </nav>
  );
}
