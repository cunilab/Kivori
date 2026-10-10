import type { ReactElement, ReactNode } from 'react';
import { cn } from '@kivori/ui/lib/utils';

export function Eyebrow({ children }: { children: ReactNode }): ReactElement {
  return (
    <p className="text-sm font-semibold tracking-tight text-primary sm:text-base">{children}</p>
  );
}

interface SectionHeadingProps {
  id: string;
  eyebrow?: string | undefined;
  title: string;
  intro?: string | undefined;
  align?: 'left' | 'center';
  size?: 'display-2' | 'display-3';
}

/** Eyebrow, headline and intro, as one block. `id` is the heading id (sections `aria-labelledby` it). */
export function SectionHeading({
  id,
  eyebrow,
  title,
  intro,
  align = 'left',
  size = 'display-2',
}: SectionHeadingProps): ReactElement {
  return (
    <div className={cn('max-w-3xl', align === 'center' && 'mx-auto text-center')}>
      {eyebrow ? <Eyebrow>{eyebrow}</Eyebrow> : null}
      <h2 id={id} className={cn(size, eyebrow && 'mt-3')}>
        {title}
      </h2>
      {intro ? <p className="lead mt-5">{intro}</p> : null}
    </div>
  );
}

interface SectionProps extends Omit<SectionHeadingProps, 'id'> {
  id: string;
  tone?: 'plain' | 'surface' | 'dark';
  className?: string;
  children: ReactNode;
}

/**
 * Page band. `tone="dark"` scopes the shared dark tokens to the band, so shared primitives inside it
 * pick up the mint accent on the brand navy, whatever the page theme is.
 */
export function Section({
  id,
  tone = 'plain',
  className,
  children,
  ...heading
}: SectionProps): ReactElement {
  const hasHeading = Boolean(heading.title);
  return (
    <section
      id={id}
      aria-labelledby={hasHeading ? `${id}-title` : undefined}
      className={cn(
        'scroll-mt-14 overflow-x-clip px-5 py-20 sm:px-8 sm:py-28',
        tone === 'surface' && 'bg-surface',
        tone === 'dark' && 'dark bg-[#0c101c] text-foreground',
        className,
      )}
    >
      <div className="mx-auto max-w-[1200px]">
        {hasHeading ? <SectionHeading id={`${id}-title`} {...heading} /> : null}
        <div className={hasHeading ? 'mt-12 sm:mt-16' : ''}>{children}</div>
      </div>
    </section>
  );
}
