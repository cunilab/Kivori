import { useState } from 'react';
import type { FormEvent, ReactElement } from 'react';
import { ArrowDown, ArrowUp, Plus, X } from 'lucide-react';
import { Badge } from '@kivori/ui/components/badge';
import { Button } from '@kivori/ui/components/button';
import { Input } from '@kivori/ui/components/input';
import { Label } from '@kivori/ui/components/label';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@kivori/ui/components/sheet';
import { saveMacro } from '@/lib/ipc';
import { MACRO_LIMITS } from '@/lib/ipc/types';
import type { ActionCatalogEntryDto, MacroSpec, StepSpec } from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { errorText } from '@/lib/utils';
import { ActionPicker, catalogId, labelProblem } from './ActionPicker';
import type { PickerTarget } from './ActionPicker';

const t = strings.controls;
const m = t.macros;

type Certainty = 'confirmed' | 'started' | 'unverified';
/** How well an outcome is known; a macro is as known as its least-known step. */
const RANK: Record<Certainty, number> = { unverified: 0, started: 1, confirmed: 2 };

/** The least certain verification among a macro's action steps; `null` with no action step. */
export function leastCertain(
  steps: StepSpec[],
  catalog: ActionCatalogEntryDto[] | null,
): Certainty | null {
  let least: Certainty | null = null;
  for (const step of steps) {
    if (step.kind !== 'action') continue;
    const found = catalog?.find((e) => e.id === catalogId(step.action))?.verification;
    // An unknown entry (catalog not loaded) is treated as the least certain.
    const kind: Certainty = found && found !== 'leastOfSteps' ? found : 'unverified';
    if (least === null || RANK[kind] < RANK[least]) least = kind;
  }
  return least;
}

/** A short plain-language line for one step. */
export function describeStep(step: StepSpec): string {
  if (step.kind === 'delay') return format(m.wait, { ms: step.ms });
  const { action } = step;
  const name = t.picker.entries[catalogId(action) as keyof typeof t.picker.entries];
  switch (action.kind) {
    case 'shortcut':
      return `${name}: ${action.keys}`;
    case 'launch':
      return `${name}: ${action.target}`;
    case 'appMute':
      return `${name}: ${action.app}`;
    default:
      return name;
  }
}

/** A readable, unique id for a new macro (`standup`, `standup-2`). Never changes once saved. */
function newId(name: string, taken: MacroSpec[]): string {
  const base =
    name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 40) || 'macro';
  const used = new Set(taken.map((existing) => existing.id));
  let id = base;
  for (let n = 2; used.has(id); n += 1) id = `${base}-${n}`;
  return id;
}

export interface MacroEditorProps {
  /** `undefined` = closed, `null` = a new macro, else the macro being edited. */
  macro: MacroSpec | null | undefined;
  macros: MacroSpec[];
  catalog: ActionCatalogEntryDto[] | null;
  onClose: () => void;
}

/** Create or edit a macro: a name and an ordered list of steps. Saving goes to the native core. */
export function MacroEditor({ macro, macros, catalog, onClose }: MacroEditorProps): ReactElement {
  return (
    <Sheet open={macro !== undefined} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full overflow-y-auto sm:max-w-md">
        {macro !== undefined ? (
          <EditorForm
            key={macro?.id ?? 'new'}
            macro={macro}
            macros={macros}
            catalog={catalog}
            onClose={onClose}
          />
        ) : null}
      </SheetContent>
    </Sheet>
  );
}

