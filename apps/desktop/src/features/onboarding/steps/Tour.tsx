import type { ReactElement } from 'react';
import { ProductArt } from '@/components/kivori/product-art';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { controlSupported, sessionState } from '@/lib/device-support';
import { StepFrame } from './frame';

const t = strings.onboarding.tour;

/** Capability-filtered guidance. Host observations do not verify physical controls. */
export function Tour({ connection }: { connection: ConnectionStatusDto | null }): ReactElement {
  const steps = (['rotate', 'press', 'button1'] as const).filter((control) =>
    controlSupported(connection, control),
  );
  return (
    <StepFrame title={t.title} body={t.body}>
      <div className="grid items-center gap-6 sm:grid-cols-[minmax(0,15rem)_1fr]">
        <ProductArt connection={connection} className="max-w-60" />
        {steps.length ? (
          <ol className="list-decimal space-y-3 pl-5" aria-label={t.title}>
            {steps.map((step) => (
              <li key={step} className="space-y-1 text-sm">
                <p className="font-medium">{t.steps[step].name}</p>
                <p className="text-muted-foreground">{t.steps[step].hint}</p>
              </li>
            ))}
          </ol>
        ) : (
          <p className="text-sm text-muted-foreground">
            {sessionState(connection) === 'ready' ? t.noControls : t.needDevice}
          </p>
        )}
      </div>
    </StepFrame>
  );
}
