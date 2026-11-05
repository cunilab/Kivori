import type { ReactElement, ReactNode } from 'react';
import { cn } from '@kivori/ui/lib/utils';

export function Eyebrow({ children }: { children: ReactNode }): ReactElement {
  return <p className="text-sm font-semibold text-teal">{children}</p>;
}

interface SectionHeadingProps {
  id: string;
  eyebrow?: string | undefined;
  title: string;
  intro?: string | undefined;
  align?: 'left' | 'center';
  size?: 'display-2' | 'display-3';
}

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
      {intro ? (
        <p className={cn('lead mt-4 max-w-[60ch]', align === 'center' && 'mx-auto')}>{intro}</p>
      ) : null}
    </div>
  );
}

interface SectionProps extends Omit<SectionHeadingProps, 'id'> {
  id: string;
  customHeading?: boolean;
  className?: string;
  children: ReactNode;
}

export function Section({
  id,
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
      className={cn('scroll-mt-20 overflow-x-clip px-5 py-16 sm:px-8 sm:py-24', className)}
    >
      <div className="mx-auto max-w-[1180px]">
        {hasHeading ? <SectionHeading id={`${id}-title`} {...heading} /> : null}
        <div className={hasHeading ? 'mt-9 sm:mt-12' : ''}>{children}</div>
      </div>
    </section>
  );
}
