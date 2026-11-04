import Image from 'next/image';
import type { ReactElement } from 'react';
import mascot from '@brand/mascot.svg';
import { Eyebrow, Section } from '@/components/section';
import { WaitlistSection } from '@/components/waitlist-section';

const RULES: readonly { app: string; description: string }[] = [
  { app: 'Browser', description: 'Turn the knob to move through your tabs.' },
  { app: 'Music', description: 'Skip tracks, pause and adjust the volume.' },
  { app: 'Meetings', description: 'Keep your meeting shortcuts one press away.' },
  { app: 'Code', description: 'Set up your own keys for your editor.' },
];

export function ProfilesSection(): ReactElement {
  return (
    <Section id="profiles" customHeading title="" className="border-t-0">
      <div className="grid items-start gap-8 md:grid-cols-[5fr_7fr] md:gap-16">
        <div>
          <Eyebrow>How it fits your day</Eyebrow>
          <h2 id="profiles-title" className="display-2 mt-3.5">
            Less reaching. More doing.
          </h2>
          <p className="lead mt-4 max-w-[45ch]">
            Kivori changes controls with the app in front of you. Customize them in the desktop app,
            or pin a profile to keep it in place.
          </p>
        </div>
        <dl className="border-t border-border">
          {RULES.map((rule) => (
            <div
              key={rule.app}
              className="grid gap-1 border-b border-border py-5 sm:grid-cols-[9rem_1fr] sm:gap-4"
            >
              <dt className="font-semibold">{rule.app}</dt>
              <dd className="text-muted-foreground">{rule.description}</dd>
            </div>
          ))}
        </dl>
      </div>
    </Section>
  );
}

export function FinalBand({
  initialProduct,
}: {
  initialProduct?: string | undefined;
}): ReactElement {
  return (
    <section
      id="waitlist"
      aria-labelledby="waitlist-title"
      className="scroll-mt-20 border-t border-border px-5 py-16 sm:px-8 sm:py-24"
    >
      <div className="mx-auto flex max-w-xl flex-col items-center text-center">
        <Image src={mascot} alt="" width={72} height={72} />
        <h2 id="waitlist-title" className="display-2 mt-6">
          Want one? Get on the list.
        </h2>
        <p className="lead mt-4">
          There is no price or ship date yet. Leave your email and we will tell you when there is.
        </p>
        <div className="mt-8 flex w-full justify-center text-left">
          <WaitlistSection initialProduct={initialProduct} idPrefix="final" variant="inline" />
        </div>
      </div>
    </section>
  );
}
