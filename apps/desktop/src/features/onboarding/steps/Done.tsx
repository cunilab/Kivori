import type { ReactElement } from 'react';
import { CircleCheck } from 'lucide-react';
import { strings } from '@/lib/i18n/strings';
import { StepFrame } from './frame';

const t = strings.onboarding.done;

export function Done({ error }: { error: string | null }): ReactElement {
  return (
    <StepFrame title={t.title} body={t.body}>
      <CircleCheck className="size-10 text-success" aria-hidden="true" />
      {error ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
    </StepFrame>
  );
}
