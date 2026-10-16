import type { ReactElement } from 'react';
import { CircleCheck, Circle } from 'lucide-react';
import { ProductArt } from '@/components/kivori/product-art';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { cn } from '@kivori/ui/lib/utils';
import { TOUR_STEPS } from '../use-onboarding';
import { StepFrame } from './frame';

const t = strings.onboarding.tour;

/** The product figure and the three live checks; `done` counts the ones already seen. */
export function Tour({
  connection,
  done,
  connected,
}: {
  connection: ConnectionStatusDto | null;
  done: number;
  connected: boolean;
}): ReactElement {
  return (
    <StepFrame title={t.title} body={t.body}>
      <div className="grid items-center gap-6 sm:grid-cols-[minmax(0,15rem)_1fr]">
        <ProductArt connection={connection} className="max-w-60" />
        <ol className="space-y-3" aria-label={t.title}>
          {TOUR_STEPS.map((step, index) => {
            const finished = index < done;
            const active = index === done;
            return (
              <li
                key={step}
                aria-current={active ? 'step' : undefined}
                data-done={finished}
                className={cn('flex items-start gap-3', !finished && !active && 'opacity-60')}
              >
                {finished ? (
                  <CircleCheck className="mt-0.5 size-5 text-success" aria-hidden="true" />
                ) : (
                  <Circle className="mt-0.5 size-5 text-muted-foreground" aria-hidden="true" />
                )}
                <div className="text-sm">
                  <p className="font-medium">
                    {t.steps[step].name}
                    {finished ? <span className="sr-only"> ({t.done})</span> : null}
                  </p>
                  {active ? <p className="text-muted-foreground">{t.steps[step].hint}</p> : null}
                </div>
              </li>
            );
          })}
        </ol>
      </div>
      {!connected ? <p className="text-sm text-muted-foreground">{t.needDevice}</p> : null}
    </StepFrame>
  );
}
