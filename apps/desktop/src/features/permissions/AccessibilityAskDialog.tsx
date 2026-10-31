import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@kivori/ui/components/alert-dialog';
import { Button } from '@kivori/ui/components/button';
import { strings } from '@/lib/i18n/strings';
import { StaleNote, useAccessibilityActions } from './AccessibilityActions';
import { useAccessibility } from './use-accessibility';

const t = strings.accessibility.ask;

/**
 * The one automatic Accessibility ask for an install that finished setup before Kivori asked
 * (first-run setup has its own step). `ready` says setup is done and the stored answer is known;
 * `asked` is whether Kivori ever asked on this install. It is decided once per run, when both are
 * known: shown only if macOS says the permission is off and Kivori never asked, and recorded
 * (`onShown`) the moment it shows, so it never comes back by itself, whatever the person does.
 * It closes by itself when the permission turns on (the window regaining focus re-reads it).
 * Allow shows the macOS prompt once and opens the Accessibility list.
 */
export function AccessibilityAskDialog({
  ready,
  asked,
  onShown,
}: {
  ready: boolean;
  asked: boolean;
  onShown: () => void;
}): ReactElement | null {
  const accessibility = useAccessibility(false);
  const actions = useAccessibilityActions(accessibility);
  const [shown, setShown] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [sent, setSent] = useState(false);
  const decided = useRef(false);

  useEffect(() => {
    if (decided.current || !ready || accessibility.state === null) return;
    decided.current = true;
    if (asked || accessibility.state !== 'missing') return;
    setShown(true);
    onShown();
  }, [ready, asked, accessibility.state, onShown]);

  // Switched on (or the window came back and macOS now says on): the card has no more to say.
  useEffect(() => {
    if (shown && accessibility.state !== 'missing') setDismissed(true);
  }, [shown, accessibility.state]);

  if (!shown) return null;
  const allow = (): void => {
    setSent(true);
    actions.allowAndOpen();
  };
  return (
    <AlertDialog open={!dismissed} onOpenChange={(next) => !next && setDismissed(true)}>
      <AlertDialogContent
        data-testid="accessibility-ask"
        className="data-[size=default]:sm:max-w-md"
      >
        <AlertDialogHeader>
          <AlertDialogTitle>{t.title}</AlertDialogTitle>
          <AlertDialogDescription>{t.body}</AlertDialogDescription>
        </AlertDialogHeader>
        {sent ? (
          <p role="status" className="text-sm">
            {t.sent}
          </p>
        ) : (
          <p className="text-sm text-muted-foreground">{t.later}</p>
        )}
        {accessibility.stale ? <StaleNote actions={actions} /> : null}
        <AlertDialogFooter>
          <Button variant="outline" onClick={() => setDismissed(true)}>
            {sent ? t.close : t.notNow}
          </Button>
          {sent ? null : (
            <Button onClick={allow} disabled={actions.busy}>
              {t.allow}
            </Button>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
