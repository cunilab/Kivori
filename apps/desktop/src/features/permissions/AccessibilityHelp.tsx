import type { ReactElement } from 'react';
import { ShieldAlert } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@kivori/ui/components/alert';
import { strings } from '@/lib/i18n/strings';
import { AccessibilityButtons, StaleNote, useAccessibilityActions } from './AccessibilityActions';
import { useAccessibility } from './use-accessibility';

const t = strings.accessibility;

/**
 * Asks for the macOS Accessibility permission where it is needed, and only then: it renders
 * nothing unless macOS says the permission is off (so nothing on other systems, or once granted).
 * Each button asks at most once per press. When the user came back and it still reads off, it
 * offers to reset this app's old approval and ask again.
 */
export function AccessibilityHelp({
  title,
  body,
}: {
  title: string;
  body: string;
}): ReactElement | null {
  const accessibility = useAccessibility(true);
  const actions = useAccessibilityActions(accessibility);
  if (accessibility.state !== 'missing') return null;
  return (
    <Alert className="border-warning/40 bg-warning/5" data-testid="accessibility-help">
      <ShieldAlert className="text-warning" aria-hidden="true" />
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription className="space-y-3">
        <p>{body}</p>
        <p>{t.steps}</p>
        <AccessibilityButtons actions={actions} />
        {accessibility.stale ? <StaleNote actions={actions} /> : null}
      </AlertDescription>
    </Alert>
  );
}
