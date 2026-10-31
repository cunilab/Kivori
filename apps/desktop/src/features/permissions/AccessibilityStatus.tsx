import type { ReactElement } from 'react';
import { CircleCheck, ShieldAlert } from 'lucide-react';
import { Badge } from '@kivori/ui/components/badge';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import { strings } from '@/lib/i18n/strings';
import { AccessibilityButtons, StaleNote, useAccessibilityActions } from './AccessibilityActions';
import { useAccessibility } from './use-accessibility';

const t = strings.accessibility;

/**
 * Settings: whether macOS lets Kivori press keys, always visible on macOS. Not allowed shows the
 * ask (and, when macOS still reads off after the user came back, the reset). Nothing on other
 * systems, where there is no such permission.
 */
export function AccessibilityStatus(): ReactElement | null {
  const accessibility = useAccessibility(true);
  const actions = useAccessibilityActions(accessibility);
  const { state } = accessibility;
  if (state !== 'granted' && state !== 'missing') return null;
  const allowed = state === 'granted';
  return (
    <Card size="sm" data-testid="accessibility-status" data-state={state}>
      <CardHeader>
        <CardTitle>{t.settings.title}</CardTitle>
        <CardDescription>{t.settings.description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <Badge
          variant="outline"
          role="status"
          className={allowed ? 'text-success' : 'text-warning'}
        >
          {allowed ? <CircleCheck aria-hidden="true" /> : <ShieldAlert aria-hidden="true" />}
          {allowed ? t.settings.allowed : t.settings.notAllowed}
        </Badge>
        {allowed ? (
          <p className="text-sm text-muted-foreground">{t.settings.allowedNote}</p>
        ) : (
          <>
            <p className="text-sm">{t.steps}</p>
            <AccessibilityButtons actions={actions} />
            {accessibility.stale ? <StaleNote actions={actions} /> : null}
          </>
        )}
      </CardContent>
    </Card>
  );
}
