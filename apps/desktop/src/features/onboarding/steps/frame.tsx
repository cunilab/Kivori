import { useEffect, useRef } from 'react';
import type { ReactElement, ReactNode } from 'react';

/** One step's heading and body. The heading takes focus when the step appears. */
export function StepFrame({
  title,
  body,
  children,
}: {
  title: string;
  body?: string | undefined;
  children?: ReactNode;
}): ReactElement {
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => heading.current?.focus({ preventScroll: true }), []);
  return (
    <section aria-labelledby="onboarding-step-title" className="space-y-5">
      <div className="space-y-1.5">
        <h1
          id="onboarding-step-title"
          ref={heading}
          tabIndex={-1}
          className="text-2xl font-semibold tracking-tight outline-none"
        >
          {title}
        </h1>
        {body ? <p className="max-w-prose text-sm text-muted-foreground">{body}</p> : null}
      </div>
      {children}
    </section>
  );
}
