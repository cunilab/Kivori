import type { ReactElement, ReactNode } from 'react';
import { cn } from '@kivori/ui/lib/utils';

/** Pixel-font mint label above a headline. */
export function Eyebrow({ children }: { children: ReactNode }): ReactElement {
  return <p className="pixel text-xs text-mint">{children}</p>;
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
      <h2 id={id} className={cn(size, eyebrow && 'mt-3.5')}>
        {title}
      </h2>
      {intro ? <p className="lead mt-4 max-w-[52ch]">{intro}</p> : null}
    </div>
  );
}

interface SectionProps extends Omit<SectionHeadingProps, 'id'> {
  id: string;
  /** `surface` lifts the band one step above the stage. */
  tone?: 'plain' | 'surface';
  /** The page renders its own `<h2 id="{id}-title">`, so the band is still labelled by it. */
  customHeading?: boolean;
  className?: string;
  children: ReactNode;
}

/** Page band: hairline on top, centred 1180 px column, optional heading block. */
export function Section({
  id,
  tone = 'plain',
  customHeading = false,
  className,
  children,
  ...heading
}: SectionProps): ReactElement {
  const hasHeading = Boolean(heading.title);
  return (
    <section
      id={id}
      aria-labelledby={hasHeading || customHeading ? `${id}-title` : undefined}
      className={cn(
        'scroll-mt-14 overflow-x-clip border-t border-border px-5 py-16 sm:px-8 sm:py-24',
        tone === 'surface' && 'bg-surface',
        className,
      )}
    >
      <div className="mx-auto max-w-[1180px]">
        {hasHeading ? <SectionHeading id={`${id}-title`} {...heading} /> : null}
        <div className={hasHeading ? 'mt-10 sm:mt-14' : ''}>{children}</div>
      </div>
    </section>
  );
}
