import { useState } from 'react';
import type { FormEvent, ReactElement } from 'react';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { testAction } from '../../lib/ipc';
import type { ActionSpec } from '../../lib/ipc/types';
import { strings } from '../../lib/i18n/strings';
import { errorText } from '../../lib/utils';

/** Device Studio panel: fires one desk action through the production Test Action; the outcome shows in Overview and the Log. */
export function TestAction(): ReactElement {
  const [shortcut, setShortcut] = useState('');
  const [target, setTarget] = useState('');
  const [error, setError] = useState<string | null>(null);
  const t = strings.testAction;

  const run = (action: ActionSpec): void => {
    setError(null);
    void testAction(action).catch((e: unknown) => setError(errorText(e)));
  };
  const submit =
    (action: () => ActionSpec) =>
    (event: FormEvent): void => {
      event.preventDefault();
      run(action());
    };

  return (
    <section aria-labelledby="test-action-heading" className="space-y-4">
      <div>
        <h2 id="test-action-heading" className="text-base font-medium">
          {t.heading}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">{t.description}</p>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="outline" onClick={() => run({ kind: 'playPause' })}>
          {t.playPause}
        </Button>
        <Button type="button" variant="outline" onClick={() => run({ kind: 'systemMute' })}>
          {t.mute}
        </Button>
      </div>
      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={submit(() => ({ kind: 'shortcut', keys: shortcut }))}
      >
        <div className="flex min-w-48 flex-1 flex-col gap-1.5">
          <Label htmlFor="test-shortcut">{t.shortcut}</Label>
          <Input
            id="test-shortcut"
            value={shortcut}
            placeholder={t.shortcutPlaceholder}
            onChange={(e) => setShortcut(e.target.value)}
          />
        </div>
        <Button type="submit" variant="outline">
          {t.runShortcut}
        </Button>
      </form>
      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={submit(() => ({ kind: 'launch', target }))}
      >
        <div className="flex min-w-48 flex-1 flex-col gap-1.5">
          <Label htmlFor="test-launch">{t.launch}</Label>
          <Input
            id="test-launch"
            value={target}
            placeholder={t.launchPlaceholder}
            onChange={(e) => setTarget(e.target.value)}
          />
        </div>
        <Button type="submit" variant="outline">
          {t.runLaunch}
        </Button>
      </form>
      {error ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
    </section>
  );
}
