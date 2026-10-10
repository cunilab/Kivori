import type { ReactElement, ReactNode } from 'react';

interface SectionProps {
  id: string;
  eyebrow?: string;
  title: string;
  intro?: string;
  tone?: 'plain' | 'muted';
  children: ReactNode;
}

/** Page band with a consistent heading block. `id` doubles as the anchor and the heading's id seed. */
export function Section({
  id,
  eyebrow,
  title,
  intro,
  tone = 'plain',
  children,
}: SectionProps): ReactElement {
  return (
    <section
      id={id}
      aria-labelledby={`${id}-title`}
      className={`scroll-mt-20 px-5 py-16 sm:py-20 ${tone === 'muted' ? 'border-y border-border bg-secondary/50' : ''}`}
    >
      <div className="mx-auto max-w-5xl">
        <div className="max-w-2xl">
          {eyebrow ? (
            <p className="text-sm font-medium tracking-wide text-primary uppercase">{eyebrow}</p>
          ) : null}
          <h2 id={`${id}-title`} className="mt-2 font-display text-3xl font-semibold sm:text-4xl">
            {title}
          </h2>
          {intro ? <p className="mt-4 text-lg text-muted-foreground">{intro}</p> : null}
        </div>
        <div className="mt-10">{children}</div>
      </div>
    </section>
  );
}
