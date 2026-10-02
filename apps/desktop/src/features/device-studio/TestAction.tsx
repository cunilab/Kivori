import { useState } from 'react';
import type { FormEvent, ReactElement } from 'react';
import { Button } from '../../components/ui/button';
import { runTestAction } from '../../lib/ipc';
import type { TestActionRequest } from '../../lib/ipc/types';
import { strings } from '../../lib/i18n/strings';
import { errorText } from '../desk/DeskPanel';

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
        <h2 id="test-action-heading" className="font-medium">
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
        <label className="flex flex-col gap-1 text-sm">
          {t.shortcut}
          <input
            className="rounded-md border px-2 py-1"
            value={shortcut}
            placeholder={t.shortcutPlaceholder}
            onChange={(e) => setShortcut(e.target.value)}
          />
        </label>
        <Button type="submit" variant="outline">
          {t.runShortcut}
        </Button>
      </form>
      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={submit(() => ({ action: 'launch', target }))}
      >
        <label className="flex flex-col gap-1 text-sm">
          {t.launch}
          <input
            className="rounded-md border px-2 py-1"
            value={target}
            placeholder={t.launchPlaceholder}
            onChange={(e) => setTarget(e.target.value)}
          />
        </label>
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
