import type { ReactElement } from 'react';
import { ProductArt } from '@/components/kivori/product-art';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { strings } from '@/lib/i18n/strings';
import { StepFrame } from './frame';

const t = strings.onboarding.welcome;

export function Welcome({ connection }: { connection: ConnectionStatusDto | null }): ReactElement {
  return (
    <StepFrame title={t.title} body={t.body}>
      <ProductArt connection={connection} className="max-w-64" />
    </StepFrame>
  );
}
