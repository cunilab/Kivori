import type { ReactElement } from 'react';
import { DeviceArt } from '@/components/kivori/device-art';
import { strings } from '@/lib/i18n/strings';
import { format } from '@/lib/i18n/strings';
import { StepFrame } from './frame';

const t = strings.onboarding.welcome;

export function Welcome(): ReactElement {
  return (
    <StepFrame title={t.title} body={t.body}>
      <DeviceArt
        mode="buddy"
        label={format(strings.home.hero.illustration, { mode: strings.display.modes.buddy.name })}
        className="w-full max-w-64"
      />
    </StepFrame>
  );
}
