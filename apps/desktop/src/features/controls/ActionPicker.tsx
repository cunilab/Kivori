import { useRef, useState } from 'react';
import type { FormEvent, KeyboardEvent, ReactElement } from 'react';
import { Button } from '@kivori/ui/components/button';
import { Input } from '@kivori/ui/components/input';
import { Label } from '@kivori/ui/components/label';
import { RadioGroup, RadioGroupItem } from '@kivori/ui/components/radio-group';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@kivori/ui/components/sheet';
import { CATALOG_SCOPES, MACRO_LIMITS } from '@/lib/ipc/types';
import type {
  ActionCatalogEntryDto,
  ActionSpec,
  ControlRef,
  MacroSpec,
  ProfileConfigDto,
  RotateSpec,
  SlotSpec,
  StepSpec,
} from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { cn } from '@kivori/ui/lib/utils';
import { errorText } from '@/lib/utils';

const t = strings.controls;
const p = t.picker;

/** The device label limit (`MediaText::CAPACITY`) and the width that reads without fading. */
export const LABEL_MAX = 32;
export const LABEL_COMFORTABLE = 10;
const NOT_SET = 'none';
const DELAY = 'delay';

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
  /** `null` when picking a macro step (no profile involved). */
  profile: ProfileConfigDto | null;
  control: ControlRef | 'rotate' | 'heldRotate' | 'step';
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
  /** The user's macros, for the Macros group's picker. */
  macros?: MacroSpec[] | undefined;
  onSaveSlot: (target: PickerTarget, slot: SlotSpec) => Promise<void>;
  onSaveRotate: (target: PickerTarget, rotate: RotateSpec | null) => Promise<void>;
  /** Called with the chosen step when the target's control is `'step'`. */
  onPickStep?: ((step: StepSpec) => void) | undefined;
}

