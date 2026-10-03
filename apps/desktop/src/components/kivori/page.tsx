import type { ReactElement, ReactNode } from 'react';

/** Page frame: one h1 per screen (focused on navigation for screen readers), then content. */
export function Page({
  title,
  description,
  actions,
  children,
}: {
  title: string;
  description: string;
  actions?: ReactNode;
  children: ReactNode;
}): ReactElement {
  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-4 py-6 sm:px-6 lg:px-8">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div className="space-y-1">
          <h1
            tabIndex={-1}
            data-page-title
            className="text-2xl font-semibold tracking-tight outline-none"
          >
            {title}
          </h1>
          <p className="text-sm text-muted-foreground">{description}</p>
        </div>
        {actions}
      </div>
      {children}
    </div>
  );
}
