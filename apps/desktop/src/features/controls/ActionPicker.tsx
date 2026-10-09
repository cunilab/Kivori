import { useState } from 'react';
import type { FormEvent, ReactElement } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { CATALOG_SCOPES } from '@/lib/ipc/types';
import type {
  ActionCatalogEntryDto,
  ActionSpec,
  ControlRef,
  ProfileConfigDto,
  RotateSpec,
  SlotSpec,
} from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { cn, errorText } from '@/lib/utils';

const t = strings.controls;
const p = t.picker;

/** The device label limit (`MediaText::CAPACITY`) and the width that reads without fading. */
export const LABEL_MAX = 32;
export const LABEL_COMFORTABLE = 10;
const NOT_SET = 'none';

/** The catalog id of a saved binding (a knob shortcut pair is `knobShortcuts` in the catalog). */
export function catalogId(spec: ActionSpec | RotateSpec): string {
  return spec.kind === 'shortcuts' ? 'knobShortcuts' : spec.kind;
}

/** `null` when the label is acceptable, else the reason. Empty is fine (the device picks one). */
export function labelProblem(label: string): string | null {
  if ([...label].length > LABEL_MAX) return p.labelTooLong;
  const latin1 = [...label].every((c) => {
    const code = c.codePointAt(0) ?? 0;
    return (code >= 0x20 && code <= 0x7e) || (code >= 0xa0 && code <= 0xff);
  });
  return latin1 ? null : p.labelCharset;
}

export interface PickerTarget {
  profile: ProfileConfigDto;
  control: ControlRef | 'rotate';
  /** The control's name, for the sheet title. */
  name: string;
  /** What is bound now (`null` = unbound). */
  spec: ActionSpec | RotateSpec | null;
  label: string | null;
}

export interface ActionPickerProps {
  target: PickerTarget | null;
  catalog: ActionCatalogEntryDto[] | null;
  onClose: () => void;
  onSaveSlot: (target: PickerTarget, slot: SlotSpec) => Promise<void>;
  onSaveRotate: (target: PickerTarget, rotate: RotateSpec) => Promise<void>;
}