/** Edit one control: pick an action, fill in what it needs, optionally name it for the device. */
export function ActionPicker({
  target,
  catalog,
  macros,
  onClose,
  onSaveSlot,
  onSaveRotate,
  onPickStep,
}: ActionPickerProps): ReactElement {
  return (
    <Sheet open={target !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full overflow-y-auto sm:max-w-md">
        {target ? (
          <PickerForm
            key={`${target.profile?.id ?? 'step'}:${target.control}`}
            target={target}
            catalog={catalog}
            macros={macros}
            onClose={onClose}
            onSaveSlot={onSaveSlot}
            onSaveRotate={onSaveRotate}
            onPickStep={onPickStep}
          />
        ) : null}
      </SheetContent>
    </Sheet>
  );
}

function PickerForm({
  target,
  catalog,
  macros = [],
  onClose,
  onSaveSlot,
  onSaveRotate,
  onPickStep,
}: Omit<ActionPickerProps, 'target'> & { target: PickerTarget }): ReactElement {
  const heldRotate = target.control === 'heldRotate';
  const rotate = target.control === 'rotate' || heldRotate;
  const stepMode = target.control === 'step';
  const spec = target.spec;
  const [id, setId] = useState(spec ? catalogId(spec) : heldRotate ? 'knobShortcuts' : NOT_SET);
  const [app, setApp] = useState(
    spec && 'app' in spec ? spec.app : (target.profile?.apps[0] ?? ''),
  );
  const [keys, setKeys] = useState(spec?.kind === 'shortcut' ? spec.keys : '');
  const [cw, setCw] = useState(spec?.kind === 'shortcuts' ? spec.cw : '');
  const [ccw, setCcw] = useState(spec?.kind === 'shortcuts' ? spec.ccw : '');
  const [launch, setLaunch] = useState(spec?.kind === 'launch' ? spec.target : '');
  const [label, setLabel] = useState(
    spec?.kind === 'shortcuts'
      ? spec.label
      : (target.label ??
          (spec?.kind === 'appVolume'
            ? (spec.label ?? '')
            : heldRotate
              ? t.gestures.heldRotate.name
              : '')),
  );
  const [macroId, setMacroId] = useState(spec?.kind === 'macro' ? spec.id : '');
  const [delayMs, setDelayMs] = useState('500');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // A step is a discrete action that is not a macro (no nesting), or a wait.
  const entries = (catalog ?? []).filter(
    (e) =>
      e.slot === (rotate ? 'rotate' : 'discrete') &&
      !(stepMode && e.id === 'macro') &&
      (!heldRotate || e.id === 'knobShortcuts'),
  );
  const entry = entries.find((e) => e.id === id) ?? null;
  const isDelay = stepMode && id === DELAY;
  const params = entry?.params ?? 'none';
  const delayValue = Number(delayMs);
  const delayBad =
    isDelay &&
    !(
      Number.isInteger(delayValue) &&
      delayValue >= MACRO_LIMITS.delayMin &&
      delayValue <= MACRO_LIMITS.delayMax
    );
  const showLabel =
    !stepMode &&
    !heldRotate &&
    id !== NOT_SET &&
    id !== 'systemVolume' &&
    (!rotate || params === 'app' || params === 'shortcutPair');
  const labelRequired = params === 'shortcutPair';
  const problem = showLabel ? labelProblem(label) : null;
  const missing =
    (params === 'app' && !app.trim()) ||
    (params === 'shortcut' && !keys.trim()) ||
    (params === 'shortcutPair' && (!cw.trim() || !ccw.trim() || !label.trim())) ||
    (params === 'target' && !launch.trim()) ||
    (params === 'macro' && !macros.some((m) => m.id === macroId)) ||
    delayBad ||
    (stepMode && id === NOT_SET);
  const unavailable =
    id !== NOT_SET && !isDelay && (entry === null || entry.availability !== 'available');
  const canSave = !saving && !problem && !missing && !unavailable;

  const discrete = (): ActionSpec | null => {
    switch (id) {
      case 'appMute':
        return { kind: 'appMute', app };
      case 'shortcut':
        return { kind: 'shortcut', keys };
      case 'launch':
        return { kind: 'launch', target: launch };
      case 'macro':
        return { kind: 'macro', id: macroId };
      case 'playPause':
      case 'previousTrack':
      case 'nextTrack':
      case 'systemMute':
      case 'micMute':
        return { kind: id };
      default:
        return null;
    }
  };
  const rotation = (): RotateSpec | null => {
    if (heldRotate && id === NOT_SET) return null;
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
    if (stepMode) {
      const action = discrete();
      if (isDelay) onPickStep?.({ kind: 'delay', ms: delayValue });
      else if (action && action.kind !== 'macro') onPickStep?.({ kind: 'action', action });
      onClose();
      return;
    }
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
        <SheetTitle>
          {stepMode ? p.stepTitle : format(p.title, { control: target.name })}
        </SheetTitle>
        <SheetDescription>
          {stepMode
            ? p.stepDescription
            : format(p.description, { profile: target.profile?.name ?? '' })}
        </SheetDescription>
      </SheetHeader>

      <div className="flex-1 space-y-5 overflow-y-auto px-4">
        <RadioGroup
          aria-label={p.actionGroup}
          value={id}
          onValueChange={(value) => setId(value as string)}
          className="gap-4"
        >
          {(!rotate || heldRotate) && !stepMode && (
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
          {stepMode && (
            <div role="group" aria-label={p.groups.delay} className="space-y-1.5">
              <p className="text-xs font-medium text-muted-foreground">{p.groups.delay}</p>
              <Option
                value={DELAY}
                name={p.delay}
                detail={p.delayHint}
                disabled={false}
                checked={id === DELAY}
              />
            </div>
          )}
        </RadioGroup>

        {id === 'micMute' && (
          <p className="text-sm text-muted-foreground" data-testid="mic-note">
            {p.micNote}
          </p>
        )}

        {params === 'macro' && (
          <Field
            id="picker-macro"
            label={p.macro}
            hint={macros.length === 0 ? p.noMacros : undefined}
          >
            <select
              id="picker-macro"
              value={macroId}
              onChange={(e) => setMacroId(e.target.value)}
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="">—</option>
              {macros.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </select>
          </Field>
        )}
        {isDelay && (
          <Field
            id="picker-delay"
            label={p.delayMs}
            hint={delayBad ? p.delayRange : undefined}
            invalid={delayBad}
          >
            <Input
              id="picker-delay"
              type="number"
              inputMode="numeric"
              value={delayMs}
              aria-invalid={delayBad}
              onChange={(e) => setDelayMs(e.target.value)}
            />
          </Field>
        )}

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
            <ShortcutInput
              id="picker-shortcut"
              value={keys}
              placeholder={p.shortcutPlaceholder}
              onChange={setKeys}
            />
          </Field>
        )}
        {params === 'shortcutPair' && (
          <>
            <Field id="picker-cw" label={p.clockwise}>
              <ShortcutInput id="picker-cw" value={cw} placeholder="Ctrl+Tab" onChange={setCw} />
            </Field>
            <Field id="picker-ccw" label={p.counterClockwise}>
              <ShortcutInput
                id="picker-ccw"
                value={ccw}
                placeholder="Ctrl+Shift+Tab"
                onChange={setCcw}
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
          {stepMode ? p.add : saving ? p.saving : p.save}
        </Button>
      </SheetFooter>
    </form>
  );
}

const namedKeys: Record<string, string> = {
  ' ': 'Space',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Enter: 'Enter',
  Tab: 'Tab',
  Escape: 'Escape',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
};
const shiftedKeys: Record<string, [string, string]> = {
  '!': ['Digit1', '1'],
  '@': ['Digit2', '2'],
  '#': ['Digit3', '3'],
  $: ['Digit4', '4'],
  '%': ['Digit5', '5'],
  '^': ['Digit6', '6'],
  '&': ['Digit7', '7'],
  '*': ['Digit8', '8'],
  '(': ['Digit9', '9'],
  ')': ['Digit0', '0'],
  '{': ['BracketLeft', '['],
  '}': ['BracketRight', ']'],
  '~': ['Backquote', '`'],
};

function ShortcutInput({
  id,
  value,
  placeholder,
  onChange,
}: {
  id: string;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
}): ReactElement {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const button = useRef<HTMLButtonElement>(null);

  function capture(event: KeyboardEvent<HTMLInputElement>): void {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(event.key)) return;
    if (
      event.key === 'Escape' &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.shiftKey &&
      !event.metaKey
    ) {
      setRecording(false);
      setError(false);
      button.current?.focus();
      return;
    }
    const functionKey = /^F(\d{1,2})$/.exec(event.key);
    const maxFunction = /Windows/i.test(navigator.userAgent) ? 24 : 20;
    const shifted = shiftedKeys[event.key];
    const key =
      namedKeys[event.key] ??
      (/^[a-z0-9]$/i.test(event.key) || ['`', '[', ']'].includes(event.key)
        ? event.key.toUpperCase()
        : null) ??
      (event.shiftKey && shifted?.[0] === event.code ? shifted[1] : null) ??
      (functionKey && Number(functionKey[1]) >= 1 && Number(functionKey[1]) <= maxFunction
        ? event.key
        : null);
    if (!key || event.getModifierState('AltGraph')) {
      setError(true);
      return;
    }
    onChange(
      [
        event.ctrlKey && 'Ctrl',
        event.altKey && 'Alt',
        event.shiftKey && 'Shift',
        event.metaKey && 'Meta',
        key,
      ]
        .filter(Boolean)
        .join('+'),
    );
    setError(false);
    setRecording(false);
    button.current?.focus();
  }

  return (
    <div className="space-y-1.5">
      <div className="flex gap-2">
        <Input
          ref={input}
          id={id}
          value={value}
          placeholder={placeholder}
          readOnly={recording}
          aria-describedby={recording || error ? `${id}-recording` : undefined}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={capture}
          onBlur={(event) => {
            if (event.relatedTarget !== button.current) setRecording(false);
          }}
        />
        <Button
          ref={button}
          type="button"
          variant="outline"
          aria-label={`${recording ? p.cancelRecording : p.recordKeys} for ${id === 'picker-cw' ? p.clockwise : id === 'picker-ccw' ? p.counterClockwise : p.shortcut}`}
          onClick={() => {
            setError(false);
            if (recording) setRecording(false);
            else {
              setRecording(true);
              input.current?.focus();
            }
          }}
        >
          {recording ? p.cancel : p.record}
        </Button>
      </div>
      {recording || error ? (
        <p id={`${id}-recording`} role="status" className="text-xs text-muted-foreground">
          {error ? p.recordUnsupported : p.recordHint}
        </p>
      ) : null}
    </div>
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
  hint?: string | undefined;
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
