import Link from 'next/link';
import type { ReactElement } from 'react';
import { Section } from '@/components/section';
import { WaitlistSection } from '@/components/waitlist-section';

const RULES = [
  { app: 'Browser', description: 'Turn the knob to move through your tabs.' },
  { app: 'Music', description: 'Skip tracks, pause and adjust the volume.' },
  { app: 'Meetings', description: 'Keep your meeting shortcuts one press away.' },
  { app: 'Code', description: 'Set up your own keys for your editor.' },
] as const;

export function ProfilesSection(): ReactElement {
  return (
    <Section id="profiles" title="" customHeading>
      <div className="grid gap-10 md:grid-cols-[1fr_1.3fr] md:gap-20">
        <div className="max-w-md">
          <h2 id="profiles-title" className="display-2">
            Built around your day.
          </h2>
          <p className="lead mt-4">
            Controls change with the app in front of you. Customize them in the desktop app, or pin
            a profile to keep it in place.
          </p>
          <Link
            href="/products/kv-p1"
            className="mt-5 inline-block font-semibold text-teal underline underline-offset-4"
          >
            Explore the details
          </Link>
        </div>
        <dl className="border-t border-border">
          {RULES.map((rule) => (
            <div
              key={rule.app}
              className="grid gap-1 border-b border-border py-4 sm:grid-cols-[9rem_1fr] sm:gap-4"
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
      className="scroll-mt-20 px-5 pb-20 sm:px-8 sm:pb-28"
    >
      <div className="mx-auto grid max-w-[1180px] gap-8 rounded-lg bg-surface p-7 sm:p-10 md:grid-cols-2 md:gap-16">
        <div>
          <h2 id="waitlist-title" className="display-2">
            Get updates on KV-P1.
          </h2>
          <p className="lead mt-4 max-w-md">
            There is no price or ship date yet. Leave your email and we will tell you when there is.
          </p>
        </div>
        <div className="flex items-center">
          <WaitlistSection initialProduct={initialProduct} idPrefix="final" variant="inline" />
        </div>
      </div>
    </section>
  );
}