/** Edit one control: pick an action, fill in what it needs, optionally name it for the device. */
export function ActionPicker({
  target,
  catalog,
  onClose,
  onSaveSlot,
  onSaveRotate,
}: ActionPickerProps): ReactElement {
  return (
    <Sheet open={target !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full overflow-y-auto sm:max-w-md">
        {target ? (
          <PickerForm
            key={`${target.profile.id}:${target.control}`}
            target={target}
            catalog={catalog}
            onClose={onClose}
            onSaveSlot={onSaveSlot}
            onSaveRotate={onSaveRotate}
          />
        ) : null}
      </SheetContent>
    </Sheet>
  );
}

function PickerForm({
  target,
  catalog,
  onClose,
  onSaveSlot,
  onSaveRotate,
}: Omit<ActionPickerProps, 'target'> & { target: PickerTarget }): ReactElement {
  const rotate = target.control === 'rotate';
  const spec = target.spec;
  const [id, setId] = useState(spec ? catalogId(spec) : NOT_SET);
  const [app, setApp] = useState(spec && 'app' in spec ? spec.app : (target.profile.apps[0] ?? ''));
  const [keys, setKeys] = useState(spec?.kind === 'shortcut' ? spec.keys : '');
  const [cw, setCw] = useState(spec?.kind === 'shortcuts' ? spec.cw : '');
  const [ccw, setCcw] = useState(spec?.kind === 'shortcuts' ? spec.ccw : '');
  const [launch, setLaunch] = useState(spec?.kind === 'launch' ? spec.target : '');
  const [label, setLabel] = useState(
    spec?.kind === 'shortcuts'
      ? spec.label
      : (target.label ?? (spec?.kind === 'appVolume' ? (spec.label ?? '') : '')),
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const entries = (catalog ?? []).filter((e) => e.slot === (rotate ? 'rotate' : 'discrete'));
  const entry = entries.find((e) => e.id === id) ?? null;
  const params = entry?.params ?? 'none';
  const showLabel =
    id !== NOT_SET &&
    id !== 'systemVolume' &&
    (!rotate || params === 'app' || params === 'shortcutPair');
  const labelRequired = params === 'shortcutPair';
  const problem = showLabel ? labelProblem(label) : null;
  const missing =
    (params === 'app' && !app.trim()) ||
    (params === 'shortcut' && !keys.trim()) ||
    (params === 'shortcutPair' && (!cw.trim() || !ccw.trim() || !label.trim())) ||
    (params === 'target' && !launch.trim());
  const unavailable = id !== NOT_SET && (entry === null || entry.availability !== 'available');
  const canSave = !saving && !problem && !missing && !unavailable;

  const discrete = (): ActionSpec | null => {
    switch (id) {
      case 'appMute':
        return { kind: 'appMute', app };
      case 'shortcut':
        return { kind: 'shortcut', keys };
      case 'launch':
        return { kind: 'launch', target: launch };
      case 'playPause':
      case 'previousTrack':
      case 'nextTrack':
      case 'systemMute':
        return { kind: id };
      default:
        return null;
    }
  };
  const rotation = (): RotateSpec => {
    const text = label.trim();
    if (id === 'appVolume') {
      return text ? { kind: 'appVolume', app, label: text } : { kind: 'appVolume', app };
    }
    if (id === 'knobShortcuts') return { kind: 'shortcuts', cw, ccw, label: text };
    return { kind: 'systemVolume' };
  };

  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (!canSave) return;
    setSaving(true);
    setError(null);
    const saved = rotate
      ? onSaveRotate(target, rotation())
      : onSaveSlot(target, { action: discrete(), label: label.trim() || null });
    saved.then(onClose).catch((e: unknown) => {
      setSaving(false);
      setError(errorText(e));
    });
  };

  const groups = CATALOG_SCOPES.map((scope) => ({
    scope,
    entries: entries.filter((e) => e.scope === scope),
  })).filter((g) => g.entries.length > 0);

  return (
    <form onSubmit={submit} className="flex min-h-0 flex-1 flex-col">
      <SheetHeader>
        <SheetTitle>{format(p.title, { control: target.name })}</SheetTitle>
        <SheetDescription>
          {format(p.description, { profile: target.profile.name })}
        </SheetDescription>
      </SheetHeader>

      <div className="flex-1 space-y-5 overflow-y-auto px-4">
        <RadioGroup
          aria-label={p.actionGroup}
          value={id}
          onValueChange={(value) => setId(value as string)}
          className="gap-4"
        >
          {!rotate && (
            <Option
              value={NOT_SET}
              name={p.notSet}
              detail={p.notSetHint}
              disabled={false}
              checked={id === NOT_SET}
            />
          )}
          {groups.map(({ scope, entries: list }) => (
            <div key={scope} role="group" aria-label={p.groups[scope]} className="space-y-1.5">
              <p className="text-xs font-medium text-muted-foreground">{p.groups[scope]}</p>
              {list.map((e) => {
                const ok = e.availability === 'available';
                return (
                  <Option
                    key={e.id}
                    value={e.id}
                    name={p.entries[e.id as keyof typeof p.entries] ?? e.id}
                    detail={ok ? t.verification[e.verification] : (e.reason ?? '')}
                    disabled={!ok}
                    checked={id === e.id}
                  />
                );
              })}
            </div>
          ))}
        </RadioGroup>

        {params === 'app' && (
          <Field id="picker-app" label={p.app} hint={p.appHint}>
            <Input
              id="picker-app"
              value={app}
              placeholder={p.appPlaceholder}
              onChange={(e) => setApp(e.target.value)}
            />
          </Field>
        )}
        {params === 'shortcut' && (
          <Field id="picker-shortcut" label={p.shortcut}>
            <Input
              id="picker-shortcut"
              value={keys}
              placeholder={p.shortcutPlaceholder}
              onChange={(e) => setKeys(e.target.value)}
            />
          </Field>
        )}
        {params === 'shortcutPair' && (
          <>
            <Field id="picker-cw" label={p.clockwise}>
              <Input
                id="picker-cw"
                value={cw}
                placeholder="Ctrl+Tab"
                onChange={(e) => setCw(e.target.value)}
              />
            </Field>
            <Field id="picker-ccw" label={p.counterClockwise}>
              <Input
                id="picker-ccw"
                value={ccw}
                placeholder="Ctrl+Shift+Tab"
                onChange={(e) => setCcw(e.target.value)}
              />
            </Field>
          </>
        )}
        {params === 'target' && (
          <Field id="picker-target" label={p.target}>
            <Input
              id="picker-target"
              value={launch}
              placeholder={p.targetPlaceholder}
              onChange={(e) => setLaunch(e.target.value)}
            />
          </Field>
        )}
        {showLabel && (
          <Field
            id="picker-label"
            label={labelRequired ? p.labelRequired : p.label}
            hint={problem ?? (label.length > LABEL_COMFORTABLE ? p.labelFade : p.labelOptional)}
            invalid={problem !== null}
          >
            <Input
              id="picker-label"
              value={label}
              aria-invalid={problem !== null}
              onChange={(e) => setLabel(e.target.value)}
            />
          </Field>
        )}
        {error ? (
          <p role="alert" className="text-sm text-destructive">
            {p.saveFailed}: {error}
          </p>
        ) : null}
      </div>

      <SheetFooter className="flex-row justify-end">
        <Button type="button" variant="outline" onClick={onClose}>
          {p.cancel}
        </Button>
        <Button type="submit" disabled={!canSave}>
          {saving ? p.saving : p.save}
        </Button>
      </SheetFooter>
    </form>
  );
}

function Option({
  value,
  name,
  detail,
  disabled,
  checked,
}: {
  value: string;
  name: string;
  detail: string;
  disabled: boolean;
  checked: boolean;
}): ReactElement {
  return (
    <label
      className={cn(
        'flex items-start gap-3 rounded-lg px-3 py-2 ring-1 ring-foreground/10',
        disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer hover:bg-muted/50',
        checked && 'ring-2 ring-primary',
      )}
    >
      <RadioGroupItem value={value} disabled={disabled} className="mt-1" />
      <span className="min-w-0 space-y-0.5">
        <span className="block text-sm font-medium">{name}</span>
        {detail ? <span className="block text-xs text-muted-foreground">{detail}</span> : null}
      </span>
    </label>
  );
}

function Field({
  id,
  label,
  hint,
  invalid,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  invalid?: boolean;
  children: ReactElement;
}): ReactElement {
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
      {hint ? (
        <p className={cn('text-xs', invalid ? 'text-destructive' : 'text-muted-foreground')}>
          {hint}
        </p>
      ) : null}
    </div>
  );
}