function EditorForm({
  macro,
  macros,
  catalog,
  onClose,
}: Omit<MacroEditorProps, 'macro'> & { macro: MacroSpec | null }): ReactElement {
  const [name, setName] = useState(macro?.name ?? '');
  const [steps, setSteps] = useState<StepSpec[]>(macro?.steps ?? []);
  const [adding, setAdding] = useState<PickerTarget | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const least = leastCertain(steps, catalog);
  const problem = name.trim() ? labelProblem(name.trim()) : null;
  const full = steps.length >= MACRO_LIMITS.steps;
  const hasAction = steps.some((step) => step.kind === 'action');
  const canSave = !saving && name.trim() !== '' && !problem && hasAction;

  const move = (from: number, to: number): void => {
    setSteps((current) => {
      const next = [...current];
      const [moved] = next.splice(from, 1);
      if (moved) next.splice(to, 0, moved);
      return next;
    });
  };

  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (!canSave) return;
    setSaving(true);
    setError(null);
    const spec: MacroSpec = {
      id: macro?.id ?? newId(name.trim(), macros),
      name: name.trim(),
      steps,
    };
    saveMacro(spec)
      .then(onClose)
      .catch((e: unknown) => {
        setSaving(false);
        setError(errorText(e));
      });
  };

  // The step picker is a sibling of the form: a portal's submit would bubble into this form.
  return (
    <>
      <form onSubmit={submit} className="flex min-h-0 flex-1 flex-col">
        <SheetHeader>
          <SheetTitle>{macro ? m.editorTitle : m.newTitle}</SheetTitle>
          <SheetDescription>{m.editorDescription}</SheetDescription>
        </SheetHeader>

        <div className="flex-1 space-y-5 overflow-y-auto px-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="macro-name">{m.name}</Label>
            <Input
              id="macro-name"
              value={name}
              aria-invalid={problem !== null}
              onChange={(e) => setName(e.target.value)}
            />
            <p className={problem ? 'text-xs text-destructive' : 'text-xs text-muted-foreground'}>
              {problem ?? m.nameHint}
            </p>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-2">
              <p className="text-sm font-medium">{m.stepsLabel}</p>
              <Badge variant="outline" data-testid="macro-least">
                {m.least}: {least ? t.verification[least] : m.leastNone}
              </Badge>
            </div>
            {steps.length === 0 ? (
              <p className="text-sm text-muted-foreground">{m.noSteps}</p>
            ) : (
              <ol className="space-y-1.5" aria-label={m.stepsLabel}>
                {steps.map((step, i) => (
                  <li
                    key={`${i}:${describeStep(step)}`}
                    className="flex items-center gap-1 rounded-lg px-3 py-1.5 ring-1 ring-foreground/10"
                  >
                    <span className="w-5 text-xs text-muted-foreground">{i + 1}</span>
                    <span className="min-w-0 flex-1 truncate text-sm">{describeStep(step)}</span>
                    <Button
                      type="button"
                      size="icon-sm"
                      variant="ghost"
                      aria-label={format(m.up, { n: i + 1 })}
                      disabled={i === 0}
                      onClick={() => move(i, i - 1)}
                    >
                      <ArrowUp aria-hidden="true" />
                    </Button>
                    <Button
                      type="button"
                      size="icon-sm"
                      variant="ghost"
                      aria-label={format(m.down, { n: i + 1 })}
                      disabled={i === steps.length - 1}
                      onClick={() => move(i, i + 1)}
                    >
                      <ArrowDown aria-hidden="true" />
                    </Button>
                    <Button
                      type="button"
                      size="icon-sm"
                      variant="ghost"
                      aria-label={format(m.remove, { n: i + 1 })}
                      onClick={() => setSteps((current) => current.filter((_, j) => j !== i))}
                    >
                      <X aria-hidden="true" />
                    </Button>
                  </li>
                ))}
              </ol>
            )}
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={full}
              onClick={() =>
                setAdding({
                  profile: null,
                  control: 'step',
                  name: m.addStep,
                  spec: null,
                  label: null,
                })
              }
            >
              <Plus data-icon="inline-start" aria-hidden="true" />
              {m.addStep}
            </Button>
            <p className="text-xs text-muted-foreground">{full ? m.tooMany : m.limits}</p>
            <p className="text-xs text-muted-foreground">{m.stopsHint}</p>
          </div>

          {error ? (
            <p role="alert" className="text-sm text-destructive">
              {m.saveFailed}: {error}
            </p>
          ) : null}
        </div>

        <SheetFooter className="flex-row justify-end">
          <Button type="button" variant="outline" onClick={onClose}>
            {m.cancel}
          </Button>
          <Button type="submit" disabled={!canSave}>
            {saving ? m.saving : m.save}
          </Button>
        </SheetFooter>
      </form>
      <ActionPicker
        target={adding}
        catalog={catalog}
        onClose={() => setAdding(null)}
        onSaveSlot={() => Promise.resolve()}
        onSaveRotate={() => Promise.resolve()}
        onPickStep={(step) => setSteps((current) => [...current, step])}
      />
    </>
  );
}
