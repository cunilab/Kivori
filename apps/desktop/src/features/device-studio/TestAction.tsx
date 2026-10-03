import { useState } from 'react';
import type { FormEvent, ReactElement } from 'react';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { runTestAction } from '../../lib/ipc';
import type { TestActionRequest } from '../../lib/ipc/types';
import { strings } from '../../lib/i18n/strings';
import { errorText } from '../../lib/utils';

/** Dev-only Device Studio panel: fires one desk action; the outcome shows in Overview and the Log. */
export function TestAction(): ReactElement {
  const [shortcut, setShortcut] = useState('');
  const [target, setTarget] = useState('');
  const [error, setError] = useState<string | null>(null);
  const t = strings.testAction;

  const run = (request: TestActionRequest): void => {
    setError(null);
    void runTestAction(request).catch((e: unknown) => setError(errorText(e)));
  };
  const submit =
    (request: () => TestActionRequest) =>
    (event: FormEvent): void => {
      event.preventDefault();
      run(request());
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
        <Button type="button" variant="outline" onClick={() => run({ action: 'playPause' })}>
          {t.playPause}
        </Button>
        <Button type="button" variant="outline" onClick={() => run({ action: 'mute' })}>
          {t.mute}
        </Button>
      </div>
      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={submit(() => ({ action: 'shortcut', shortcut }))}
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
        onSubmit={submit(() => ({ action: 'launch', target }))}
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
